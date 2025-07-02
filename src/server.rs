use std::{net::SocketAddr, time::Duration};
use crate::puncher::{self, puncher_service_client::PuncherServiceClient, puncher_service_server::{PuncherService, PuncherServiceServer}, AddListingRequest, AddListingResult, GetListingsResult, ListingNoId, RemoveListingRequest, RemoveListingResult};
use tokio::time::sleep;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Default)]
pub struct PuncherServer;

#[tonic::async_trait]
impl PuncherService for PuncherServer {
    async fn add_listing(
        &self,
        request: Request<AddListingRequest>,
    ) -> Result<Response<AddListingResult>, Status> {
        Err(Status::aborted(""))
    }

    async fn remove_listing(
        &self,
        request: Request<RemoveListingRequest>,
    ) -> Result<Response<RemoveListingResult>, Status> {
        Err(Status::aborted(""))
    }

    async fn get_listings(
        &self,
        request: Request<puncher::Empty>,
    ) -> Result<Response<GetListingsResult>, Status> {
        Err(Status::aborted(""))
    }
}

pub async fn run() -> anyhow::Result<()> {
	let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
	let server = PuncherServer::default();

    tokio::spawn(async move {
        let mut client = PuncherServiceClient::connect("https://localhost:3000").await.unwrap();
        
        
        loop {
            sleep(Duration::from_secs(1)).await;
            let request = tonic::Request::new(AddListingRequest {
                listing: Some(ListingNoId {
					name: "Raphael".to_string(),
				}),
            });

            let response = match client.add_listing(request).await {
				Ok(resp) => resp,
				Err(e) => {
					eprintln!("Err: {e}");
					continue;
				}
			};

            println!("Response = {response:?}");
        }
    });

	Server::builder()
		.add_service(PuncherServiceServer::new(server))
		.serve(addr)
		.await?;
	
	Ok(())
}