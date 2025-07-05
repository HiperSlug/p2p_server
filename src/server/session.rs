use std::{net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use tokio::sync::{mpsc::Sender, watch::Receiver, RwLock};
use tonic::Status;
use uuid::Uuid;
use crate::{puncher::{client_status::Status as ClientStatus, Order}, server::listing::Listing};

pub type SessionRef = Arc<RwLock<Session>>;

pub type OrderStream = Sender<Result<Order, Status>>;
pub type StatusStream = Receiver<ClientStatus>;

pub struct Session {
	pub listing: Option<Listing>,
	pub stream: Option<(OrderStream, StatusStream)>,
	id: Uuid,
	addr: SocketAddr,
	last_seen: Instant,
	timeout_time: Duration,
}

impl Session {
	pub const TIMEOUT_TIME: Duration = Duration::from_secs(60);

	pub fn new(addr: SocketAddr) -> Self {
		Self {
			id: Uuid::new_v4(),
			last_seen: Instant::now(),
			listing: None,
			stream: None,
			timeout_time: Self::TIMEOUT_TIME,
			addr,
		}
	}
	
	pub fn new_ref(addr: SocketAddr) -> SessionRef {
		Arc::new(RwLock::new(Self::new(addr)))
	}

	pub fn id(&self) -> Uuid {self.id}

	pub fn addr(&self) -> SocketAddr {self.addr}

	pub fn see(&mut self) {
		self.last_seen = Instant::now();
	}

	pub fn is_valid(&self) -> bool {
		!self.is_timed_out() && self.stream.is_some()
	}

	pub fn is_timed_out(&self) -> bool {
		self.last_seen.elapsed() > self.timeout_time
	}

	#[cfg(test)]
	pub fn set_last_seen(&mut self, instant: Instant) {
		self.last_seen = instant;
	}
}