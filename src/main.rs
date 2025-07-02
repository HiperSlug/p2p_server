use tonic::{transport::Server, Request, Response, Status};
use puncher::hello_service_server::{HelloService, HelloServiceServer};
use puncher::{HelloRequest, HelloResponse};

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
	let addr = "[::1]:50051".parse()?;
	let hello_service = MyHelloService::default();

	Server::builder()
		.add_service(HelloServiceServer::new(hello_service))
		.serve(addr)
		.await?;

	Ok(())
}
