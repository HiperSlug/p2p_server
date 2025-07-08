use std::sync::Arc;
use godot::prelude::*;
use tokio::sync::RwLock;

type ThreadSafe<T> = Arc<RwLock<T>>;

pub mod server;
pub mod client;
pub mod godot_client;

pub mod listing;

#[cfg(test)]
mod tests;

pub mod proto {
	tonic::include_proto!("puncher");
}


struct NATPuncher;

#[gdextension]
unsafe impl ExtensionLibrary for NATPuncher {}