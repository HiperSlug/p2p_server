use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod server;
pub mod client;

#[derive(Serialize, Deserialize, Clone)]
pub struct Listing {
	id: Uuid,
	name: String,
}

impl Listing {
	pub fn from(name: String) -> Listing {
		let id = Uuid::new_v4();
		Listing {
			id,
			name,
		}
	}
}

