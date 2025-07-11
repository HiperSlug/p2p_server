use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc::{Receiver, Sender}, Mutex};
use tonic::Status;
use uuid::Uuid;
use crate::{proto::{client_stream_message::ClientStreamEnum, ServerStreamMessage}, server::listing::RustListing};

pub type SessionRef = Arc<Mutex<Session>>;

pub type StreamSender = Sender<Result<ServerStreamMessage, Status>>;
pub type StreamReceiver = Receiver<ClientStreamEnum>;

pub struct Session {
	pub listing: Option<RustListing>,
	streams: (StreamSender, StreamReceiver),
	id: Uuid,
	addr: SocketAddr,
}

impl Session {
	pub fn new(addr: SocketAddr, stream_tx: StreamSender, stream_rx: StreamReceiver) -> Self {
		Self {
			id: Uuid::new_v4(),
			listing: None,
			streams: (stream_tx, stream_rx),
			addr,
		}
	}
	
	pub fn new_ref(addr: SocketAddr, stream_tx: StreamSender, stream_rx: StreamReceiver) -> SessionRef {
		Arc::new(Mutex::new(Self::new(addr, stream_tx, stream_rx)))
	}

	pub fn id(&self) -> &Uuid {&self.id}

	pub fn addr(&self) -> &SocketAddr {&self.addr}

	pub fn streams(&mut self) -> (&StreamSender, &mut StreamReceiver) { (&self.streams.0, &mut self.streams.1) }
}
