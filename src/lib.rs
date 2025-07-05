use godot::prelude::*;

pub mod server;
pub mod client;

pub mod puncher {
	tonic::include_proto!("puncher");
}

#[cfg(test)]
mod tests;

struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}