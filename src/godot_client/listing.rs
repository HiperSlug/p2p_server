use godot::prelude::*;
use crate::listing::{RustListing, RustListingNoId};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GodotListing {
	#[var]
	id: GString,
	#[var]
	listing_no_id: Gd<GodotListingNoId>,
}

#[godot_api]
impl IRefCounted for GodotListing {
	fn init(_: Base<RefCounted>) -> Self {
		Self {
			id: GString::new(),
			listing_no_id: Gd::from_init_fn(|b|GodotListingNoId::init(b)),
		}
	}
}

impl From<RustListing> for GodotListing {
	fn from(listing: RustListing) -> Self {
		Self {
			id: listing.id().to_string().into(),
			listing_no_id: Gd::from_object(listing.into_inner().into()),
		}
	}
}


// LISTINGNOID //
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct GodotListingNoId {
	#[var]
	pub name: GString,
}

#[godot_api]
impl IRefCounted for GodotListingNoId   {
	fn init(_: Base<RefCounted>) -> Self { 
		Self {
			name: GString::new(),
		}
	}
}

impl From<RustListingNoId> for GodotListingNoId {
	fn from(listing_no_id: RustListingNoId) -> Self {
		Self {
			name: listing_no_id.name.into(),
		}
	}
}

impl From<GodotListingNoId> for RustListingNoId {
	fn from(gd_listing_no_id: GodotListingNoId) -> Self {
		Self {
			name: gd_listing_no_id.name.into(),
		}
	}
}