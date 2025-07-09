use crate::{client::{punch_nat, Client}, listing::RustListingNoId, server::{run_signal, session::Session}, ThreadSafe};
use tokio::{net::UdpSocket, sync::{oneshot, RwLock}};
use std::{net::SocketAddr, str::FromStr, sync::Arc, time::{Duration, Instant}};

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

// -- UTIL -- //

async fn test_server() -> SocketAddr {
	let addr = ephemeral_addr().await;

	let (ready_tx, ready_rx) = oneshot::channel();
	tokio::spawn(run_signal(addr, ready_tx));
	ready_rx.await.unwrap();

	addr
}

async fn test_client(server_addr: SocketAddr) -> (Client, ThreadSafe<Vec<SocketAddr>>) {
	let dst = Arc::new(RwLock::new(Vec::new()));
	(Client::new(ephemeral_addr().await, "http://127.0.0.1".to_string(), server_addr.port(), dst.clone()).await.unwrap(), dst)
}

async fn ephemeral_addr() -> SocketAddr {
	let temp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
	temp.local_addr().unwrap()
}


// -- RELEVANT STUFF -- //

#[tokio::test]
/* AI */ async fn listings() {
	let s_addr = test_server().await;
	let (mut c_1, _) = test_client(s_addr).await;
	let (mut c_2, _) = test_client(s_addr).await;

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

	assert!(a.is_ok() && b.is_ok(), "a: {a:?}, \nb: {b:?}");
}


#[tokio::test]
async fn client_punching() {
	let s_addr = test_server().await;
	let (mut c_1, dst_1) = test_client(s_addr).await;
	let (mut c_2, dst_2) = test_client(s_addr).await;
	
	
	let listing = RustListingNoId { name: "test listing".to_string() };
	c_1.create_listing(listing).await.unwrap();

	let listings = c_2.get_listings().await.unwrap();
	assert_eq!(listings.len(), 1);

	let target_listing = &listings[0];
	
	{
		let dst_1 = dst_1.read().await;
		let dst_2 = dst_2.read().await;
		assert_eq!(dst_1.len(), 0);
		assert_eq!(dst_2.len(), 0);
	}
	

	c_2.join(*target_listing.id()).await.unwrap();

	{
		let dst_1 = dst_1.read().await;
		let dst_2 = dst_2.read().await;
		assert_eq!(dst_1.len(), 1);
		assert_eq!(dst_2.len(), 1);
	}
}