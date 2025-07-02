use nat_puncher::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run().await?;
	Ok(())
}
