use std::net::SocketAddr ;
use crate::puncher::puncher_service_server::PuncherServiceServer;
use tonic::transport::Server;


mod session;
mod listing;
mod server;

pub async fn run() -> anyhow::Result<()> {
	let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
	let server = server::PuncherServer::default();

	Server::builder()
		.add_service(PuncherServiceServer::new(server))
		.serve(addr)
		.await?;
	
	Ok(())
}
