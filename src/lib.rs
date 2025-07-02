use godot::prelude::*;

pub mod server;
pub mod client;

pub mod rpc {
	use tarpc::{context::Context, service};
	
	#[service]
	pub trait Puncher {
		async fn hello(name: String) -> String;
	}

	pub struct Server {

	}

	impl Puncher for Server {
		async fn hello(self, _: Context, str: String) -> String {
			format!("Hello {str}!")
		}
	}
}

struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}