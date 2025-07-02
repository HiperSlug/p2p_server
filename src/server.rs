use tarpc::context;

#[tarpc::service]
pub trait Puncher {
    async fn hello(name: String) -> String;
}
#[derive(Clone)]
pub struct Server {}

impl Puncher for Server {
    async fn hello(self, _: context::Context, name: String) -> String {
        format!("Hello, {name}!")
    }
}