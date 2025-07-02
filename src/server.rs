use crate::puncher::{self, puncher_service_server::PuncherService, AddListingRequest, AddListingResult, GetListingsResult, RemoveListingRequest, RemoveListingResult};
use tonic::{Request, Response, Status};

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