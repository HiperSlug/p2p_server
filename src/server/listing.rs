use anyhow::{anyhow, Result};
use uuid::Uuid;
use crate::puncher::{Listing as ListingPacket, ListingNoId as ListingNoIdPacket};

pub struct Listing {
	listing_no_id: ListingNoId,
	id: Uuid,
}

impl Listing {
	pub fn new(listing_no_id: impl Into<ListingNoId>) -> Listing {
		Listing {
			listing_no_id: listing_no_id.into(),
			id: Uuid::new_v4(),
		}
	}

	pub fn id(&self) -> &Uuid {&self.id}

	pub fn inner(&self) -> &ListingNoId {&self.listing_no_id}

	pub fn into_inner(self) -> ListingNoId {self.listing_no_id}
}

impl TryFrom<ListingPacket> for Listing {
	type Error = anyhow::Error;

	fn try_from(listing_packet: ListingPacket) -> Result<Self> {
		Ok(Self {
			listing_no_id: listing_packet
				.listing_no_id
				.ok_or(anyhow!("Empty inner listing."))?
				.into(),
			id: listing_packet.id.try_into()?,
		})
	}
}

impl From<&Listing> for ListingPacket {
	fn from(listing: &Listing) -> Self {
		Self {
			listing_no_id: Some(listing.listing_no_id.clone().into()),
			id: listing.id.into(),
		}
	}
}

#[derive(Clone)]
pub struct ListingNoId {
	pub name: String, 
}

impl From<ListingNoIdPacket> for ListingNoId {
	fn from(listing_no_id_packet: ListingNoIdPacket) -> Self {
		Self {
			name: listing_no_id_packet.name,
		}
	}
}

impl From<ListingNoId> for ListingNoIdPacket {
	fn from(listing_no_id: ListingNoId) -> Self {
		Self {
			name: listing_no_id.name,
		}
	}
}