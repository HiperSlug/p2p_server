use godot::prelude::*;

use crate::server::listing::{Listing, ListingNoId};


#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GDListing {
	id: String,
	gd_listing_no_id: GDListingNoId,
}

#[godot_api]
impl IRefCounted for GDListing {
	fn init(_: Base<RefCounted>) -> Self { 
		godot_error!("Dont instantiate GDListing yourself. Send a GDListingNoId to a server.");
		Self {
			id: "".to_string(),
			gd_listing_no_id: GDListingNoId { name: "".to_string() },
		}
	}
}

impl From<Listing> for GDListing {
	fn from(listing: Listing) -> Self {
		Self {
			id: listing.id().to_string(),
			gd_listing_no_id: listing.into_inner().into(),
		}
	}
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GDListingNoId {
	pub name: String,
}

#[godot_api]
impl IRefCounted for GDListingNoId   {
	fn init(_: Base<RefCounted>) -> Self { 
		Self {
			name: "".to_string(),
		}
	}
}

impl From<ListingNoId> for GDListingNoId {
	fn from(listing_no_id: ListingNoId) -> Self {
		Self {
			name: listing_no_id.name,
		}
	}
}

impl From<GDListingNoId> for ListingNoId {
	fn from(gd_listing_no_id: GDListingNoId) -> Self {
		Self {
			name: gd_listing_no_id.name,
		}
	}
}