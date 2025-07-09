use std::{net::{SocketAddr, SocketAddrV4}, sync::{Arc, OnceLock}};
use godot::{obj::WithBaseField, prelude::*};
use tokio::{runtime::{self, Handle, Runtime}, sync::RwLock};
use crate::{client::Client, listing::{GodotListing, GodotListingNoId, RustListing, RustListingNoId}, ThreadSafe};

mod asyncvalue;
use asyncvalue::AsyncValue;


static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn handle() -> Handle {
	RUNTIME.get_or_init(|| { runtime::Builder::new_multi_thread().enable_all().build().unwrap() }).handle().clone()
}

#[derive(GodotClass)]
#[class(base=Node)]
struct PunchingClient {
	base: Base<Node>,
	client: ThreadSafe<Option<Client>>,
	
	errors: AsyncValue<String>,
	connected: AsyncValue<bool>,
	listings: AsyncValue<Option<Vec<RustListing>>>,
	owned_listing: AsyncValue<Option<String>>,
	joined_dst: AsyncValue<Vec<SocketAddr>>,
}

#[godot_api]
impl INode for PunchingClient {
	fn enter_tree(&mut self) {
		self.base_mut().set_physics_process(true);
	}

	fn init(base: Base<Node>) -> Self { 
		Self {
			base,
			client: Arc::new(RwLock::new(None)),
			
			connected: AsyncValue::from_default("connection_changed"),
			listings: AsyncValue::from_default("listings_changed"),
			owned_listing: AsyncValue::from_default("owned_listing_changed"),
			joined_dst: AsyncValue::from_default("joined_addrs_changed"),
			errors: AsyncValue::from_default("async_error"),
		}
	}

	fn physics_process(&mut self, _: f64) {
		
		let mut base = self.base().clone();

		if let Some((sig, error)) = self.errors.poll() {
			base.emit_signal(sig, &[error.to_variant()]);
		}

		if let Some((sig, val)) = self.connected.poll() {
			base.emit_signal(sig, &[val.to_variant()]);
		};

		if let Some((sig, val)) = self.listings.poll() {
			let gd_listings = val
				.as_ref()
				.map_or(Variant::nil(), |v| {
					let arr: Array<Gd<GodotListing>> = v
						.into_iter()
						.map(|l| Gd::from_object(GodotListing::from(l.clone())))
						.collect();
					arr.to_variant()
				});

			base.emit_signal(sig, &[gd_listings]);
		};

		if let Some((sig, val)) = self.owned_listing.poll() {
			let val = val.clone().map_or(Variant::nil(), |v| v.to_variant());
			base.emit_signal(sig, &[val]);
		};
		
		if let Some((sig, val)) = self.joined_dst.poll() {
			
			
			let val: Array<Dictionary> = val
				.clone()
				.iter()
				.map(|addr| {
					let mut dict = Dictionary::new();
					dict.set("ip", addr.ip().to_string());
					dict.set("port", addr.port());
					dict
				})
				.collect();

			base.emit_signal(sig, &[val.to_variant()]);
		};
	}
}

#[godot_api]
impl PunchingClient {
	#[signal]
	pub fn connection_changed(new_connection: Variant);
	#[signal]
	pub fn listings_changed(new_listings: Variant);
	#[signal]
	pub fn owned_listing_changed(new_owned_listing: Variant);
	#[signal]
	pub fn joined_addrs_changed(new_joined_addr: Variant);
	#[signal]
	pub fn async_error(msg: GString);

	#[func]
	pub fn connect(&self, server_url: String, server_port: u16, ip: String, port: u16) {
		let ip = match ip.parse() {
			Ok(ip) => ip,
			Err(e) => {
				godot_error!("Could not parse ip: {e}");
				return;
			}
		};
		let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));
		
		// spawn task //
		let client = self.client.clone();
		let connected_flag = self.connected.inner().clone();
		let joined_dst = self.joined_dst.inner().clone();
		let error = self.errors.inner().clone();
		
		
		let fut = async move {
			let new_client = match Client::new(addr, server_url, server_port, joined_dst).await {
				Ok(c) => c,
				Err(e) => {
					let mut err = error.write().await;
					*err = e.to_string();
					return;
				},
			};

			let mut client = client.write().await;
			*client = Some(new_client);

			let mut flag = connected_flag.write().await;
			*flag = true;
		};

		handle().spawn(fut);
	}

	#[func]
	pub fn remove_joined(&self, index: u32) {
		let index = match index.try_into() {
			Ok(i) => i,
			Err(e) => {
				godot_error!("Unable to convert u32 to usize for indexing: {e}");
				return;
			}
		};
		let joined_dst = self.joined_dst.inner().clone();
		let error = self.errors.inner().clone();

		handle().spawn(async move {
			let mut joined_dst = joined_dst.write().await;
			if index >= joined_dst.len() {
				let mut err = error.write().await;
				*err = String::from("Index out of bounds");
				return;
			}
			joined_dst.remove(index);
		});
	}
	
	#[func]
	pub fn disconnect(&self) {
		let client = self.client.clone();
		let connected_flag = self.connected.inner().clone();
		let error = self.errors.inner().clone();

		handle().spawn( async move {
			let mut client = client.write().await;
			let c = client.take();
			if let Some(c) = c {
				c.end().await;
			} else {
				let mut err = error.write().await;
				*err = String::from("Not connected");
			}

			let mut flag = connected_flag.write().await;
			*flag = false
		});
	}
	
	#[func]
	pub fn create_listing(&self, listing: Gd<GodotListingNoId>) {
		let client = self.client.clone();
		let listing = RustListingNoId::from(listing.bind().clone());
		let owned_listing_flag = self.owned_listing.inner().clone();
		let error = self.errors.inner().clone();

		handle().spawn(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				let mut err = error.write().await;
				*err = String::from("Not connected");
				return;
			};

			let listing_id = match c.create_listing(listing).await {
				Ok(l) => l,
				Err(e) => {
					let mut err = error.write().await;
					*err = e.to_string();
					return;
				}
			};

			let mut flag = owned_listing_flag.write().await;
			*flag = Some(listing_id.to_string());
		});
	}

	#[func]
	pub fn remove_listing(&self) {
		let client = self.client.clone();
		let owned_listing_flag = self.owned_listing.inner().clone();
		let error = self.errors.inner().clone();

		handle().spawn(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				let mut err = error.write().await;
				*err = String::from("Not connected");
				return;
			};

			if let Err(e) = c.remove_listing().await {
				let mut err = error.write().await;
				*err = e.to_string();
				return;
			};

			let mut flag = owned_listing_flag.write().await;
			*flag = None
		});
	}
	
	#[func]
	pub fn get_listings(&self) {
		let client = self.client.clone();
		let client_listings = self.listings.inner().clone();
		let error = self.errors.inner().clone();

		handle().spawn(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				let mut err = error.write().await;
				*err = String::from("Not connected");
				return;
			};

			let listings = match c.get_listings().await {
				Ok(l) => l,
				Err(e) => {
					let mut err = error.write().await;
					*err = e.to_string();
					return
				}
			};

			let mut flag = client_listings.write().await;
			*flag = Some(listings);
		});
	}

	
	#[func]
	pub fn join_listing(&self, listing_id: String) {
		let listing_id = match listing_id.parse() {
			Ok(id) => id,
			Err(e) => {
				godot_error!("Couldnt join listing: {e}");
				return
			}
		};

		let client = self.client.clone();
		let error = self.errors.inner().clone();

		handle().spawn(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				let mut err = error.write().await;
				*err = String::from("Not connected");
				return;
			};

			if let Err(e) = c.join(listing_id).await {
				let mut err = error.write().await;
				*err = e.to_string();
				return;
			};
		});
	}
}