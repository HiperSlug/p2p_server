use std::{collections::HashMap, net::{SocketAddr, SocketAddrV4}, sync::Arc, time::{Duration, Instant}, pin::Pin};
use crate::puncher::{puncher_service_server::{PuncherService, PuncherServiceServer}, AddListingRequest, AddListingResponse, CreateSessionRequest, CreateSessionResponse, EndSessionRequest, EndSessionResponse, GetListingsRequest, GetListingsResponse, JoinRequest, JoinResponse, Order, Ping, RemoveListingRequest, RemoveListingResponse};
use crate::puncher::Listing as ListingPacket;
use crate::puncher::ListingNoId as ListingNoIdPacket;
use anyhow::{anyhow, bail, Result};
use futures::{Stream, StreamExt};
use rand::random;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use uuid::Uuid;

// -- Session -- //
type SessionRef = Arc<RwLock<Session>>;
type OrderStream = mpsc::Sender<Result<Order, Status>>;

struct Session {
	id: Uuid,
	addr: SocketAddr,
	last_seen: Instant,
	listing: Option<Listing>,
	stream: Option<OrderStream>,
}

impl Session {
	const TIMEOUT: Duration = Duration::from_secs(60 * 15);

	pub async fn new(addr: SocketAddr) -> SessionRef {
		Arc::new(RwLock::new(Self {
			id: Uuid::new_v4(),
			last_seen: Instant::now(),
			listing: None,
			stream: None,
			addr,
		}))
	}

	pub fn see(&mut self) {
		self.last_seen = Instant::now();
	}

	pub fn is_valid(&self) -> bool {
		self.last_seen.elapsed() < Self::TIMEOUT
	}
}

// -- Listing -- //
struct Listing {
	listing_no_id: ListingNoId,
	id: Uuid,
}

impl Listing {
	fn new(listing_no_id: ListingNoId) -> Listing {
		Listing {
			listing_no_id,
			id: Uuid::new_v4(),
		}
	}
}

impl TryFrom<ListingPacket> for Listing {
	type Error = anyhow::Error;

	fn try_from(listing_packet: ListingPacket) -> Result<Self> {
		Ok(Self {
			listing_no_id: listing_packet
				.listing_no_id
				.ok_or(anyhow!("Empty packet."))?
				.into(),
			id: listing_packet.id.try_into()?,
		})
	}
}

impl From<&Listing> for ListingPacket {
	fn from(listing: &Listing) -> Self {
		Self {
			listing_no_id: Some(listing.listing_no_id.clone().into()),
			id: listing.id.into(),
		}
	}
}

#[derive(Clone)]
struct ListingNoId {
	name: String, 
}

impl From<ListingNoIdPacket> for ListingNoId {
	fn from(listing_no_id_packet: ListingNoIdPacket) -> Self {
		Self {
			name: listing_no_id_packet.name,
		}
	}
}

impl From<ListingNoId> for ListingNoIdPacket {
	fn from(listing_no_id: ListingNoId) -> Self {
		Self {
			name: listing_no_id.name,
		}
	}
}

// -- Server -- //
#[derive(Default)]
pub struct PuncherServer {
	sessions: Arc<RwLock<HashMap<Uuid, SessionRef>>>,
	id_map: Arc<RwLock<HashMap<Uuid, Uuid>>>,
}

impl PuncherServer {
	

	async fn get(&self, session_id: &Uuid) -> Option<SessionRef> {
		let sessions = self.sessions.read().await;
		sessions.get(session_id).map(|s| s.clone())
	}

	async fn validate(&self, session_id: &Uuid) -> Result<SessionRef> {
		let session = self
			.get(session_id)
			.await
			.ok_or(anyhow!("Not found."))?
			.clone();

		{
			let session = session.read().await;
			
			if !session.is_valid() {
				self.remove_deep(&[*session_id]).await;
				
				bail!("Expired.")
			}
		}
		
		Ok(session)
	}

	async fn remove_deep(&self, session_ids: &[Uuid]) {
		let mut sessions = self.sessions.write().await;

		let removed_sessions = session_ids
			.iter()
			.filter_map(|id| sessions.remove(id))
			.collect::<Vec<SessionRef>>();
		
		let mut id_map = self.id_map.write().await;
		for session in removed_sessions {
			let session = session.write().await;

			if let Some(listing) = session.listing.as_ref() {
				id_map.remove(&listing.id);
			}
		}
	}

	async fn remove_expired(&self) {
		let sessions = self.sessions.read().await;
		let mut expired = Vec::new();
		for (id, session) in sessions.iter() {
			let session = session.read().await;
			if !session.is_valid() {
				expired.push(id.clone());
			}
		};

		self.remove_deep(&expired).await;
	}

	pub async fn remove_expired_chance(&self) {
		if random::<f32>() < 0.1 {
			self.remove_expired().await;
		}
	}
}


#[tonic::async_trait]
impl PuncherService for PuncherServer {
	type StreamSessionStream = Pin<Box<dyn Stream<Item = Result<Order, Status>> + Send + Sync + 'static>>;

	// -- listings -- //
	async fn add_listing(
        &self,
        request: Request<AddListingRequest>,
    ) -> Result<Response<AddListingResponse>, Status> {
		self.remove_expired_chance().await;

		let request = request.into_inner();
		
		// validate session //
		let session_id = request
			.session_id
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let session = self
			.validate(&session_id)
			.await
			.map_err(|e| Status::invalid_argument(format!("Invalid session_id: {e}")))?;
		

		// validate assignment //
		{
			let session = session.read().await;
			if session.listing.is_some() {
				return Err(Status::already_exists("This session already has an associated listing."))
			}
		}


		// generate listing //
		let listing_no_id_packet = request
			.listing
			.ok_or(Status::invalid_argument("No supplied listing."))?;
		let listing = Listing::new(listing_no_id_packet.into());
		

		// assign listing //
		let mut id_map = self.id_map.write().await;
		id_map.insert(listing.id, session_id);

		let listing_id = listing.id.into();

		let mut session = session.write().await;
		session.listing = Some(listing);


		Ok(Response::new(AddListingResponse { listing_id }))
    }

    async fn remove_listing(
        &self,
        request: Request<RemoveListingRequest>,
    ) -> Result<Response<RemoveListingResponse>, Status> {
		self.remove_expired_chance().await;

        let request = request.into_inner();

		
		// validate session //
		let session_id = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let session = self
			.validate(&session_id)
			.await
			.map_err(|e| Status::invalid_argument(format!("Invalid session_id: {e}")))?;

		
		// assignment //
		let mut session = session.write().await;

		if let Some(listing) = session.listing.as_ref() {
			let mut id_map = self.id_map.write().await;
			id_map.remove(&listing.id);
			
			session.listing = None;
		}

		Ok(Response::new(RemoveListingResponse {}))
    }

    async fn get_listings(
        &self,
        _: Request<GetListingsRequest>,
    ) -> Result<Response<GetListingsResponse>, Status> {
		self.remove_expired_chance().await;


        let sessions = self.sessions.read().await;
		let mut listings = Vec::new();
		for (_, session) in sessions.iter() {
			let session = session.read().await;
			if let Some(listing) = session.listing.as_ref() {
				listings.push(listing.into());
			}
		}

		Ok(Response::new(GetListingsResponse { listings }))
    }

	// -- connection -- //
	async fn create_session(
		&self,
		request: Request<CreateSessionRequest>,
	) -> Result<Response<CreateSessionResponse>, Status> {
		self.remove_expired_chance().await;

		let request = request.into_inner();


		// parse addr //
		let ip = request
			.ip
			.parse()
			.map_err(|e| Status::invalid_argument(format!("Invalid ip: {e}")))?;
		let port = request
			.port
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid port: {e}")) )?;
		let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));


		// create session //
		let session = Session::new(addr).await;
		
		let session_id = {
			let session = session.read().await;
			session.id
		};

		let session_id_bytes = session_id.into();

		let mut sessions = self.sessions.write().await;
		sessions.insert(session_id, session.into());
		
		Ok(Response::new(CreateSessionResponse { session_id: session_id_bytes }))
	}

	async fn stream_session(
		&self,
		request: Request<Streaming<Ping>>,
	) -> Result<Response<Self::StreamSessionStream>, Status> {
		let mut request = request.into_inner();

		let session_id = request
			.message()
			.await?
			.ok_or(Status::invalid_argument("No first ping."))?
			.session_id
			.ok_or(Status::invalid_argument("No session_id in first ping."))?
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;
		
		let session = self
			.get(&session_id)
			.await
			.ok_or(Status::invalid_argument("Session ID points to no session."))?;

		let (tx, rx) = mpsc::channel(32);
		
		
		let s = session.clone();
		tokio::spawn(async move {
			while let Some(ping_result) = request.next().await {
				if let Ok(_) = ping_result {
					let mut session = s.write().await;
					session.see();
				} else {
					break;
				}
			}
		});

		let mut session = session.write().await;
		session.stream = Some(tx);

		let stream = Box::pin(ReceiverStream::new(rx));
		Ok(Response::new(stream))
	}

	async fn end_session(
		&self,
		request: Request<EndSessionRequest>,
	) -> Result<Response<EndSessionResponse>, Status> {
		self.remove_expired_chance().await;

		let session_id = request
			.into_inner()
			.session_id
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		self.remove_deep(&[session_id]).await;

		Ok(Response::new(EndSessionResponse {}))
	}

	async fn join(
		&self,
		request: Request<JoinRequest>
	) -> Result<Response<JoinResponse>, Status> {
		let request = request.into_inner();

		// validate session //
		let session_id = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid session Uuid: {e}")))?;

		let session = self
			.validate(&session_id)
			.await
			.map_err(|e| Status::invalid_argument(format!("Invalid session id: {e}")))?;


		// validate target session //
		let target_listing_id = request
			.target_listing_id
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid listing Uuid: {e}")))?;
		
		let id_map = self.id_map.read().await;
		let target_session_id = id_map
			.get(&target_listing_id)
			.ok_or(Status::invalid_argument("Listing ID has no associated session."))?
			.clone();

		let target_session = self
			.validate(&target_session_id)
			.await
			.map_err(|e| Status::invalid_argument(format!("Invalid session id: {e}")))?;

		// TODO send Punch order

		// TODO handle Proxy fallback		

		Ok(Response::new(JoinResponse { }))
	}
}

pub async fn run() -> anyhow::Result<()> {
	let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
	let server = PuncherServer::default();

	Server::builder()
		.add_service(PuncherServiceServer::new(server))
		.serve(addr)
		.await?;
	
	Ok(())
}
