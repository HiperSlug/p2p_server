use std::{net::SocketAddr, sync::Arc, time::Duration};
use anyhow::{anyhow, Result};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{client::legacy::{self, connect::HttpConnector}, rt::TokioExecutor};
use tokio::{net::UdpSocket, sync::{broadcast, mpsc, RwLock}, time::{sleep, timeout}};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{body::Body, transport::Uri, Request};
use tonic_web::{GrpcWebCall, GrpcWebClientLayer, GrpcWebClientService};
use uuid::Uuid;
use crate::{proto::{puncher_service_client::PuncherServiceClient, AddListingRequest, GetListingsRequest, JoinRequest, RemoveListingRequest}, server::listing::{RustListing, RustListingNoId}, ThreadSafe, TIMEOUT};

mod session;
use session::Session;

type WebClient = PuncherServiceClient<GrpcWebClientService<legacy::Client<HttpsConnector<HttpConnector>, GrpcWebCall<Body>>>>;

pub struct Client {
	client: ThreadSafe<WebClient>,
	session: Option<Session>,
}

impl Client {
	pub fn inner(&self) -> &ThreadSafe<WebClient> { &self.client }

	pub fn session(&self) -> &Option<Session> { &self.session }

	pub fn end_session(&mut self) {	
		if let Some(s) = self.session.take() {
			s.end();
		}
	}

	pub async fn new(server_url: Uri) -> Result<Self> {
		let https = HttpsConnectorBuilder::new()
			.with_native_roots()
			.map_err(|e| anyhow!("With native roots error: {e}"))?
			.https_or_http()
			.enable_http1()
			.build();

		let client = hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build(https);

		let svc = tower::ServiceBuilder::new()
			.layer(GrpcWebClientLayer::new())
			.service(client);

		let client = PuncherServiceClient::with_origin(svc, server_url);
		let client = Arc::new(RwLock::new(client));

		Ok(Self {
			client,
			session: None,
		})
	}

	pub async fn start_session(&mut self) -> Result<broadcast::Receiver<SocketAddr>> {
		let (client_tx, client_rx) = mpsc::channel(8);

		let req = Request::new(ReceiverStream::new(client_rx));

		let resp = {
			let mut client = self.inner().write().await;
			client.stream_session(req).await
				.map_err(|e| anyhow!("Stream session status: {e}"))?
		};

		let mut server_rx = resp.into_inner();

		let session_id: Uuid = timeout(TIMEOUT, server_rx.message()).await
			.map_err(|e| anyhow!("Timeout waiting for session_id: {e}"))?
			.map_err(|e| anyhow!("Received grpc error waiting for session_id: {e}"))?
			.ok_or(anyhow!("First received message had no contents"))?
			.session_id_assignment
			.ok_or(anyhow!("First received message had no session_id"))?
			.try_into()
			.map_err(|e| anyhow!("Unable to convert received Vec<u8> to Uuid: {e}"))?;

		let (session, joined_dst) = Session::start(session_id, server_rx, client_tx)
			.await
			.map_err(|e| anyhow!("Session creation error: {e}"))?;

		self.session = Some(session);

		Ok(joined_dst)
	}

	pub async fn create_listing(&mut self, listing: RustListingNoId) -> Result<Uuid> {
		let session_id = self
			.session()
			.as_ref()
			.ok_or(anyhow!("Cannot create listing without a session"))?
			.id();
		
		let req = Request::new( AddListingRequest { 
			listing: Some(listing.into()), 
			session_id,
		});

		let fut = async {
			let mut client = self.inner().write().await;
			client.add_listing(req).await
		};

		let resp = timeout(TIMEOUT, fut)
			.await
			.map_err(|e| anyhow!("Add listing timeout: {e}"))?
			.map_err(|e| anyhow!("Add listing error status: {e}"))?;

		let listing_id: Uuid = resp
			.into_inner()
			.listing_id
			.try_into()
			.map_err(|e| anyhow!("Received bad listing_id from server: {e}"))?;

		Ok(listing_id)
	}

	pub async fn remove_listing(&mut self) -> Result<()> {
		let session_id = self
			.session()
			.as_ref()
			.ok_or(anyhow!("Cannot remove listing without a session"))?
			.id();
		
		let req = Request::new( RemoveListingRequest { 
			session_id,
		});

		let fut = async {
			let mut client = self.inner().write().await;
			client.remove_listing(req).await
		};

		timeout(TIMEOUT, fut)
			.await
			.map_err(|e| anyhow!("Remove listing timeout: {e}"))?
			.map_err(|e| anyhow!("Remove listing error status: {e}"))?;

		Ok(())
	}

	pub async fn get_listings(&mut self) -> Result<Vec<RustListing>> {
		let req = Request::new( GetListingsRequest { });

		let fut = async {
			let mut client = self.inner().write().await;
			client.get_listings(req).await
		};

		let resp = timeout(TIMEOUT, fut)
			.await
			.map_err(|e| anyhow!("Get listings timeout: {e}"))?
			.map_err(|e| anyhow!("Get listings error status: {e}"))?;

		let listings: Vec<RustListing> = resp
			.into_inner()
			.listings
			.into_iter()
			.filter_map(|l| l.try_into().ok())
			.collect();

		Ok(listings)
	}

	pub async fn join(&mut self, listing_id: Uuid) -> Result<()> {
		let session_id = self
			.session()
			.as_ref()
			.ok_or(anyhow!("Cannot join listing without a session"))?
			.id();
		
		let req = Request::new( JoinRequest { 
			session_id,
			target_listing_id: listing_id.into_bytes().to_vec(),
		});

		let fut = async {
			let mut client = self.inner().write().await;
			client.join(req).await
		};

		timeout(TIMEOUT, fut)
			.await
			.map_err(|e| anyhow!("Join listing timeout: {e}"))?
			.map_err(|e| anyhow!("Join listing error status: {e}"))?;

		Ok(())
	}
}


pub async fn punch(addr: SocketAddr) -> Result<()> {
	let socket = Arc::new(UdpSocket::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap()).await?);
	socket.connect(addr).await?;

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

		result = timeout(TIMEOUT, socket.recv(&mut recv)) => {
			result
				.map_err(|e| anyhow!("Punch timeout: {e}"))?
				.map_err(|e| anyhow!("Error receiving punch packets: {e}"))?;
		},
	};

	Ok(())
}
