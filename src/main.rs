use std::net::SocketAddr;
use p2p_server::server::{self, SharedData};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let shared_data: SharedData = server::create_shared_data();

    let app = server::server_app(shared_data.clone())
        .into_make_service_with_connect_info::<SocketAddr>();

    let listener = TcpListener::bind("127.0.0.1:8080").await
        .expect("TcpListener bind failed.");
    
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {e}")
    };
}