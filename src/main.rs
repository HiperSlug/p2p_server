use futures::{prelude::*};
use tarpc::{client, context, server::{self, incoming::Incoming, Channel}};

#[tarpc::service]
trait Puncher {
    async fn hello(name: String) -> String;
}

#[derive(Clone)]
struct PuncherServer;

impl Puncher for PuncherServer {
    async fn hello(self, _: context::Context, name: String) -> String {
        format!("Hello, {name}!")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (client_transport, server_transport) = tarpc::transport::channel::unbounded();

    let server = server::BaseChannel::with_defaults(server_transport);
    tokio::spawn(
        server.execute(PuncherServer.serve())
            .for_each(|response| async move {
                tokio::spawn(response);
            }));

    let client = PuncherClient::new(client::Config::default(), client_transport).spawn();

    let hello = client.hello(context::current(), "Stim".to_string()).await?;

    println!("{hello}");

    Ok(())
}