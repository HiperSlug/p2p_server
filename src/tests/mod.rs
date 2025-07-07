use crate::{client::{client::{punch_nat, Client}}, server::{self, listing::ListingNoId, session::Session}};
use tokio::{net::UdpSocket, sync::oneshot};
use std::{net::SocketAddr, str::FromStr, time::{Duration, Instant}};

// -- SESSION -- //
#[test]
fn test_session_timeout() {
	let addr = SocketAddr::from_str("127.0.0.1:8080").unwrap();
	
	let timeout_time =  Instant::now() - Session::TIMEOUT_TIME;

	let mut s = Session::new(addr);
	let t = timeout_time + Duration::from_secs(5);
	s.set_last_seen(t);
	assert!(!s.is_timed_out());
	
	let mut s = Session::new(addr);
	let t = timeout_time - Duration::from_secs(5);
	s.set_last_seen(t); 
	assert!(s.is_timed_out());
}

// -- OTHER STUFF -- //

async fn test_server() -> SocketAddr {
	let addr = ephemeral_addr().await;

	let (ready_tx, ready_rx) = oneshot::channel();
	tokio::spawn(server::run_signal(addr, ready_tx));
	ready_rx.await.unwrap();

	addr
}

async fn test_client(server_addr: SocketAddr) -> Client {
	Client::new(ephemeral_addr().await, server_addr).await
}

async fn ephemeral_addr() -> SocketAddr {
	let temp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
	temp.local_addr().unwrap()
}


// -- ACTUALLY RELEVANT STUFF -- //
#[tokio::test]
/* AI */ async fn listings() { // inconsistent
	let s_addr = test_server().await;
	let mut c_1 = test_client(s_addr).await;
	let mut c_2 = test_client(s_addr).await;

	let listing = ListingNoId { name: "test listing".to_string() };
	let l = listing.clone();
	c_1.create_listing(listing).await.expect("Failed to create listing.");

	let listings = c_2.get_listings().await.expect("Failed to get listing."); // inconsistent
	assert_eq!(listings.len(), 1);

	let target_listing = &listings[0];
	assert_eq!(*target_listing.inner(), l);

	let listing = ListingNoId { name: "test listing".to_string() };
	c_2.create_listing(listing).await.expect("Failed to create listing.");

	let listings = c_2.get_listings().await.expect("Failed to get listings.");
	assert_eq!(listings.len(), 2);

	c_2.remove_listing().await.expect("Failed to remove listing."); 
	c_1.remove_listing().await.expect("Failed to remove listing.");

	let listings = c_1.get_listings().await.expect("Failed to get listings.");
	assert_eq!(listings.len(), 0);
}

#[tokio::test]
/* AI */ async fn punching() {
	let bind_a = ephemeral_addr().await;
	let bind_b = ephemeral_addr().await;
	
	let (a, b) = tokio::join!(
		async move {
			punch_nat(bind_a, bind_b).await
		},
		async move {
			punch_nat(bind_b, bind_a).await
		},
	);

	assert!(a.is_ok() && b.is_ok(), "a: {a:?}, \nb: {b:?},");
}


#[tokio::test]
async fn client_punching() { // inconsistent
	let s_addr = test_server().await;
	let mut c_1 = test_client(s_addr).await;
	let mut c_2 = test_client(s_addr).await;

	let listing = ListingNoId { name: "test listing".to_string() };
	c_1.create_listing(listing).await.expect("Failed to create listing.");

	let listings = c_2.get_listings().await.expect("Failed to get listings.");
	assert_eq!(listings.len(), 1);

	let target_listing = &listings[0];

	c_2.join(*target_listing.id()).await.expect("Failed to join listing.");
}