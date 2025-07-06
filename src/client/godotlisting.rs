use godot::prelude::*;
use crate::server::listing::{Listing, ListingNoId};


#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GdListing {
	id: String,
	gd_listing_no_id: GdListingNoId,
}

#[godot_api]
impl IRefCounted for GdListing {
	fn init(_: Base<RefCounted>) -> Self { 
		godot_error!("Dont instantiate GDListing yourself. Send a GDListingNoId to a server.");
		Self {
			id: "".to_string(),
			gd_listing_no_id: GdListingNoId { name: "".to_string() },
		}
	}
}

impl From<Listing> for GdListing {
	fn from(listing: Listing) -> Self {
		Self {
			id: listing.id().to_string(),
			gd_listing_no_id: listing.into_inner().into(),
		}
	}
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GdListingNoId {
	pub name: String,
}

#[godot_api]
impl IRefCounted for GdListingNoId   {
	fn init(_: Base<RefCounted>) -> Self { 
		Self {
			name: "".to_string(),
		}
	}
}

impl From<ListingNoId> for GdListingNoId {
	fn from(listing_no_id: ListingNoId) -> Self {
		Self {
			name: listing_no_id.name,
		}
	}
}

impl From<Gd<GdListingNoId>> for ListingNoId {
	fn from(gd_listing_no_id: Gd<GdListingNoId>) -> Self {
		Self {
			name: (gd_listing_no_id.bind()).name.clone(),
		}
	}
}