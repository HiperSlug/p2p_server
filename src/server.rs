use std::{collections::HashMap, error::Error, sync::Arc};
use axum::{extract::ws::{WebSocket, WebSocketUpgrade, Message as WsMessage}, response::IntoResponse, routing::get, Extension, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;
use futures::{stream::SplitStream, SinkExt, StreamExt};
use futures::stream::SplitSink;
use crate::client::{Message as ClientMessage, Listing as ClientListing};

#[derive(Serialize, Deserialize, Clone)]
pub struct Listing {
	id: Uuid,
	listing: ClientListing,
}

#[derive(Serialize, Deserialize)]
pub enum Message{
	AddListing(ClientListing),
	RemoveListing,
	GetListings,
	Forward(Uuid, ClientMessage),
}

struct Server {
	sockets: Mutex<HashMap<Uuid, Arc<Mutex<SplitSink<WebSocket, WsMessage>>>>>,
	listings: Mutex<HashMap<Uuid, Listing>>,
	id_map: Mutex<HashMap<Uuid, Uuid>>,
}

impl Server {
	pub fn new() -> Arc<Server> {
		Arc::new(Server {
			sockets: Mutex::new(HashMap::new()),
			id_map: Mutex::new(HashMap::new()),
			listings: Mutex::new(HashMap::new()),
		})
	}

	pub async fn add_ws_write(&self, session_id: Uuid, ws_write: SplitSink<WebSocket, WsMessage>) {
		let mut sockets = self.sockets.lock().await;
		sockets.insert(session_id, Arc::new(Mutex::new(ws_write)));
	}

	pub async fn remove_ws_write(&self, session_id: &Uuid) {
		self.remove_listing(session_id).await;
		let mut sockets = self.sockets.lock().await;
		sockets.remove(session_id);
	}

	pub async fn send_msg(
		&self, 
		to: &Uuid, 
		msg: &ClientMessage,
	) -> Result<(), Box<dyn Error>> {
		let mut sockets = self.sockets.lock().await;
		let ws_write = sockets.get_mut(to).ok_or("ID doesn't point to a socket.")?.clone();

		let mut ws_write = ws_write.lock().await;

		let json = serde_json::to_string(msg)?;
		let msg = WsMessage::text(json);

		ws_write.send(msg).await?;
		Ok(())
	}

	pub async fn listing_to_session(&self, listing_id: &Uuid) -> Option<Uuid> {
		let id_map = self.id_map.lock().await;
		id_map.get(listing_id).cloned()
	}

	pub async fn listings(&self) -> Vec<Listing> {
		let listings = self.listings.lock().await;
		listings
			.values()
			.cloned()
			.collect()
	}

	pub async fn add_listing(&self, session_id: Uuid, listing: ClientListing) {
		self.remove_listing(&session_id).await;

		println!("New listing: {listing:?}");

		let listing = Listing {
			id: Uuid::new_v4(), 
			listing
		};

		let mut id_map = self.id_map.lock().await;
		id_map.insert(listing.id, session_id);
		
		let mut listings = self.listings.lock().await;
		listings.insert(session_id, listing);
	}

	pub async fn remove_listing(&self, session_id: &Uuid) {
		let mut listings = self.listings.lock().await;
		listings.remove(session_id);
	}
}

pub fn app() -> Router {
	let server = Server::new();
	
	Router::new()
	 	.route("/ws", get(ws_handler))
		.layer(Extension(server.clone()))
}

async fn ws_handler(
	ws: WebSocketUpgrade,
	Extension(server): Extension<Arc<Server>>,
) -> impl IntoResponse {
	ws.on_upgrade(move |socket| handle_socket(socket, server))
}

async fn handle_socket(
	socket: WebSocket,
	server: Arc<Server>,
) {
	let session_id = Uuid::new_v4();

	println!("New connection: {session_id}");
	
	let (ws_write, ws_read) = socket.split();
	server.add_ws_write(session_id, ws_write).await;

	handle_ws_read(ws_read, &session_id, server.clone()).await;	

	server.remove_ws_write(&session_id).await;

	println!("Disconnect: {session_id}");
}

async fn handle_ws_read(
	mut ws_read: SplitStream<WebSocket>,
	session_id: &Uuid,
	server: Arc<Server>,
) {
	while let Some(Ok(msg)) = ws_read.next().await {
		if let WsMessage::Text(text) = msg {
			let received: Message = match serde_json::from_str(&text) {
				Ok(data) => data,
				Err(e) => {
					eprintln!("Received invalid message: {e}");
					continue;
				}
			};
			match received {
				Message::AddListing(l) => server.add_listing(*session_id, l).await,
				Message::RemoveListing => server.remove_listing(session_id).await,
				Message::GetListings => {
					if let Err(e) = server.send_msg(session_id, &ClientMessage::Listings(server.listings().await)).await {
						eprintln!("Unable to send listings: {e}")
					}
				}
				Message::Forward(listing_id, msg) => {
					let to = server.listing_to_session(&listing_id).await.unwrap_or(listing_id);
					if let Err(e) = server.send_msg(&to, &msg).await {
						eprintln!("Failed to forward message: {e}")
					}
				}
			}
		}
	}
}