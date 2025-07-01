use serde::{Deserialize, Serialize};
use crate::server::Listing as ServerListing;

#[derive(Serialize, Deserialize)]
pub enum Message {
	Listings(Vec<ServerListing>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Listing {
	name: String,
}