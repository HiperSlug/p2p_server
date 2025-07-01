use serde::{Deserialize, Serialize};
use crate::Listing;



#[derive(Serialize, Deserialize)]
pub enum Message {
	Listings(Vec<Listing>),
}