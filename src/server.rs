use std::{collections::HashMap, sync::Arc};
use axum::{extract::{ws::{Message, WebSocket}, WebSocketUpgrade}, response::IntoResponse, routing::get, Extension, Router};
use tokio::sync::Mutex;
use uuid::Uuid;
use futures::StreamExt;
use futures::stream::SplitSink;

struct Server {
	sockets: Mutex<HashMap<Uuid, SplitSink<WebSocket, Message>>>,
}

pub fn app() -> Router {
	let server = Arc::new(Server {
		sockets: Mutex::new(HashMap::new())
	});
	
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
	let (ws_write, mut ws_read) = socket.split();

	let uuid = Uuid::new_v4();
	{
		let mut sockets = server.sockets.lock().await;
		sockets.insert(uuid, ws_write);
	}

	while let Some(Ok(msg)) = ws_read.next().await {
		println!("Received from client: {msg:?}");
	}

	let mut sockets = server.sockets.lock().await;
	sockets.remove(&uuid);
}