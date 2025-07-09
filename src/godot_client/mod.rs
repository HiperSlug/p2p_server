use std::{net::{SocketAddr, SocketAddrV4}, sync::Arc};
use godot::prelude::*;
use tokio::{sync::RwLock, task::spawn_local};
use crate::{client::Client, listing::{GodotListing, GodotListingNoId, RustListingNoId}, ThreadSafe};

mod asyncvalue;
use asyncvalue::AsyncValue;

#[derive(GodotClass)]
#[class(base=RefCounted)]
struct PunchingClient {
	base: Base<RefCounted>,
	client: ThreadSafe<Option<Client>>,
	
	connected: AsyncValue<bool>,
	listings: AsyncValue<Option<Array<Gd<GodotListing>>>>,
	owned_listing: AsyncValue<Option<String>>,
	joined_dst: AsyncValue<Vec<SocketAddr>>,
}

#[godot_api]
impl IRefCounted for PunchingClient {
	fn init(base: Base<RefCounted>) -> Self { Self {
		base,
		client: Arc::new(RwLock::new(None)),
		
		connected: AsyncValue::from_default("connection_changed"),
		listings: AsyncValue::from_default("listings_changed"),
		owned_listing: AsyncValue::from_default("owned_listing_changed"),
		joined_dst: AsyncValue::from_default("joined_addrs_changed"),
	}}
}

#[godot_api]
impl PunchingClient {
	#[func]
	pub fn _physics_process(&mut self, _: f64) {
		let mut base = self.base().clone();

		if let Some((sig, val)) = self.connected.poll() {
			base.emit_signal(sig, &[val.to_variant()]);
		};

		if let Some((sig, val)) = self.listings.poll() {
			let val = val.clone().map_or(Variant::nil(), |v| v.to_variant());
			base.emit_signal(sig, &[val]);
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
	
	#[signal]
	pub fn connection_changed(new_connection: Variant);
	#[signal]
	pub fn listings_changed(new_listings: Variant);
	#[signal]
	pub fn owned_listing_changed(new_owned_listing: Variant);
	#[signal]
	pub fn joined_addrs_changed(new_joined_addr: Variant);

	#[func]
	pub fn connect(&self, server_ip: String, server_port: u16, ip: String, port: u16) {
		// parse addrs //
		let server_ip = match server_ip.parse() {
			Ok(ip) => ip,
			Err(e) => {
				godot_error!("Could not parse server_ip: {e}.");
				return;
			}
		};
		let server_addr = SocketAddr::V4(SocketAddrV4::new(server_ip, server_port));

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

		spawn_local(async move {
			let new_client = match Client::new(addr, server_addr, joined_dst).await {
				Ok(c) => c,
				Err(e) => {
					godot_error!("Unable to make new client: {e}");
					return;
				},
			};
			let mut client = client.write().await;
			*client = Some(new_client);
			
			let mut flag = connected_flag.write().await;
			*flag = true;
		});
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
		spawn_local(async move {
			let mut joined_dst = joined_dst.write().await;
			if index >= joined_dst.len() {
				godot_error!("Remove joined index out of bounds");
				return;
			}
			joined_dst.remove(index);
		});
	}
	
	#[func]
	pub fn disconnect(&self) {
		let client = self.client.clone();
		let connected_flag = self.connected.inner().clone();
		
		spawn_local( async move {
			let mut client = client.write().await;
			let c = client.take();
			if let Some(c) = c {
				c.end().await;
			} else {
				godot_error!("Disconnected a non-connected Client.");
			}

			let mut flag = connected_flag.write().await;
			*flag = false
		});
	}
	
	#[func]
	pub fn create_listing(&self, listing: Gd<GodotListingNoId>) {
		let client = self.client.clone();
		// let owned_listing_flag = self.owned_listing.inner().clone();

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when creating listing.");
				return;
			};

			let _ = match c.create_listing(RustListingNoId::from((*listing.bind()).clone())).await {
				Ok(l) => l,
				Err(e) => {
					godot_error!("Couldnt create listing: {e}.");
					return;
				}
			};

			// let flag = owned_listing_flag.write().await;
			// *flag = listing_id // TODO TURN THIS INTO A UUID THEN STRING
		});
	}

	#[func]
	pub fn remove_listing(&self) {
		let client = self.client.clone();
		let owned_listing_flag = self.owned_listing.inner().clone();

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when removing listing.");
				return;
			};

			if let Err(e) = c.remove_listing().await {
				godot_error!("Couldnt remove listing: {e}");
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

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when getting listings.");
				return;
			};

			let listings = match c.get_listings().await {
				Ok(l) => l,
				Err(e) => {
					godot_error!("Couldnt get listings: {e}");
					return
				}
			};

			let gd_listings: Array<Gd<GodotListing>> = listings
				.into_iter()
				.map(|l| Gd::from_object(GodotListing::from(l)))
				.collect();

			let mut listings = client_listings.write().await;
			*listings = Some(gd_listings);
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

		spawn_local(async move {
			let mut client = client.write().await;
			let Some(c) = client.as_mut() else {
				godot_error!("Client not connected when getting listings.");
				return;
			};

			if let Err(e) = c.join(listing_id).await {
				godot_error!("Error while joining listing: {e}");
				return;
			};
		});
	}
}