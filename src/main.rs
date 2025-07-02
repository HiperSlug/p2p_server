use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use tokio::time::sleep;
use tonic::{transport::Server, Request, Response, Status};
use puncher::hello_service_server::{HelloService, HelloServiceServer};
use puncher::{HelloRequest, HelloResponse};

use crate::puncher::hello_service_client::HelloServiceClient;

pub mod puncher {
	tonic::include_proto!("puncher");
}

#[derive(Default)]
pub struct MyHelloService;

#[tonic::async_trait]
impl HelloService for MyHelloService {
	async fn say_hello(&self, request: Request<HelloRequest>) -> Result<Response<HelloResponse>, Status> {
		let reply = HelloResponse {
			message: format!("Hello {}!", request.into_inner().name),
		};
		Ok(Response::new(reply))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000));
	let hello_service = MyHelloService::default();

    tokio::spawn(async move {
        let mut client = HelloServiceClient::connect("https://localhost:3000").await.unwrap();
        
        
        loop {
            sleep(Duration::from_secs(1)).await;
            let request = tonic::Request::new(HelloRequest {
                name: "World".into(),
            });

            let response = client.say_hello(request).await.unwrap();

            println!("Response={response:?}");

        }
    });

	Server::builder()
		.add_service(HelloServiceServer::new(hello_service))
		.serve(addr)
		.await?;

	Ok(())
}
