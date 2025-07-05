use std::{net::SocketAddr, str::FromStr, time::{Duration, Instant}};
use crate::server::session::Session;

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
	assert!(s.is_timed_out()); // failing
}

