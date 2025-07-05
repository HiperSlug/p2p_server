use nat_puncher::server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    server::run("127.0.0.1:3000".parse().unwrap()).await?;
	Ok(())
}
