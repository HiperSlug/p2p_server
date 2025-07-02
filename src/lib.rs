use godot::prelude::*;

pub mod server;
pub mod client;

pub mod rpc {
	use tarpc::{context::Context, service};
	
	#[service]
	pub trait Puncher {
		async fn hello(name: String) -> String;
	}

	#[derive(Clone)]
	pub struct PuncherServer;

	impl Puncher for PuncherServer {
		async fn hello(self, _: Context, name: String) -> String {
			format!("Hello, {name}!")
		}
	}
}

struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}