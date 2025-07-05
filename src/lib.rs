use godot::prelude::*;

#[cfg(test)]
mod tests;

pub mod server;
pub mod client;

pub mod puncher {
	tonic::include_proto!("puncher");
}

struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}