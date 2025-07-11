use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

type ThreadSafe<T> = Arc<RwLock<T>>;

const TIMEOUT: Duration = Duration::from_secs(10);

pub mod server;
pub mod client;

pub mod proto {
	tonic::include_proto!("puncher");
}


#[cfg(test)]
mod tests;


#[cfg(feature = "godot")]
use godot::prelude::*;

#[cfg(feature = "godot")]
struct NATPuncher;

#[cfg(feature = "godot")]
#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}

#[cfg(feature = "godot")]
pub mod godot_client;