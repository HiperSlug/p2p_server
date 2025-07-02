use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;
use nat_puncher::server;

#[tokio::main]
async fn main() {
    let app = server::app();
    let addr: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 3000);

    let listener = TcpListener::bind(addr).await.expect("TcpListener failed binding.");
    if let Err(e) = axum::serve(listener, app).await {
        println!("Server error: {e}")
    }
}