use anyhow::{anyhow, Error, Result};
use uuid::Uuid;
use crate::proto::{Listing as TonicListing, ListingNoId as TonicListingNoId};

// ---- RUST ---- //

// Rust Listing //
#[derive(Debug, PartialEq, Clone)]
pub struct RustListing {
	listing_no_id: RustListingNoId,
	id: Uuid,
}

impl RustListing {
	pub fn new(listing_no_id: impl Into<RustListingNoId>) -> Self {
		Self {
			listing_no_id: listing_no_id.into(),
			id: Uuid::new_v4(),
		}
	}

	pub fn id(&self) -> &Uuid {&self.id}

	pub fn inner(&self) -> &RustListingNoId {&self.listing_no_id}

	pub fn into_inner(self) -> RustListingNoId {self.listing_no_id}
}

impl TryFrom<TonicListing> for RustListing {
	type Error = Error;

	fn try_from(listing_packet: TonicListing) -> Result<Self> {
		Ok(Self {
			listing_no_id: listing_packet
				.listing_no_id
				.ok_or(anyhow!("Empty inner listing."))?
				.into(),
			id: listing_packet.id.try_into()?,
		})
	}
}


// RUST ListingNoId //
#[derive(Clone, Debug, PartialEq)]
pub struct RustListingNoId {
	pub name: String, 
}

impl From<TonicListingNoId> for RustListingNoId {
	fn from(listing_no_id_packet: TonicListingNoId) -> Self {
		Self {
			name: listing_no_id_packet.name,
		}
	}
}



// ---- TONIC ---- //

// TONIC Listing //
impl From<RustListing> for TonicListing {
	fn from(listing: RustListing) -> Self {
		Self {
			listing_no_id: Some(listing.listing_no_id.into()),
			id: listing.id.into(),
		}
	}
}


// TONIC ListingNoId //
impl From<RustListingNoId> for TonicListingNoId {
	fn from(listing_no_id: RustListingNoId) -> Self {
		Self {
			name: listing_no_id.name,
		}
	}
}