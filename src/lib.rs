use godot::prelude::*;

pub mod server;
pub mod client;

pub mod puncher {
	tonic::include_proto!("puncher");
}

struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}