use std::{net::{SocketAddr, SocketAddrV4}, sync::Arc, time::Duration};
use anyhow::Result;
use godot::prelude::*;
use tokio::{sync::{mpsc::{self, Sender}, watch, RwLock}, task::spawn_local, time::sleep};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Channel, Request};
use crate::puncher::{puncher_service_client::PuncherServiceClient, CreateSessionRequest, Ping};

type ThreadSafe<T> = Arc<RwLock<T>>;

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct PunchingClient {
	base: Base<RefCounted>,
	client: ThreadSafe<Option<Client>>,
}

#[godot_api]
impl IRefCounted for PunchingClient {
	fn init(base: Base<RefCounted>) -> Self { Self {
		base,
		client: Arc::new(RwLock::new(None)),
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
			let new_client = Client::from(addr, server_addr).await;
			let mut client = client.write().await;
			*client = Some(new_client);
		});
	}
}


struct Client {
	client: PuncherServiceClient<Channel>,
	session_id: Vec<u8>,
	stop_ping: watch::Sender<bool>,
}

impl Client {
	async fn from(
		addr: SocketAddr, 
		server_addr: SocketAddr
	) -> Self {
		let mut client = match Self::create_client(server_addr).await {
			Ok(c) => c,
			Err(e) => {
				godot_error!("Unable to create client: {e}");
				panic!("Unable to create client: {e}");
			}
		};

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

		let stop_ping = match Self::start_session(&mut client, &session_id).await {
			Ok(s) => s,
			Err(e) => {
				godot_error!("Unable to start session: {e}");
				panic!("Unable to start session: {e}");
			}
		};

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
	) -> Result<watch::Sender<bool>> {
		// start stream //
		let (ping_tx, ping_rx) = mpsc::channel(8);
		let request = Request::new(ReceiverStream::new(ping_rx));
		
		let response = client.stream_session(request).await?;
		let response = response.into_inner();

		// pings //
		ping_tx.send(Ping { session_id: Some(session_id.clone()) }).await?;
		
		let (stop_tx, stop_rx) = watch::channel(false);

		tokio::spawn(ping(ping_tx, stop_rx));
	
		// orders //
		// TODO w/ response

		Ok(stop_tx)
	}
}


async fn ping(sender: Sender<Ping>, mut stopper: watch::Receiver<bool>) {
	loop {
		tokio::select! {
			_ = sleep(Duration::from_secs(60)) => {
				let _ = sender.send(Ping { session_id: None }).await;
			}
			_ = stopper.changed() => {
				if *stopper.borrow() {
					break;
				}
			}
		}
	}
}
