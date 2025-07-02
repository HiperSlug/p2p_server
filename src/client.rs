use godot::{classes::{ISprite2D, Sprite2D}, obj::Base, prelude::{godot_api, GodotClass}};
use serde::{Deserialize, Serialize};
use crate::server::Listing as ServerListing;

#[derive(GodotClass)]
#[class(base=Sprite2D)]
struct Puncher{
	base: Base<Sprite2D>
}

#[godot_api]
impl ISprite2D for Puncher {
	fn init(base: Base<Sprite2D>) -> Self {
		Self {
			base,
		}
	}
}

#[derive(Serialize, Deserialize)]
pub enum Message {
	Listings(Vec<ServerListing>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Listing {
	name: String,
}