use std::net::{Ipv4Addr, SocketAddrV4};

use nat_puncher::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let addr = std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000));
    server::run(addr).await
}
