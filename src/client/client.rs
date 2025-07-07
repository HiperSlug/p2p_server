use std::{net::{Ipv4Addr, SocketAddr, SocketAddrV4}, sync::Arc, time::Duration};
use anyhow::{bail, Result};
use futures::StreamExt;
use godot::prelude::*;
use tokio::{net::UdpSocket, sync::{mpsc::{self, Sender}, RwLock, RwLockWriteGuard}, time::{sleep, timeout}};
use tokio_stream::{wrappers::ReceiverStream};
use tokio_util::sync::CancellationToken;
use tonic::{transport::Channel, Request, Streaming};
use uuid::Uuid;
use crate::{client::ThreadSafe, puncher::{client_status::Status as CStatus, order::Order as OrderEnum, puncher_service_client::PuncherServiceClient, AddListingRequest, ClientStatus, CreateSessionRequest, EndSessionRequest, GetListingsRequest, JoinRequest, Order, PunchStatus, RemoveListingRequest}, server::listing::{Listing, ListingNoId}};


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

		let cancel = match Self::start_session(client.clone(), &session_id, addr).await {
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

		let response = timeout(
			Duration::from_secs(5),
			client.create_session(Request::new(request)),// this keeps getting timed out.
		).await??;
		let response = response.into_inner();

		Ok(response.session_id)
	}

	async fn start_session(
		client: ThreadSafe<PuncherServiceClient<Channel>>, 
		session_id: &Vec<u8>,
		addr: SocketAddr,
	) -> Result<CancellationToken> {
		// start stream //
		let (status_tx, status_rx) = mpsc::channel(8);
		let request = Request::new(ReceiverStream::new(status_rx));
		
		status_tx.send(ClientStatus { session_id: Some(session_id.clone()), status: None }).await?;

		let response = {
			let mut c = client.write().await;
			timeout(
				Duration::from_secs(5), 
				c.stream_session(request),
			).await??
		};

		let order_stream = response.into_inner();

		let cancel = CancellationToken::new();
		
		// pings //
		tokio::spawn(ping(status_tx.clone(), cancel.clone()));
	
		// orders //
		tokio::spawn(handle_orders(order_stream, status_tx, cancel.clone(), addr, session_id.clone()));

		Ok(cancel)
	}

	pub async fn end(self) {
		let session_id = self.session_id.clone();
		let _ = timeout(
			Duration::from_secs(5),
			self.inner_mut().await.end_session(Request::new(EndSessionRequest {session_id})),
		).await;
		self.cancel.cancel();
	}

	pub async fn create_listing(&mut self, listing: ListingNoId) -> Result<()> {
		let session_id = self.session_id.clone();
		let _ = timeout(
			Duration::from_secs(5),
			self.inner_mut().await.add_listing(Request::new( AddListingRequest { listing: Some(listing.into()), session_id } )),
		).await??;
		Ok(())
	}

	pub async fn remove_listing(&mut self) -> Result<()> {
		let session_id = self.session_id.clone();
		let _ = timeout(
			Duration::from_secs(5), 
			self.inner_mut().await.remove_listing(Request::new(RemoveListingRequest { session_id })),
		).await??;
		
		Ok(())
	}

	pub async fn get_listings(&mut self) -> Result<Vec<Listing>> {
		let resp = timeout(
			Duration::from_secs(20),
			self.inner_mut().await.get_listings(Request::new( GetListingsRequest {})),
		).await??;
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

		let _ = timeout(
			Duration::from_secs(5), 
			self.inner_mut().await.join(req),
		).await??;
		
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

pub async fn punch_nat(target_addr: SocketAddr, bind_addr: SocketAddr) -> Result<()> {
	let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
	socket.connect(target_addr).await?;

	let packet = b"punch";
	let mut recv = [0u8; 5];

	let cancel = CancellationToken::new();

	let c = cancel.clone();
	let s = socket.clone();
	tokio::spawn(async move {
		loop {
			if c.is_cancelled() {
				break;
			}

			if let Err(e) = s.send(packet).await {
				godot_error!("Unable to send packet {e}");
				panic!("Unable to send packet {e}");
			}
			
			sleep(Duration::from_millis(300)).await;
		}
	});

	let _ = timeout(
		Duration::from_secs(3), 
		socket.recv_from(&mut recv),
	).await??; 
	cancel.cancel();

	Ok(())
}


async fn handle_orders(mut order_stream: Streaming<Order>, status_tx: mpsc::Sender<ClientStatus>, cancel: CancellationToken, addr: SocketAddr, session_id: Vec<u8>) {
	loop {
		tokio::select! {
			msg = order_stream.next() => {
				println!("/c received order_msg: {msg:?}");
				if let Some(msg) = msg {
					match msg {
						Ok(msg) => {
							if let Some(order) = msg.order {
								if let Err(e) = receive_order(order, &status_tx, addr, &session_id).await {
									godot_error!("Error handling order: {e}");
									eprintln!("Error handling order: {e}");
								}
							}
						},
						Err(e) => {
							godot_error!("Received error from server: {e}");
							eprintln!("Received error from server: {e}");
						},
					}
				}
				
			}
			_ = cancel.cancelled() => {
				break;
			}
		}
	}
}


async fn receive_order(order: OrderEnum, status_tx: &mpsc::Sender<ClientStatus>, addr: SocketAddr, session_id: &Vec<u8>) -> Result<()> {
	println!("/c handling order");
	match order {
		OrderEnum::Punch(pu) => {
			let ip = pu.ip.parse::<Ipv4Addr>()?;
			let port = pu.port.try_into()?;
			let target_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(ip, port));

			println!("/c pre-punch");
			match punch_nat(target_addr, addr).await {
				Ok(_) => {
					println!("/c punch success");
					status_tx.send(ClientStatus { 
						session_id: Some(session_id.clone()), 
						status: Some(CStatus::PunchStatus(PunchStatus {
							message: None,
							success: true,
						})),
					}).await?;
				}

				Err(punch_err) => {
					println!("/c punch failure");
					status_tx.send(ClientStatus { 
						session_id: Some(session_id.clone()), 
						status: Some(CStatus::PunchStatus(PunchStatus {
							message: Some(format!("Error punching: {punch_err}")),
							success: false,
						})),
					}).await?;
				}
			};
			println!("/c punched");

			Ok(())
		},
		OrderEnum::Proxy(_) => {
			bail!("Unimplimented");
		}
	}
}

