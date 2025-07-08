use std::{net::{SocketAddr, SocketAddrV4}, sync::Arc, time::Duration};
use anyhow::{bail, Result};
use futures::StreamExt;
use tokio::{sync::mpsc::Sender, time::sleep};
use tokio_util::sync::CancellationToken;
use tonic::{Status, Streaming};
use crate::proto::{client_status::Status as ClientStatusEnum, order::Order as OrderEnum, ClientStatus as ClientStatusStruct, Order as OrderStruct, Proxy, Punch, PunchStatus};
use super::punch_nat;


pub struct StreamHandler {
	status_stream: Sender<ClientStatusStruct>,
	bind_addr: SocketAddr,
	cancel: CancellationToken,
}

impl StreamHandler {
	pub fn new(status_stream: Sender<ClientStatusStruct>, bind_addr: SocketAddr) -> Arc<Self> {
		Arc::new(Self {
			status_stream,
			bind_addr,
			cancel: CancellationToken::new(),
		})
	}

	pub async fn start(self: Arc<Self>, order_stream: Streaming<OrderStruct>) {
		tokio::join!(
			self.keepalive(),
			self.orders(order_stream),
		);
	}

	pub fn stop(&self) { self.cancel.cancel() }

	async fn keepalive(&self) {
		loop {
			tokio::select! {
				_ = sleep(Duration::from_secs(55)) => {
					if let Err(e) = self.status_stream.send(ClientStatusStruct { session_id: None, status: None }).await {
						eprintln!("Error sending keepalive: {e}");
					};
				}
				_ = self.cancel.cancelled() => break,
			}
		}
	}

	async fn orders(&self, mut order_stream: Streaming<OrderStruct>) {
		loop {
			tokio::select! {
				msg = order_stream.next() => {
					match msg {
						Some(order) => self.handle_order(order).await,
						None => break,
					}
				},
				_ = self.cancel.cancelled() => break,
			}
		}
	}

	async fn handle_order(&self, order: Result<OrderStruct, Status>) {
		let order = match order {
			Ok(o) => o,
			Err(s) => {
				eprintln!("Received error status order from server: {s}");
				return;
			},
		};
		let Some(order) = order.order else {
			eprintln!("Received empty order from server.");
			return;
		};

		match order {
			OrderEnum::Punch(p) => if let Err(e) = self.punch(p).await { eprintln!("Error punching: {e}") },
			OrderEnum::Proxy(p) => if let Err(e) = self.proxy(p).await { eprintln!("Error proxying: {e}") },
		}
	}

	async fn punch(&self, order: Punch) -> Result<()> {
		let target_addr = SocketAddr::V4(SocketAddrV4::new(order.ip.parse()?, order.port.try_into()?));

		match punch_nat(target_addr, self.bind_addr).await {
			Ok(()) => {
				let msg = ClientStatusStruct { 
					session_id: None, 
					status: Some(ClientStatusEnum::PunchStatus( PunchStatus { 
						message: None, 
						success: true,
					}))
				};

				if let Err(e) = self.status_stream.send(msg).await {
					eprintln!("Unable to send punch status: {e}");
				};
				Ok(())
			},
			Err(e) => {
				let msg = ClientStatusStruct { 
					session_id: None, 
					status: Some(ClientStatusEnum::PunchStatus( PunchStatus { 
						message: Some(format!("Unable to punch: {e}")), 
						success: false,
					}))
				};
				
				if let Err(e) = self.status_stream.send(msg).await {
					eprintln!("Unable to send punch status: {e}");
				};
				Err(e)
			}
		}
	}

	async fn proxy(&self, _: Proxy) -> Result<()> {bail!("Unimplemented.")}

}





