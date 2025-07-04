use std::{net::{SocketAddr, SocketAddrV4}, sync::{Arc, Mutex as StdMutex}};
use godot::prelude::*;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use crate::puncher::{peer_service_server::PeerServiceServer, puncher_service_client::PuncherServiceClient};

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct PunchingClient {
	client: Arc<Mutex<Option<PuncherServiceClient<Channel>>>>,
	server: Option<PeerServiceServer<Channel>>,
	base: Base<RefCounted>,
	connected: Arc<StdMutex<bool>>,
}

#[godot_api]
impl IRefCounted for PunchingClient {
	fn init(base: Base<RefCounted>) -> Self {
		Self {
			client: Arc::new(Mutex::new(None)),
			server: None,
			base,
			connected: Arc::new(StdMutex::new(false)),
		}
	}
}

#[godot_api]
impl PunchingClient {
	#[signal]
	fn connected();

	#[func]
	pub fn connect(&mut self, target_ip: String, target_port: u16, self_ip: String, self_port: u16) {
		// connect to outgoing server
		let target_ip = match target_ip.parse() {
			Ok(ip) => ip,
			Err(e) => {
				godot_error!("PuncherClient: Could not parse ip: {e}.");
				return;
			}
		};

		let target_addr = SocketAddr::V4(SocketAddrV4::new(target_ip, target_port));
		let target_uri = format!("https://{target_addr}").parse().expect("Socket addr shouldnt fail entering URI.");

		let client = self.client.clone();
		let connected = self.connected.clone();

		// create server to receive incoming RPC

		tokio::spawn( async move {
			let channel = match Channel::builder(target_uri).connect().await {
				Ok(c) => c,
				Err(e) => {
					godot_error!("Failed to create a channel for the client: {e}");
					return;
				}
			};
			
			let mut client = client.lock().await;
			*client = Some(PuncherServiceClient::new(channel));

			let mut connected = connected.lock().expect("Mutex poisoned");
			*connected = true
		});
	}

	#[func]
	fn _physics_process(&mut self, _delta: f64) {
		if let Ok(flag) = self.connected.clone().try_lock().as_mut() {
			if **flag {
				**flag = false;
				self.base_mut().emit_signal("connected", &[]);
			}
		}
	}
}