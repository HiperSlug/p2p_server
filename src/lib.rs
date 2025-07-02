use godot::prelude::*;

pub mod server;
pub mod client;


struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}