pub mod server;
pub mod client;


use godot::prelude::*;

struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}