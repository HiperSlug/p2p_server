use std::{net::{SocketAddr, SocketAddrV4}, sync::Arc, time::Duration};
use anyhow::Result;
use godot::prelude::*;
use tokio::{sync::{mpsc::{self, Sender}, oneshot, RwLock}, task::spawn_local, time::sleep};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Channel, Request};
use uuid::Uuid;
use crate::{client::gdlisting::GDListing, puncher::{puncher_service_client::PuncherServiceClient, AddListingRequest, ClientStatus, CreateSessionRequest, EndSessionRequest, GetListingsRequest, JoinRequest, RemoveListingRequest}, server::listing::{Listing, ListingNoId}};

type ThreadSafe<T> = Arc<RwLock<T>>;

pub mod gdlisting;

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct PunchingClient {
	base: Base<RefCounted>,
	client: ThreadSafe<Option<Client>>,
	listings: ThreadSafe<Vec<GDListing>>
}

#[godot_api]
impl IRefCounted for PunchingClient {
	fn init(base: Base<RefCounted>) -> Self { Self {
		base,
		client: Arc::new(RwLock::new(None)),
		listings: Arc::new(RwLock::new(Vec::new())),
	}}
}

#[godot_api]
impl PunchingClient {
	#[func]
	pub fn connect(&self, server_ip: String, server_port: u16, ip: String, port: u16) {
		let server_ip = match server_ip.parse() {
			Ok(ip) => ip,
			Err(e) => {
				godot_error!("PuncherClient: Could not parse server_ip: {e}.");
				return;
			}
		};
		let server_addr = SocketAddr::V4(SocketAddrV4::new(server_ip, server_port));

		let ip = match ip.parse() {
			Ok(ip) => ip,
			Err(e) => {
				godot_error!("PuncherClient: Could not parse ip: {e}");
				return;
			}
		};
		let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));
		
		let client = self.client.clone();

		spawn_local(async move {
			let new_client = Client::new(addr, server_addr).await;
			let mut client = client.write().await;
			*client = Some(new_client);
		});
	}
	
	#[func]
	pub fn disconnect(&self) {
		let client = self.client.clone();
		
		spawn_local( async move {
			let mut client = client.write().await;
			let c = client.take();
			if let Some(c) = c {
				c.end().await;
			} else {
				godot_error!("Disconnected a non-connected Client.");
			}
		});
	}

	
	#[func]
	pub fn create_listing(&self/*, listing: GDListing*/) {
		let client = self.client.clone();

		spawn_local(async move {

		});
	}

	#[func]
	pub fn remove_listing(&self) {
		let client = self.client.clone();

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when removing listing.");
				return;
			};

			if let Err(e) = c.remove_listing().await {
				godot_error!("Couldnt remove listing: {e}");
			};
		});
	}
	
	#[func]
	pub fn get_listings(&self) {
		let client = self.client.clone();

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when removing listing.");
				return;
			};

			let listings = match c.get_listings().await {
				Ok(l) => l,
				Err(e) => {
					godot_error!("Couldnt get listings: {e}");
					return
				}
			};

			// GIVE GODOT THE LISTINGS.
		});
	}

	
	#[func]
	pub fn join_listing(&self, listing_id: GString) {

	}
}


pub struct Client {
	client: PuncherServiceClient<Channel>,
	session_id: Vec<u8>,
	stop_ping: oneshot::Sender<()>,
}

impl Client {
	pub fn uuid(&self) -> Uuid {Uuid::from_slice(&self.session_id[0..16]).expect("Invalid UUID bytes")}

	pub fn id(&self) -> &Vec<u8> {&self.session_id}

	pub fn inner_mut(&mut self) -> &mut PuncherServiceClient<Channel> {&mut self.client}

	pub async fn new(
		addr: SocketAddr, 
		server_addr: SocketAddr
	) -> Self {
		println!("Creating client");

		let mut client = match Self::create_client(server_addr).await {
			Ok(c) => c,
			Err(e) => {
				godot_error!("Unable to create client: {e}");
				panic!("Unable to create client: {e}");
			}
		};
		println!("Client connected");

		let request = CreateSessionRequest { 
			ip: addr.ip().to_string(), 
			port: addr.port().into(),
		};
		
		let session_id = match Self::create_session(&mut client, request).await {
			Ok(id) => id,
			Err(e) => {
				godot_error!("Unable to create session: {e}");
				panic!("Unable to create session: {e}");
			}
		};

		println!("Session created");

		let stop_ping = match Self::start_session(&mut client, &session_id).await {
			Ok(s) => s,
			Err(e) => {
				godot_error!("Unable to start session: {e}");
				panic!("Unable to start session: {e}");
			}
		};

		println!("Session started");

		Self {
			client,
			session_id,
			stop_ping,
		}
	}

	async fn create_client(
		server_addr: SocketAddr
	) -> Result<PuncherServiceClient<Channel>> {
		let uri = format!("https://{server_addr}").parse()?;
		let channel = Channel::builder(uri).connect().await?;

		Ok(PuncherServiceClient::new(channel))
	}

	async fn create_session(
		client: &mut PuncherServiceClient<Channel>, 
		request: CreateSessionRequest
	) -> Result<Vec<u8>> {
		let response = client.create_session(Request::new(request)).await?;
		let response = response.into_inner();

		Ok(response.session_id)
	}

	async fn start_session(
		client: &mut PuncherServiceClient<Channel>, 
		session_id: &Vec<u8>,
	) -> Result<oneshot::Sender<()>> {
		// start stream //
		let (ping_tx, ping_rx) = mpsc::channel(8);
		let request = Request::new(ReceiverStream::new(ping_rx));
		
		ping_tx.send(ClientStatus { session_id: Some(session_id.clone()), status: None }).await?;

		let response = client.stream_session(request).await?;
		let mut stream = response.into_inner();

		// pings //
		let (stop_tx, stop_rx) = oneshot::channel();

		tokio::spawn(ping(ping_tx, stop_rx));
	
		// orders //
		tokio::spawn(async move {
			while let Ok(Some(_)) = stream.message().await {}
		});

		Ok(stop_tx)
	}

	pub async fn end(mut self) {
		let session_id = self.session_id.clone();
		let _ = self.inner_mut().end_session(Request::new(EndSessionRequest {session_id})).await;
		let _ = self.stop_ping.send(());
	}

	pub async fn create_listing(&mut self, listing: ListingNoId) -> Result<()> {
		let session_id = self.session_id.clone();
		self.inner_mut().add_listing(Request::new( AddListingRequest { listing: Some(listing.into()), session_id } )).await?;
		Ok(())
	}

	pub async fn remove_listing(&mut self) -> Result<()> {
		let session_id = self.session_id.clone();
		self.inner_mut().remove_listing(Request::new(RemoveListingRequest { session_id })).await?;
		Ok(())
	}

	pub async fn get_listings(&mut self) -> Result<Vec<Listing>> {
		let resp = self.inner_mut().get_listings(Request::new( GetListingsRequest {})).await?;
		let resp = resp.into_inner();
		Ok(resp
			.listings
			.into_iter()
			.filter_map(|l| Listing::try_from(l).ok())
			.collect())
	}

	pub async fn join(&mut self, listing_id: Uuid) -> Result<()> {
		let req = Request::new(JoinRequest {
			session_id: self.session_id.clone(),
			target_listing_id: listing_id.as_bytes().to_vec(),
		});

		self.inner_mut().join(req).await?;
		
		Ok(())
	}
}


async fn ping(sender: Sender<ClientStatus>, mut stopper: oneshot::Receiver<()>) {
	loop {
		tokio::select! {
			_ = sleep(Duration::from_secs(60)) => {
				let _ = sender.send(ClientStatus { session_id: None, status: None }).await;
			}
			_ = &mut stopper => {
			}
		}
	}
}
