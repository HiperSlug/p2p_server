use std::{net::SocketAddr, sync::Arc, time::{Duration, Instant}};
use tokio::sync::{mpsc, watch::Receiver, RwLock};
use tonic::Status;
use uuid::Uuid;
use crate::{puncher::{client_status::Status as ClientStatus, Order}, server::listing::Listing};

pub type SessionRef = Arc<RwLock<Session>>;
pub type OrderStream = mpsc::Sender<Result<Order, Status>>;
pub type StatusStream = Receiver<ClientStatus>;

pub struct Session {
	pub id: Uuid,
	pub addr: SocketAddr,
	pub last_seen: Instant,
	pub listing: Option<Listing>,
	pub stream: Option<(OrderStream, StatusStream)>,
}

impl Session {
	const TIMEOUT: Duration = Duration::from_secs(60 * 15);

	pub async fn new(addr: SocketAddr) -> SessionRef {
		Arc::new(RwLock::new(Self {
			id: Uuid::new_v4(),
			last_seen: Instant::now(),
			listing: None,
			stream: None,
			addr,
		}))
	}

	pub fn see(&mut self) {
		self.last_seen = Instant::now();
	}

	pub fn is_valid(&self) -> bool {
		!self.is_timed_out() && self.stream.is_some()
	}

	pub fn is_timed_out(&self) -> bool {
		self.last_seen.elapsed() > Self::TIMEOUT
	}
}