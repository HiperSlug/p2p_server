use crate::{client::Client, puncher::puncher_service_server::PuncherServiceServer, server::{session::Session, PuncherServer}};
use anyhow::Result;
use tokio::{net::TcpListener, sync::oneshot::Sender};
use tonic::transport::Server;
use std::{net::{SocketAddr, SocketAddrV4}, str::FromStr, time::{Duration, Instant}};use crate::puncher::{AddListingRequest, GetListingsRequest, RemoveListingRequest, ListingNoId};

// -- SESSION -- //

#[test]
fn test_session_timeout() {
	let addr = SocketAddr::from_str("127.0.0.1:8080").unwrap();
	
	let timeout_time =  Instant::now() - Session::TIMEOUT_TIME;

	println!("Now: {:?}", Instant::now());

	let mut s = Session::new(addr);
	let t = timeout_time + Duration::from_secs(5);
	println!("Under: {t:?}");
	s.set_last_seen(t);
	assert!(!s.is_timed_out());
	
	let mut s = Session::new(addr);
	let t = timeout_time - Duration::from_secs(5);
	println!("Over: {t:?}");
	s.set_last_seen(t); 
	assert!(s.is_timed_out());
}

// -- SERVER -- //

/* AI */ async fn setup() -> (Client, Sender<Result<()>> ) {
	println!("Setting up server and client");
	let server_ip = "127.0.0.1".parse().unwrap();
	let server_port = 0;
	let server_addr = SocketAddr::V4(SocketAddrV4::new(server_ip, server_port));

	let listener = TcpListener::bind(server_addr).await.unwrap();
	let local_addr = listener.local_addr().unwrap();
	println!("Server bound to {local_addr}");

	let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<Result<()>>();

	tokio::spawn(async move {
		println!("Starting server...");
		Server::builder()
			.add_service(PuncherServiceServer::new(PuncherServer::default()))
			.serve_with_incoming_shutdown(
				tokio_stream::wrappers::TcpListenerStream::new(listener),
				async {
					shutdown_rx.await.ok();
					println!("Server shutting down");
				}
			)
			.await
			.unwrap();
	});

	let client = Client::new(local_addr, local_addr).await;
	println!("Client created");

	(client, shutdown_tx)
}

#[tokio::test]
/* AI */ async fn test_add_and_get_listing() {
	println!("Starting test_add_and_get_listing");
	let (mut client, end) = setup().await;
	let session_id = client.id().clone();

	let name = "test listing";
	println!("Adding listing: {name}");

	let res = client.inner_mut().add_listing(AddListingRequest {
		session_id,
		listing: Some(ListingNoId { name: name.to_string() }),
	}).await.unwrap();

	println!("Listing added with ID: {:?}", res.get_ref().listing_id);

	let listings = match client.inner_mut().get_listings(GetListingsRequest {}).await {
		Ok(l) => l,
		Err(e) => panic!("Err({e})"),
	};
	println!("Got listings: {:?}", listings.get_ref().listings);

	assert_eq!(listings.get_ref().listings.len(), 1);
	assert_eq!(listings.get_ref().listings[0].listing_no_id.as_ref().unwrap().name, name);
	println!("test_add_and_get_listing finished successfully");

	end.send(Ok(())).unwrap();
}

#[tokio::test]
/* AI */ async fn test_remove_listing() {
	println!("Starting test_remove_listing");
	let (mut client, end) = setup().await;
	let session_id = client.id().clone();

	println!("Adding test listing");
	client.inner_mut().add_listing(AddListingRequest {
		session_id: session_id.clone(),
		listing: Some(ListingNoId { name: "test".to_string() }),
	}).await.unwrap();

	println!("Removing listing");
	client.inner_mut().remove_listing(RemoveListingRequest {
		session_id: session_id.clone(),
	}).await.unwrap();

	let listings = client.inner_mut().get_listings(GetListingsRequest {}).await.unwrap();
	println!("Remaining listings: {:?}", listings.get_ref().listings);

	assert!(listings.get_ref().listings.is_empty());
	println!("test_remove_listing finished successfully");

	end.send(Ok(())).unwrap();
}