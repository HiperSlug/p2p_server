use reqwest::Client;
use tokio::time::sleep;
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use uuid::Uuid;
use crate::server::ExternalGame;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct CreateGamePayload {
    pub name: String,
}

async fn handle_match(addr: &SocketAddr, payload: CreateGamePayload, client: &Client) -> Result<(), reqwest::Error>{
	let uuid = create_match(addr, payload, client)
		.await?;
	
	loop {
		let clients = get_clients(addr, &uuid, client)
			.await?;
		
		// if there are new clients start pinging them.
		sleep(Duration::from_secs(5)).await;
	}

}


async fn create_match(addr: &SocketAddr, payload: CreateGamePayload, client: &Client) -> Result<Uuid, reqwest::Error> {
	let url = format!("http://{addr}/matches");
	let uuid = client.post(url)
		.json(&payload)
		.send()
		.await?
		.json::<Uuid>()
		.await?;
	
	Ok(uuid)
}

async fn get_clients(addr: &SocketAddr, uuid: &Uuid, client: &Client) -> Result<Vec<SocketAddr>, reqwest::Error> {
	let url = format!("http://{addr}/matches/{uuid}/clients");
	let clients = client.get(url)
		.send()
		.await?
		.json::<Vec<SocketAddr>>()
		.await?;
	
	Ok(clients)
}

async fn get_matches(addr: &SocketAddr, client: &Client) -> Result<HashMap<Uuid, ExternalGame>, reqwest::Error> {
	let url = format!("http://{addr}/matches");
	let game = client.get(url)
		.send()
		.await?
		.json::<HashMap<Uuid, ExternalGame>>()
		.await?;

	Ok(game)
}

async fn join_match(addr: &SocketAddr, client: &Client, uuid: &Uuid) -> Result<(), reqwest::Error> {
	let url = format!("http://{addr}/matches/{uuid}/join");
	client.post(url)
		.send()
		.await?
		.error_for_status()?;

	Ok(())
}