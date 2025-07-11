use crate::{client::{punch, Client}, server::{listing::RustListingNoId, run}};
use tokio::net::UdpSocket;
use tonic::transport::Uri;
use std::net::SocketAddr;

// -- UTIL -- //
async fn local_addr() -> SocketAddr {
	let temp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
	temp.local_addr().unwrap()
}

async fn test_server() -> SocketAddr {
	let addr = local_addr().await;
	tokio::spawn(run(addr));
	addr
}

async fn test_client(addr: SocketAddr) -> Client {
	let uri = Uri::builder()
		.scheme("https")
		.authority(addr.to_string())
		.path_and_query("/")
		.build()
		.unwrap();
	Client::new(uri).await.unwrap()
}

// -- TESTS -- //

#[tokio::test]
/* AI */ async fn listings() {
	let s_addr = test_server().await;
	let mut c_1 = test_client(s_addr).await;
	let mut c_2 = test_client(s_addr).await;

	let _ = c_1.start_session().await.unwrap();
	let _ = c_2.start_session().await.unwrap();

	let listing = RustListingNoId { name: "test listing".to_string() };
	let l = listing.clone();
	c_1.create_listing(listing).await.unwrap(); 

	let listings = c_2.get_listings().await.unwrap();
	assert_eq!(listings.len(), 1);

	let target_listing = &listings[0];
	assert_eq!(*target_listing.inner(), l);

	let listing = RustListingNoId { name: "test listing".to_string() };
	c_2.create_listing(listing).await.unwrap();

	let listings = c_2.get_listings().await.unwrap();
	assert_eq!(listings.len(), 2);

	c_2.remove_listing().await.unwrap(); 
	c_1.remove_listing().await.unwrap();

	let listings = c_1.get_listings().await.unwrap();
	assert_eq!(listings.len(), 0);
}

#[tokio::test]
/* AI */ async fn punching() {
	let bind_a = local_addr().await;
	let bind_b = local_addr().await;
	
	let (a, b) = tokio::join!(
		punch(bind_a),
		punch(bind_b),
	);

	assert!(a.is_ok() && b.is_ok(), "a: {a:?}, \nb: {b:?}");
}

#[tokio::test]
async fn client_punching() {
	let s_addr = test_server().await;
	let mut c_1 = test_client(s_addr).await;
	let mut c_2 = test_client(s_addr).await;
	
	let dst_1 = c_1.start_session().await.unwrap();
	let dst_2 = c_2.start_session().await.unwrap();
	
	let listing = RustListingNoId { name: "test listing".to_string() };
	c_1.create_listing(listing).await.unwrap();

	let listings = c_2.get_listings().await.unwrap();
	assert_eq!(listings.len(), 1);

	let target_listing = &listings[0];
	
	assert!(dst_1.is_empty());
	assert!(dst_2.is_empty());

	c_2.join(*target_listing.id()).await.unwrap();

	assert!(!dst_1.is_empty());
	assert!(!dst_2.is_empty());
}