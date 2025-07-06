use std::{net::{Ipv4Addr, SocketAddr, SocketAddrV4}, sync::Arc, time::Duration};
use anyhow::Result;
use futures::StreamExt;
use godot::prelude::*;
use tokio::{net::UdpSocket, sync::{mpsc::{self, Sender}, RwLock, RwLockWriteGuard}, task::spawn_local, time::sleep};
use tokio_stream::{wrappers::ReceiverStream};
use tokio_util::sync::CancellationToken;
use tonic::{transport::Channel, Request, Status, Streaming};
use uuid::Uuid;
use crate::{client::godotlisting::{GdListing, GdListingNoId}, puncher::{order::Order as OrderEnum, puncher_service_client::PuncherServiceClient, AddListingRequest, ClientStatus, CreateSessionRequest, EndSessionRequest, GetListingsRequest, JoinRequest, Order, RemoveListingRequest}, server::listing::{Listing, ListingNoId}};

type ThreadSafe<T> = Arc<RwLock<T>>;

pub mod godotlisting;

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct PunchingClient {
	base: Base<RefCounted>,
	client: ThreadSafe<Option<Client>>,
	listings: ThreadSafe<Option<Array<Gd<GdListing>>>>,
}

#[godot_api]
impl IRefCounted for PunchingClient {
	fn init(base: Base<RefCounted>) -> Self { Self {
		base,
		client: Arc::new(RwLock::new(None)),
		listings: Arc::new(RwLock::new(None)),
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
	pub fn create_listing(&self, listing: Gd<GdListingNoId>) {
		let client = self.client.clone();

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when creating listing.");
				return;
			};

			if let Err(e) = c.create_listing(ListingNoId::from(listing)).await {
				godot_error!("Couldnt create listing: {e}.");
			};
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
		let client_listings = self.listings.clone();

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when getting listings.");
				return;
			};

			let listings = match c.get_listings().await {
				Ok(l) => l,
				Err(e) => {
					godot_error!("Couldnt get listings: {e}");
					return
				}
			};

			let gd_listings: Array<Gd<GdListing>> = listings
				.into_iter()
				.map(|l| Gd::from_object(GdListing::from(l)))
				.collect();

			let mut listings = client_listings.write().await;
			*listings = Some(gd_listings);
		});
	}

	
	#[func]
	pub fn join_listing(&self, listing_id: String) {
		let client = self.client.clone();
		let listing_id = match listing_id.parse() {
			Ok(id) => id,
			Err(e) => {
				godot_error!("Couldnt join listing: {e}");
				return
			}
		};

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when getting listings.");
				return;
			};

			if let Err(e) = c.join(listing_id).await {
				godot_error!("Error while joining listing: {e}");
				return;
			};
		});
	}
}


pub struct Client {
	client: ThreadSafe<PuncherServiceClient<Channel>>,
	session_id: Vec<u8>,
	cancel: CancellationToken,
}

impl Client {
	pub fn uuid(&self) -> Uuid {Uuid::from_slice(&self.session_id[0..16]).expect("Invalid UUID bytes")}

	pub fn id(&self) -> &Vec<u8> {&self.session_id}

	pub async fn inner_mut(&self) -> RwLockWriteGuard<'_, PuncherServiceClient<Channel>> {self.client.write().await}

	pub async fn new(
		addr: SocketAddr, 
		server_addr: SocketAddr
	) -> Self { 
		println!("Creating client");

		let client = match Self::create_client(server_addr).await {
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
		
		let session_id = match Self::create_session(client.clone(), request).await {
			Ok(id) => id,
			Err(e) => {
				godot_error!("Unable to create session: {e}");
				panic!("Unable to create session: {e}");
			}
		};

		println!("Session created");

		let cancel = match Self::start_session(client.clone(), &session_id).await {
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
			cancel,
		}
	}

	async fn create_client(
		server_addr: SocketAddr
	) -> Result<ThreadSafe<PuncherServiceClient<Channel>>> {
		let uri = format!("https://{server_addr}").parse()?;
		let channel = Channel::builder(uri).connect().await?;

		Ok(Arc::new(RwLock::new(PuncherServiceClient::new(channel))))
	}

	async fn create_session(
		client: ThreadSafe<PuncherServiceClient<Channel>>, 
		request: CreateSessionRequest
	) -> Result<Vec<u8>> {
		let mut client = client.write().await;

		let response = client.create_session(Request::new(request)).await?;
		let response = response.into_inner();

		Ok(response.session_id)
	}

	async fn start_session(
		client: ThreadSafe<PuncherServiceClient<Channel>>, 
		session_id: &Vec<u8>,
	) -> Result<CancellationToken> {
		// start stream //
		let (status_tx, status_rx) = mpsc::channel(8);
		let request = Request::new(ReceiverStream::new(status_rx));
		
		status_tx.send(ClientStatus { session_id: Some(session_id.clone()), status: None }).await?;

		let response = {
			let mut c = client.write().await;
			c.stream_session(request).await?
		};
		let order_stream = response.into_inner();

		let cancel = CancellationToken::new();
		
		// pings //
		tokio::spawn(ping(status_tx.clone(), cancel.clone()));
	
		// orders //
		tokio::spawn(follow_orders(client, order_stream, status_tx, cancel.clone()));

		Ok(cancel)
	}

	pub async fn end(self) {
		let session_id = self.session_id.clone();
		let _ = self.inner_mut().await.end_session(Request::new(EndSessionRequest {session_id})).await;
		self.cancel.cancel();
	}

	pub async fn create_listing(&mut self, listing: ListingNoId) -> Result<()> {
		let session_id = self.session_id.clone();
		self.inner_mut().await.add_listing(Request::new( AddListingRequest { listing: Some(listing.into()), session_id } )).await?;
		Ok(())
	}

	pub async fn remove_listing(&mut self) -> Result<()> {
		let session_id = self.session_id.clone();
		self.inner_mut().await.remove_listing(Request::new(RemoveListingRequest { session_id })).await?;
		Ok(())
	}

	pub async fn get_listings(&mut self) -> Result<Vec<Listing>> {
		let resp = self.inner_mut().await.get_listings(Request::new( GetListingsRequest {})).await?;
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

		self.inner_mut().await.join(req).await?;
		
		Ok(())
	}
}


async fn ping(sender: Sender<ClientStatus>, cancel: CancellationToken) {
	loop {
		tokio::select! {
			_ = sleep(Duration::from_secs(60)) => {
				let _ = sender.send(ClientStatus { session_id: None, status: None }).await;
			}
			_ = cancel.cancelled() => {
				break;
			}
		}
	}
}

async fn handle_orders(client: ThreadSafe<PuncherServiceClient<Channel>>, mut order_stream: Streaming<Order>, status_tx: mpsc::Sender<ClientStatus>, cancel: CancellationToken) {
	loop {
		tokio::select! {
			msg = order_stream.next() => {
				receive_order(msg, status_tx, client).await;
			}
			_ = cancel.cancelled() => {
				break;
			}
		}
	}
}

async fn receive_order(msg: Option<Result<Order, Status>>, status_tx: mpsc::Sender<ClientStatus>, client: ThreadSafe<PuncherServiceClient<Channel>>) {
	if let Some(msg) = msg {
		match msg {
			Ok(order) => {
				if let Some(order) = order.order {
					match order {
						OrderEnum::Punch(pu) => {
							let ip = match pu.ip.parse::<Ipv4Addr>() {
								Ok(ip) => ip,
								Err(e) => {
									godot_error!("Received bad ip from server: {e}");
									return;
								},
							};
							let port = match pu.port.try_into() {
								Ok(port) => port,
								Err(e) => {
									godot_error!("Received bad port from server: {e}");
									return;
								}
							};
							let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));

							// send udp packets to addr.
						},
						OrderEnum::Proxy(_) => {},
					}
				}

			}
			Err(e) => {
				godot_error!("Received error order: {e}")
			}
		}
	}
	//  else {
	// 	break;
	// }
}

pub async fn punch_nat(target_addr: SocketAddr, bind_addr: SocketAddr) -> Result<()> {
	let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
	socket.connect(target_addr).await?;

	let packet = b"punch";
	let mut recv = [0u8; 1];

	let cancel = CancellationToken::new();

	let c = cancel.clone();
	let s = socket.clone();
	tokio::spawn(async move {
		loop {
			tokio::select! {
				Err(e) = s.send(packet) => {
					godot_error!("Unable to send packet {e}");
				}
				_ = c.cancelled() => {
					break;
				}
			}
		}
	});

	tokio::select! {
		_ = socket.recv(&mut recv) => {
			cancel.cancel();
			return Ok(());
		}
		_ = sleep(Duration::from_secs(60)) => {
			cancel.cancel();
			anyhow::bail!("Timeout.");
		}
	};
}