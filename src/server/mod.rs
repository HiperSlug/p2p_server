use std::{collections::HashMap, net::{SocketAddr, SocketAddrV4}, pin, sync::Arc, time::Duration};
use anyhow::{anyhow, bail, Result};
use rand::random;
use tokio::{join, sync::{mpsc, oneshot, RwLock}, time::timeout};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming, transport::Server};
use uuid::Uuid;
use futures::{future::join_all, Stream, StreamExt};
use crate::{listing::RustListing, proto::{client_status::Status as ClientStatusEnum, order::Order as OrderEnum, puncher_service_server::{PuncherService, PuncherServiceServer}, AddListingRequest, AddListingResponse, ClientStatus, CreateSessionRequest, CreateSessionResponse, EndSessionRequest, EndSessionResponse, GetListingsRequest, GetListingsResponse, JoinRequest, JoinResponse, Order as OrderMessage, Punch, PunchStatus, RemoveListingRequest, RemoveListingResponse}};

pub mod session;
use session::{Session, SessionRef};

pub async fn run(addr: SocketAddr) -> anyhow::Result<()> {
	let server = PuncherServer::default();

	Server::builder()
		.add_service(PuncherServiceServer::new(server))
		.serve(addr)
		.await?;
	
	Ok(())
}

pub async fn run_signal(addr: SocketAddr, ready_tx: oneshot::Sender<()>) -> anyhow::Result<()> {
	let server = PuncherServer::default();

	let svc = Server::builder()
		.add_service(PuncherServiceServer::new(server));
	
	let fut =	svc.serve(addr);

	let _ = ready_tx.send(());
	fut.await?;
	
	Ok(())
}

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
			.ok_or(anyhow!("Not found."))?;

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
				id_map.remove(listing.id());
			}
		}
	}

	async fn remove_timed_out(&self) {
		let expired = {
			let sessions = self.sessions.read().await;
			let mut expired = Vec::new();
			for s in sessions.values().cloned() {
				expired.push(async move {
					let session = s.read().await;
					if session.is_timed_out() {
						Some(session.id().clone())
					} else {
						None
					}
				})
			}
			expired
		};
		
		let e: Vec<Uuid> = join_all(expired)
			.await
			.into_iter()
			.flatten()
			.collect();

		self.remove_deep(&e).await;
	}

	pub async fn remove_timed_out_chance(&self) {
		if random::<f32>() < 0.025 {
			self.remove_timed_out().await;
		}
	}
}


#[tonic::async_trait]
impl PuncherService for PuncherServer {
	type StreamSessionStream = pin::Pin<Box<dyn Stream<Item = Result<OrderMessage, Status>> + Send + Sync + 'static>>;

	// -- listings -- //
	async fn add_listing( // ADD LISTING //
        &self,
        request: Request<AddListingRequest>,
    ) -> Result<Response<AddListingResponse>, Status> {
		println!("Add listing req");
		self.remove_timed_out_chance().await;

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
		let listing = RustListing::new(listing_no_id_packet);
		

		// assign listing //
		let mut id_map = self.id_map.write().await;
		id_map.insert(*listing.id(), session_id);

		let listing_id = listing.id().as_bytes().to_vec();

		let mut session = session.write().await;
		session.listing = Some(listing);


		Ok(Response::new(AddListingResponse { listing_id }))
    }

    async fn remove_listing( // REMOVE LISTING //
        &self,
        request: Request<RemoveListingRequest>,
    ) -> Result<Response<RemoveListingResponse>, Status> {
		println!("Remove listing req");
		self.remove_timed_out_chance().await;
		
		// validate session //
		let session_id = request.into_inner().session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let session = self
			.validate(&session_id)
			.await
			.map_err(|e| Status::invalid_argument(format!("Invalid session_id: {e}")))?;

		
		// assignment //
		let mut session = session.write().await;

		if let Some(listing) = session.listing.as_ref() {
			let mut id_map = self.id_map.write().await;
			id_map.remove(listing.id());
			
			session.listing = None;
		}

		Ok(Response::new(RemoveListingResponse {}))
    }

    async fn get_listings( // GET LISTINGS //
        &self,
        _: Request<GetListingsRequest>,
    ) -> Result<Response<GetListingsResponse>, Status> {
		println!("Get listing req");

		self.remove_timed_out_chance().await;


        let sessions = self.sessions.read().await;
		let mut listings = Vec::new();
		for (_, session) in sessions.iter() {
			let session = session.read().await;
			if let Some(listing) = session.listing.as_ref() {
				listings.push(listing.clone().into());
			}
		}

		Ok(Response::new(GetListingsResponse { listings }))
    }

	// -- connection -- //
	async fn create_session( // CREATE SESSION //
		&self,
		request: Request<CreateSessionRequest>,
	) -> Result<Response<CreateSessionResponse>, Status> {
		println!("Create session req");
		
		self.remove_timed_out_chance().await;

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
		let session = Session::new_ref(addr);
		
		let session_id = {
			let session = session.read().await;
			session.id().clone()
		};

		let session_id_bytes = session_id.as_bytes().to_vec();

		let mut sessions = self.sessions.write().await;
		sessions.insert(session_id, session);
		
		Ok(Response::new(CreateSessionResponse { session_id: session_id_bytes }))
	}

	async fn stream_session(
		&self,
		request: Request<Streaming<ClientStatus>>,
	) -> Result<Response<Self::StreamSessionStream>, Status> {
		println!("Stream session req");

		let mut request = request.into_inner();

		let session_id = timeout(Duration::from_secs(5), request.message())
			.await
			.map_err(|e| Status::invalid_argument(format!("First ping timeout: {e}")))??
			.ok_or(Status::invalid_argument("Stream Closed."))?
			.session_id
			.ok_or(Status::invalid_argument("No session_id in first ping."))?
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;
		
		let session = self
			.get(&session_id)
			.await
			.ok_or(Status::invalid_argument("Session ID points to nothing."))?;

		let (order_tx, order_rx) = mpsc::channel(32);

		let (status_tx, status_rx) = mpsc::channel(8);
		
		// handle ping then forward status to status rx
		let s = session.clone();
		tokio::spawn(async move {
			while let Some(msg) = request.next().await {
				if let Ok(status) = msg {
					if let Some(status) = status.status {
						if let Err(e) = status_tx.send(status).await {
							eprintln!("Couldnt forward status: {e}");
							break;
						};
					}

					let mut session = s.write().await;
					session.see();
				} else {
					break;
				}
			}
		});

		let mut session = session.write().await;
		session.streams = Some((order_tx, status_rx));

		let stream = Box::pin(ReceiverStream::new(order_rx)) as Self::StreamSessionStream;
		Ok(Response::new(stream))
	}

	async fn end_session(
		&self,
		request: Request<EndSessionRequest>,
	) -> Result<Response<EndSessionResponse>, Status> {
		println!("End session req");

		self.remove_timed_out_chance().await;

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
		println!("Join session req");

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
			.ok_or(Status::invalid_argument("Listing ID has no associated session."))?;

		let target_session = self
			.validate(&target_session_id)
			.await
			.map_err(|e| Status::invalid_argument(format!("Invalid session id: {e}")))?;

		// send both clients punch orders //

		let target_addr = {
			let target_session = target_session.write().await;
			target_session.addr().clone()
		};

		let addr = {
			let session = session.write().await;
			session.addr().clone()
		};


		let (resp, target_resp) = join!(order_punch(session, target_addr), order_punch(target_session, addr));


		let resp = match resp {
			Ok(r) => r,
			Err(e) => {
				eprintln!("Error while  trying to punch: {e}");
				PunchStatus{ message: None, success: false}
			},
		};
		let target_resp = match target_resp {
			Ok(r) => r,
			Err(e) => {
				eprintln!("Error while  trying to punch: {e}");
				PunchStatus{ message: None, success: false}
			},
		};

		if resp.success && target_resp.success {
			println!("Punch success.");
			return Ok(Response::new(JoinResponse { }));
		} else {
			if let Some(msg) = resp.message {
				eprintln!("Punch failure message(joiner): {msg}")
			}
			if let Some(msg) = target_resp.message {
				eprintln!("Punch failure message(target): {msg}")
			}
		}

		// TODO handle Proxy fallback		

		Ok(Response::new(JoinResponse { }))
	}
}

async fn order_punch(session: SessionRef, addr: SocketAddr) -> Result<PunchStatus> {
	let mut session = session.write().await;
	let (tx, rx) = session.streams.as_mut().ok_or(anyhow!("No stream found on a validated session."))?;

	let order = Ok(OrderMessage {
		order: Some(OrderEnum::Punch(Punch {
			ip: addr.ip().to_string(),
			port: addr.port().into(),
		})),
	});

	tx.send(order).await.map_err(|e| anyhow!("Unable to send order: {e}"))?;
	let ClientStatusEnum::PunchStatus(s) = timeout(Duration::from_secs(5), rx.recv()).await?.ok_or(anyhow!("Channel closed."))? /* else { bail!("Incorrect status type received.") } */;

	Ok(s)
}