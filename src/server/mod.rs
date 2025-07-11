use std::{collections::HashMap, net::SocketAddr, pin, sync::Arc};
use anyhow::{anyhow, Result};
use tokio::{join, sync::{mpsc::{self, Sender}, RwLock}, time::timeout};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming, transport::Server};
use tonic_web::GrpcWebLayer;
use uuid::Uuid;
use futures::Stream;
use crate::{proto::{client_stream_message::ClientStreamEnum, puncher_service_server::{PuncherService, PuncherServiceServer}, server_stream_message::ServerStreamEnum, AddListingRequest, AddListingResponse, ClientStreamMessage, GetListingsRequest, GetListingsResponse, JoinRequest, JoinResponse, Punch, PunchStatus, RemoveListingRequest, RemoveListingResponse, ServerStreamMessage}, TIMEOUT};

pub mod session;
use session::{Session, SessionRef};
pub mod listing;
use listing::RustListing;

pub async fn run(addr: SocketAddr) -> anyhow::Result<()> {
	let server = PuncherServer::default();

	let svc = PuncherServiceServer::new(server);
	Server::builder()
		.accept_http1(true)
		.layer(GrpcWebLayer::new())
		.add_service(svc)
		.serve(addr)
		.await?;
	
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

	fn cleanup_fut(&self, session_id: &Uuid)  -> impl Future<Output = ()> + Send + 'static {
		let sessions = self.sessions.clone();
		let id_map = self.id_map.clone();
		let session_id = session_id.clone();

		async move {
			let mut sessions = sessions.write().await;

			let Some(session) = sessions.remove(&session_id) else {
				eprintln!("When trying to remove session it didnt exist.");
				return;
			};
			
			let session = session.lock().await;

			if let Some(listing) = session.listing.as_ref() {
				let mut id_map = id_map.write().await;
				id_map.remove(listing.id());
			}
		}
	}
}


#[tonic::async_trait]
impl PuncherService for PuncherServer {
	type StreamSessionStream = pin::Pin<Box<dyn Stream<Item = Result<ServerStreamMessage, Status>> + Send + Sync + 'static>>;

	async fn add_listing( // ADD LISTING //
        &self,
        request: Request<AddListingRequest>,
    ) -> Result<Response<AddListingResponse>, Status> {
		println!("Add listing req: {}", Uuid::from_slice(&request.get_ref().session_id).unwrap_or(Uuid::max()));

		let request = request.into_inner();
		
		// validate session //
		let session_id = request
			.session_id
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let session = self
			.get(&session_id)
			.await
			.ok_or(Status::invalid_argument(format!("Invalid session_id")))?;
		

		// validate assignment //
		{
			let session = session.lock().await;
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

		let mut session = session.lock().await;
		session.listing = Some(listing);


		Ok(Response::new(AddListingResponse { listing_id }))
    }

    async fn remove_listing( // REMOVE LISTING //
        &self,
        request: Request<RemoveListingRequest>,
    ) -> Result<Response<RemoveListingResponse>, Status> {
		println!("Remove listing req: {}", Uuid::from_slice(&request.get_ref().session_id).unwrap_or(Uuid::max()));
		
		// validate session //
		let session_id = request.into_inner().session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid Uuid: {e}")))?;

		let session = self
			.get(&session_id)
			.await
			.ok_or(Status::invalid_argument(format!("Invalid session_id")))?;

		
		// assignment //
		let mut session = session.lock().await;

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

        let sessions = self.sessions.read().await;
		let mut listings = Vec::new();
		for (_, session) in sessions.iter() {
			let session = session.lock().await;
			if let Some(listing) = session.listing.as_ref() {
				listings.push(listing.clone().into());
			}
		}

		Ok(Response::new(GetListingsResponse { listings }))
    }

	async fn stream_session( // STREAM //
		&self,
		request: Request<Streaming<ClientStreamMessage>>,
	) -> Result<Response<Self::StreamSessionStream>, Status> {
		let session_id = Uuid::new_v4();

		println!("Stream session req: {session_id}");

		let addr = match request.remote_addr()  {
			Some(a) => Ok(a),
			None => {
				eprintln!("Unable to establish connection with client because remote_addr() returned none.");
				Err(Status::internal("Couldnt get client remote_addr"))
			}
		}?;

		let streaming_rx = request.into_inner();
		let (client_tx, client_rx) = mpsc::channel(32);

		let (server_tx, server_rx) = mpsc::channel(32);

		let session = Session::new_ref(addr, server_tx, client_rx);

		let cleanup = self.cleanup_fut(&session_id);

		tokio::spawn(handle_stream(
			streaming_rx, 
			client_tx, 
			cleanup,
		));

		let mut sessions = self.sessions.write().await;
		sessions.insert(session_id, session);

		let out_stream = Box::pin(ReceiverStream::new(server_rx)) as Self::StreamSessionStream;
		Ok(Response::new(out_stream))
	}

	async fn join( // JOIN //
		&self,
		request: Request<JoinRequest>
	) -> Result<Response<JoinResponse>, Status> {
		println!("Join session req: {}, {}", Uuid::from_slice(&request.get_ref().session_id).unwrap_or(Uuid::max()), Uuid::from_slice(&request.get_ref().target_listing_id).unwrap_or(Uuid::max()));

		let request = request.into_inner();

		// validate session //
		let session_id: Uuid = request.session_id.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid session Uuid: {e}")))?;

		let session = self
			.get(&session_id)
			.await
			.ok_or(Status::invalid_argument(format!("Invalid session id")))?;

		// validate target session //
		let target_listing_id: Uuid = request
			.target_listing_id
			.try_into()
			.map_err(|e| Status::invalid_argument(format!("Invalid listing Uuid: {e}")))?;
		
		let id_map = self.id_map.read().await;
		let target_session_id = id_map
			.get(&target_listing_id)
			.ok_or(Status::invalid_argument("Listing ID has no associated session."))?;

		let target_session = self
			.get(&target_session_id)
			.await
			.ok_or(Status::invalid_argument(format!("Invalid session id")))?;

		// send both clients punch orders //
		let target_addr = {
			let target_session = target_session.lock().await;
			target_session.addr().clone()
		};

		let addr = {
			let session = session.lock().await;
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
	let mut session = session.lock().await;
	let (tx, rx) = session.streams();

	let punch_order = Ok(ServerStreamMessage {
		session_id_assignment: None,
		server_stream_enum: Some(ServerStreamEnum::Punch(Punch {
			ip: addr.ip().to_string(),
			port: addr.port().into(),
		})),
	});

	timeout(TIMEOUT, tx.send(punch_order))
		.await
		.map_err(|e| anyhow!("Timeout sending punch order: {e}"))?
		.map_err(|e| anyhow!("Unable to send order: {e}"))?;

	let ClientStreamEnum::PunchStatus(status) = timeout(TIMEOUT, rx.recv())
		.await?
		.ok_or(anyhow!("Stream closed when receiving punch status"))?;

	Ok(status)
}


async fn handle_stream<Fut>(
	mut stream: Streaming<ClientStreamMessage>, 
	output: Sender<ClientStreamEnum>, 
	cleanup: Fut,
) 
where 
	Fut: Future<Output = ()> + Send + 'static,
{
	loop {
		match stream.message().await {
			Ok(opt) => {
				match opt {
					Some(msg) => {
						if let Some(msg_enum) = msg.client_stream_enum {
							if let Err(e) = output.send(msg_enum).await {
								eprintln!("Unable to forward streaming message: {e}; closing stream handler");
								break;
							};
						};
					},
					None => break,
				}
			},
			Err(e) => {
				eprintln!("Received stream error: {e}; continuing");
			},
		};
	}
	cleanup.await;
}