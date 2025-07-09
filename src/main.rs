use nat_puncher::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run("0.0.0.0:3000".parse().unwrap()).await?;
	Ok(())
}
