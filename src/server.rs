use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use crate::puncher::{puncher_service_client::PuncherServiceClient, puncher_service_server::{PuncherService, PuncherServiceServer}, AddListingRequest, AddListingResponse, CreateSessionRequest, CreateSessionResponse, EndSessionRequest, EndSessionResponse, GetListingsRequest, GetListingsResponse, PingRequest, PingResponse, RemoveListingRequest, RemoveListingResponse};
use crate::puncher::Listing as ListingPacket;
use crate::puncher::ListingNoId as ListingNoIdPacket;
use anyhow::{anyhow, bail, Result};
use rand::random;
use tokio::{sync::RwLock, time::sleep};
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

// -- Session -- //
struct Session {
	id: Uuid,
	last_seen: Instant,
	listing: Option<Listing>,
}

impl Session {
	const TIMEOUT: Duration = Duration::from_secs(30);

	pub fn new() -> Self {
		Self {
			id: Uuid::new_v4(),
			last_seen: Instant::now(),
			listing: None,
		}
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

	fn to_packet(&self) -> ListingPacket {
		ListingPacket {
			listing_no_id: Some(self.listing_no_id.to_packet()),
			id: self.id.to_string(),
		}
	}
}

struct ListingNoId {
	name: String, 
}

impl ListingNoId {
	fn to_packet(&self) -> ListingNoIdPacket {
		ListingNoIdPacket {
			name: self.name.clone(),
		}
	}

	fn from_packet(listing_no_id_packet: ListingNoIdPacket) -> ListingNoId {
		ListingNoId {
			name: listing_no_id_packet.name,
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
		let session_id = request.session_id.parse::<Uuid>()
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
		let listing = Listing::new(ListingNoId::from_packet(listing_no_id_packet));
		
		let mut id_map = self.id_map.write().await;
		id_map.insert(listing.id, session_id);

		let listing_id = listing.id.to_string();

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
		let session_id = request.session_id.parse::<Uuid>()
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
			.filter_map(|(_, s)| s.listing.as_ref().map(|l| l.to_packet()))
			.collect::<Vec<ListingPacket>>();
		Ok(Response::new(GetListingsResponse { listings }))
    }

	// -- connection -- //
	async fn create_session(
		&self,
		_: Request<CreateSessionRequest>,
	) -> Result<Response<CreateSessionResponse>, Status> {
		let session = Session::new();
		
		let session_id = session.id.to_string();

		let mut sessions = self.sessions.write().await;
		sessions.insert(session.id, session);
		
		Ok(Response::new(CreateSessionResponse {session_id}))
	}

	async fn end_session(
		&self,
		request: Request<EndSessionRequest>,
	) -> Result<Response<EndSessionResponse>, Status> {
		let request = request.into_inner();

		let session_id = request.session_id.parse::<Uuid>()
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
			.parse::<Uuid>()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let mut sessions = self.sessions.write().await;
		let session = sessions.get_mut(&session_id).ok_or(Status::invalid_argument("Session ID expired or non-existent."))?;

		session.see();

		Ok(Response::new(PingResponse {}))
	}
}

pub async fn run() -> anyhow::Result<()> {
    dummy_client();

	let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
	let server = PuncherServer::default();

	Server::builder()
		.add_service(PuncherServiceServer::new(server))
		.serve(addr)
		.await?;
	
	Ok(())
}

fn dummy_client() {
	tokio::spawn(async move {
        let mut client = PuncherServiceClient::connect("https://localhost:3000").await.unwrap();
        
		let session_id =client
			.create_session(CreateSessionRequest {})
			.await
			.unwrap()
			.into_inner()
			.session_id;
        
        loop {
            sleep(Duration::from_secs(1)).await;
            let request: Request<AddListingRequest> = Request::new(AddListingRequest {
				session_id: session_id.clone(),
                listing: Some(ListingNoIdPacket {
					name: "Raphael".to_string(),
				}),
            });

            match client.add_listing(request).await {
				Ok(resp) => println!("Response = {resp:?}"),
				Err(e) => eprintln!("Err: {e}")
			};
        }
    });
} 