use std::{net::SocketAddr, sync::Arc, time::Duration};
use anyhow::Result;
use tokio::{net::UdpSocket, sync::{mpsc::{self}, RwLock}, time::{sleep, timeout}};
use tokio_stream::{wrappers::ReceiverStream};
use tonic::{transport::Channel, Request};
use uuid::Uuid;
use crate::{listing::{RustListing, RustListingNoId}, proto::{puncher_service_client::PuncherServiceClient, AddListingRequest, ClientStatus, CreateSessionRequest, EndSessionRequest, GetListingsRequest, JoinRequest, RemoveListingRequest}, ThreadSafe};

mod stream;
use stream::StreamHandler;

const TIMEOUT: Duration = Duration::from_secs(5);

pub struct Client {
	client: ThreadSafe<PuncherServiceClient<Channel>>,
	session_id: Uuid,
	stream_handler: Arc<StreamHandler>,
	
}

impl Client {
	pub fn uuid(&self) -> &Uuid { &self.session_id }

	pub fn id(&self) -> Vec<u8> { self.session_id.clone().into_bytes().to_vec() }

	pub fn inner(&self) -> &ThreadSafe<PuncherServiceClient<Channel>> { &self.client }

	pub fn cancel(&self) { self.stream_handler.stop() }

	pub async fn new(
		addr: SocketAddr, 
		server_addr: SocketAddr
	) -> Result<Self> {
		let client = create_threadsafe_client(server_addr).await?;
		
		let session_id = create_session(client.clone(), addr).await?;

		let stream_handler = start_session(client.clone(), &session_id, addr).await?;
		
		Ok(Self {
			client,
			session_id,
			stream_handler,
		})
	}

	pub async fn end(self) {
		let session_id = self.id();
		let _ = timeout(TIMEOUT,
			async {
				let mut client = self.inner().write().await;
				client.end_session(Request::new(EndSessionRequest {session_id})).await
			}
		).await;
		self.cancel();
	}

	pub async fn create_listing(&mut self, listing: RustListingNoId) -> Result<Uuid> {
		let session_id = self.id();
		
		let resp = timeout(TIMEOUT,
			async {
				let req = Request::new( AddListingRequest { 
					listing: Some(listing.into()), 
					session_id,
				});

				let mut client = self.inner().write().await;
				client.add_listing(req).await
			}
		).await??;

		let id = resp.into_inner().listing_id;
		Ok(Uuid::from_slice(&id[..16])?)
	}

	pub async fn remove_listing(&mut self) -> Result<()> {
		let session_id = self.id();
		timeout(TIMEOUT,
			async {
				let mut client = self.inner().write().await;
				client.remove_listing(Request::new(RemoveListingRequest { session_id })).await
			}
		).await??;
		
		Ok(())
	}

	pub async fn get_listings(&mut self) -> Result<Vec<RustListing>> {
		let resp = timeout(TIMEOUT,
			async {
				let mut client = self.inner().write().await;
				client.get_listings(Request::new( GetListingsRequest {})).await
			}
		).await??;

		Ok(resp
			.into_inner()
			.listings
			.into_iter()
			.filter_map(|l| RustListing::try_from(l).ok())
			.collect())
	}

	pub async fn join(&mut self, listing_id: Uuid) -> Result<()> {
		let req = Request::new(JoinRequest {
			session_id: self.id(),
			target_listing_id: listing_id.as_bytes().to_vec(),
		});

		timeout(TIMEOUT, 
			async { 
				self.inner().write().await.join(req).await
			}
		).await??;
		
		Ok(())
	}

	
}

async fn create_threadsafe_client(
	server_addr: SocketAddr,
) -> Result<ThreadSafe<PuncherServiceClient<Channel>>> {
	let uri = format!("https://{server_addr}").parse()?;
	let channel = Channel::builder(uri).connect().await?;

	Ok(Arc::new(RwLock::new(PuncherServiceClient::new(channel))))
}

async fn create_session(
	client: ThreadSafe<PuncherServiceClient<Channel>>, 
	addr: SocketAddr,
) -> Result<Uuid> {
	let req = Request::new(CreateSessionRequest {
		ip: addr.ip().to_string(),
		port: addr.port().into(),
	});
	
	let mut client = client.write().await;
	let response = timeout(TIMEOUT,
		client.create_session(req)
	).await??;

	let id = response.into_inner().session_id;
	Ok(Uuid::from_slice(&id[..16])?)
}

async fn start_session(
	client: ThreadSafe<PuncherServiceClient<Channel>>, 
	session_id: &Uuid,
	addr: SocketAddr,
) -> Result<Arc<StreamHandler>> {
	let (status_tx, status_rx) = mpsc::channel(8);
	let request = Request::new(ReceiverStream::new(status_rx));
	
	// initial ping with session_id must be send before
	status_tx.send(ClientStatus { session_id: Some(session_id.as_bytes().to_vec()), status: None }).await?;

	let response = {
		let mut c = client.write().await;
		timeout(TIMEOUT,
			c.stream_session(request)
		).await??
	};

	let stream_handler = StreamHandler::new(status_tx, addr);
	tokio::spawn(stream_handler.clone().start(response.into_inner()));

	Ok(stream_handler)
}

pub async fn punch_nat(target_addr: SocketAddr, bind_addr: SocketAddr) -> Result<()> {
	let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
	socket.connect(target_addr).await?;

	let packet = b"punch";
	let mut recv = [0u8; 5];
	
	tokio::select! {
		_ = async { 
			loop {
				if let Err(e) = socket.send(packet).await {
					eprintln!("Unable to send punching packet {e}");
				}
				
				sleep(Duration::from_millis(300)).await;
			}
		} => {},

		res = timeout(TIMEOUT, socket.recv_from(&mut recv)) => {
			res??;
		},
	};

	Ok(())
}
