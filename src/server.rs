use std::{collections::HashMap, net::{SocketAddr, SocketAddrV4}, sync::Arc, time::{Duration, Instant}};
use crate::puncher::{peer_service_client::PeerServiceClient, puncher_service_server::{PuncherService, PuncherServiceServer}, AddListingRequest, AddListingResponse, CreateSessionRequest, CreateSessionResponse, EndSessionRequest, EndSessionResponse, ForwardJoinRequest, ForwardJoinResponse, GetListingsRequest, GetListingsResponse, PingRequest, PingResponse, PunchRequest, RemoveListingRequest, RemoveListingResponse};
use crate::puncher::Listing as ListingPacket;
use crate::puncher::ListingNoId as ListingNoIdPacket;
use anyhow::{anyhow, bail, Result};
use rand::random;
use tokio::sync::RwLock;
use tonic::{transport::{Channel, Server}, Request, Response, Status};
use uuid::Uuid;

// -- Session -- //
struct Session {
	id: Uuid,
	last_seen: Instant,
	listing: Option<Listing>,
	addr: SocketAddr,
	client: Arc<RwLock<PeerServiceClient<Channel>>>,
}

impl Session {
	const TIMEOUT: Duration = Duration::from_secs(30);

	pub async fn new(addr: SocketAddr) -> Result<Self> {
		let uri = format!("https://{addr}").parse()?;

		let channel = Channel::builder(uri)
			.connect()
			.await?;

		Ok(Self {
			id: Uuid::new_v4(),
			last_seen: Instant::now(),
			listing: None,
			addr,
			client: Arc::new(RwLock::new(PeerServiceClient::new(channel)))
		})
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
	sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
	id_map: Arc<RwLock<HashMap<Uuid, Uuid>>>,
}

impl PuncherServer {
	async fn check(&self, session_id: &Uuid) -> Result<()> {
		let sessions = self.sessions.read().await;
		let session = sessions.get(session_id).ok_or(anyhow!("Not found."))?;
		
		if !session.is_valid() {
			self.remove_deep(&[*session_id]).await;
			
			bail!("Expired.")
		} else {
			Ok(())
		}
	}

	async fn remove_deep(&self, session_ids: &[Uuid]) {
		let mut sessions = self.sessions.write().await;
		let removed_sessions = session_ids
			.iter()
			.filter_map(|id| sessions.remove(id))
			.collect::<Vec<Session>>();
		
		let mut id_map = self.id_map.write().await;
		removed_sessions
			.iter()
			.for_each(|s| {
				if let Some(listing) = s.listing.as_ref() {
					id_map.remove(&listing.id);
				}
			});
	}

	async fn cleanup(&self) {
		let expired = {
			let sessions = self.sessions.read().await;
			sessions
				.iter()
				.filter(|(_, s)| !s.is_valid())
				.map(|(id, _)| *id)
				.collect::<Vec<Uuid>>()
		};

		if expired.is_empty() {
			return;
		}

		self.remove_deep(&expired).await;
	}

	pub async fn cleanup_chance(&self) { // I couldnt be bothered to spawn and despawn an async task.
		if random::<f32>() < 0.1 {
			self.cleanup().await;
		}
	}
}

#[tonic::async_trait]
impl PuncherService for PuncherServer {
	// -- listings -- //
	async fn add_listing(
        &self,
        request: Request<AddListingRequest>,
    ) -> Result<Response<AddListingResponse>, Status> {
		self.cleanup_chance().await;

		let request = request.into_inner();

		// validate session //
		let session_id = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		self.check(&session_id).await.map_err(|e| Status::invalid_argument(format!("Invalid session_id: {e}")))?;

		let mut sessions = self.sessions.write().await;
		let session = sessions.get_mut(&session_id)
			.ok_or(Status::internal("Session ID expired or non-existent (after session validation)"))?;

		// validate assignment //
		if session.listing.is_some() {
			return Err(Status::already_exists("This session already has an associated listing."))
		}

		// assignment //
		let listing_no_id_packet = request
			.listing
			.ok_or(Status::invalid_argument("No supplied listing."))?;
		let listing = Listing::new(listing_no_id_packet.into());
		
		let mut id_map = self.id_map.write().await;
		id_map.insert(listing.id, session_id);

		let listing_id = listing.id.into();

		session.listing = Some(listing);

		Ok(Response::new(AddListingResponse { listing_id }))
    }

    async fn remove_listing(
        &self,
        request: Request<RemoveListingRequest>,
    ) -> Result<Response<RemoveListingResponse>, Status> {
		self.cleanup_chance().await;

        let request = request.into_inner();

		// validate session //
		let session_id = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		self.check(&session_id).await.map_err(|e| Status::invalid_argument(format!("Invalid session_id: {e}")))?;

		let mut sessions = self.sessions.write().await;
		let session = sessions.get_mut(&session_id)
			.ok_or(Status::internal("Session ID expired or non-existent (after session validation)"))?;

		// assignment //
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
		self.cleanup_chance().await;

        let sessions = self.sessions.read().await;
		let listings = sessions
			.iter()
			.filter_map(|(_, s)| s.listing.as_ref().map(|l| l.into()))
			.collect::<Vec<ListingPacket>>();
		Ok(Response::new(GetListingsResponse { listings }))
    }

	// -- connection -- //
	async fn create_session(
		&self,
		request: Request<CreateSessionRequest>,
	) -> Result<Response<CreateSessionResponse>, Status> {
		let request = request.into_inner();
		
		let ip = request
			.ip
			.parse()
			.map_err(|e| Status::invalid_argument(format!("Invalid ip: {e}")))?;
		let port = request
			.port
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid port: {e}")) )?;
		let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));

		let session = Session::new(addr)
			.await
			.map_err(|e| Status::internal(format!("Unable to parse addr despite having already parsed addr: {e}")))?;
		
		let session_id = session.id.into();

		let mut sessions = self.sessions.write().await;
		sessions.insert(session.id, session);
		
		Ok(Response::new(CreateSessionResponse {session_id}))
	}

	async fn end_session(
		&self,
		request: Request<EndSessionRequest>,
	) -> Result<Response<EndSessionResponse>, Status> {
		let request = request.into_inner();

		let session_id = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		self.remove_deep(&[session_id]).await;

		Ok(Response::new(EndSessionResponse {}))
	}

	async fn ping(
		&self,
		request: Request<PingRequest>,
	) -> Result<Response<PingResponse>, Status> {
		let session_id = request
			.into_inner()
			.session_id
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let mut sessions = self.sessions.write().await;
		let session = sessions.get_mut(&session_id).ok_or(Status::invalid_argument("Session ID expired or non-existent."))?;

		session.see();

		Ok(Response::new(PingResponse {}))
	}

	async fn forward_join(
		&self,
		request: Request<ForwardJoinRequest>
	) -> Result<Response<ForwardJoinResponse>, Status> {
		let request = request.into_inner();
		
		let join_request = request.request
			.ok_or(Status::invalid_argument("No join request supplied."))?;

		// validate session //
		let session_id = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid session Uuid: {e}")))?;

		self.check(&session_id).await.map_err(|e| Status::invalid_argument(format!("Invalid session id: {e}")))?;


		// validate target session //
		let target_listing_id = request.target_listing_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid listing Uuid: {e}")))?;
		
		let id_map = self.id_map.read().await;
		let target_session_id = id_map
			.get(&target_listing_id)
			.ok_or(Status::invalid_argument("Listing ID has no associated session."))?
			.clone();

		self.check(&target_session_id).await.map_err(|e| Status::invalid_argument(format!("Invalid target session id: {e}")))?;

		// forward request to target //
		let target_client = {
			let sessions = self.sessions.read().await;
			sessions
				.get(&target_session_id)
				.ok_or(Status::internal("Target session non existent despite having a respective map."))?
				.client.clone()
		};

		{
			let mut target_client = target_client.write().await;

			// return err response, can be changed to handle custom data later
			let _ = target_client.join(join_request).await?;
		}
		
		let joining_client = {
			let sessions = self.sessions.read().await;
			sessions
				.get(&session_id)
				.ok_or(Status::internal("Target session non existent despite having a respective map."))?
				.client.clone()
		};

		// getting addr
		let target_addr = {
			let sessions = self.sessions.read().await;
			let session = sessions.get(&target_session_id).ok_or(Status::internal("Invalid session after session validation."))?;
			session.addr
		};
		let joining_addr = {
			let sessions = self.sessions.read().await;
			let session = sessions.get(&session_id).ok_or(Status::internal("Invalid session after session validation."))?;
			session.addr
		};

		// punch
		let (target_response, joining_response) = tokio::join!(
			async {
				let (ip, port) = { (joining_addr.ip().to_string(), joining_addr.port().into()) };
				let target_client = target_client.clone();
				let mut target_client = target_client.write().await;

				target_client.punch(PunchRequest { ip, port }).await
			},
			async {
				let (ip, port) = { (target_addr.ip().to_string(), target_addr.port().into()) };
				let joining_client = joining_client.clone();
				let mut joining_client = joining_client.write().await;

				joining_client.punch(PunchRequest { ip, port }).await
				}
		);

		if target_response.is_ok() && joining_response.is_ok() {
			return Ok(Response::new(ForwardJoinResponse { }))
		}

		// punch failed -- proxy fallback
		
		// TODO		

		Ok(Response::new(ForwardJoinResponse { }))
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
