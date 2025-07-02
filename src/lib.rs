use std::error::Error;
use axum::extract::ws::Message as WsMessage;
use serde::Serialize;

pub mod server;
pub mod client;

pub trait ToWs: Serialize {
	fn to_ws(&self) -> Result<WsMessage, Box<dyn Error>> {
		let json = serde_json::to_string(&self)?;
		let msg = WsMessage::text(json);
		Ok(msg)
	}
}

pub enum Message {
	Server(ServerMsg),
	Client(ClientMsg),
}

pub enum Destination {
	Server,
	Client,
}

pub enum ServerMsg {
	GetListings,
	PostListing(Listing),
}

pub enum ClientMsg {
	ReceiveListings()
}

use godot::prelude::*;
use uuid::Uuid;

use crate::server::Listing;
struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}