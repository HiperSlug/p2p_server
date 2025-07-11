use std::{net::{IpAddr, SocketAddr}, time::Duration};
use tokio::{sync::{broadcast, mpsc::Sender}, time::sleep};
use tokio_util::sync::CancellationToken;
use tonic::Streaming;
use uuid::Uuid;
use anyhow::{anyhow, Result};
use crate::{proto::{client_stream_message::ClientStreamEnum, server_stream_message::ServerStreamEnum, ClientStreamMessage, PunchStatus, ServerStreamMessage}, TIMEOUT};



pub struct Session {
	session_id: Uuid,
	cancellation_token: CancellationToken,
}

impl Session {
	pub fn uuid(&self) -> &Uuid { &self.session_id }

	pub fn id(&self) -> Vec<u8> { self.session_id.as_bytes().to_vec() }

	pub fn end(self) { self.cancellation_token.cancel() }

	pub async fn start(
		session_id: Uuid,
		server_rx: Streaming<ServerStreamMessage>,
		client_tx: Sender<ClientStreamMessage>,
	) -> Result<(Self, broadcast::Receiver<SocketAddr>)> {
		let cancellation_token = CancellationToken::new();

		let (joined_tx, joined_rx) = broadcast::channel(8);

		tokio::spawn(handle_stream(server_rx, client_tx.clone(), cancellation_token.clone(), joined_tx));
		tokio::spawn(keepalive(client_tx, cancellation_token.clone()));

		Ok((
			Self {
				session_id,
				cancellation_token,
			},
			joined_rx,
		))
	}
}

async fn handle_stream(
	mut server_rx: Streaming<ServerStreamMessage>, 
	client_tx: Sender<ClientStreamMessage>, 
	cancellation_token: CancellationToken,
	joined_broadcast: broadcast::Sender<SocketAddr>,
) {
	tokio::select! {
		_ = async {
			loop {
				match server_rx.message().await {
					Ok(opt) => {
						match opt {
							Some(msg_struct) => {
								if let Some(msg) = msg_struct.server_stream_enum {
									
									// Actual message handling
									match msg {
										ServerStreamEnum::Punch(punch) => { // PUNCH
											// parse the addr
											let addr = match parse_addr(&punch.ip, punch.port) {
												Ok(a) => a,
												Err(e) => {
													eprintln!("Received bad punch addr: {e}");
													let msg = ClientStreamMessage { client_stream_enum: Some(ClientStreamEnum::PunchStatus(PunchStatus { 
														message: Some(format!("Bad punch addr: {e}")),
														success: false,
													}))};
													if let Err(e) = client_tx.send_timeout(msg, TIMEOUT).await {
														eprintln!("Unable to send punch status (bad addr) to server: {e}");
													};
													continue;
												},
											};
											
											// attempt punching
											match super::punch(addr).await {
												Ok(_) => {
													// broadcast joined addr
													if let Err(e) = joined_broadcast.send(addr) {
														eprintln!("Unable to broadcast joined addr: {e}");
													};

													// send ok message to server
													let msg = ClientStreamMessage { client_stream_enum: Some(ClientStreamEnum::PunchStatus(PunchStatus { 
														message: None,
														success: true,
													}))};
													if let Err(e) = client_tx.send_timeout(msg, TIMEOUT).await {
														eprintln!("Unable to send punch status (bad addr) to server: {e}");
													};
												},
												Err(e) => {
													// send not-ok message to server
													println!("Unable to punch: {e}"); // not neccecarily an error to fail punching.
													let msg = ClientStreamMessage { client_stream_enum: Some(ClientStreamEnum::PunchStatus(PunchStatus { 
														message: Some(format!("Unable to punch: {e}")),
														success: false,
													}))};
													if let Err(e) = client_tx.send_timeout(msg, TIMEOUT).await {
														eprintln!("Unable to send punch status (bad addr) to server: {e}");
													};
												},
											}
										}
									}

								};
							},
							None => break,
						}
					},
					Err(status) => {
						eprintln!("Received status message from server: {status}. Continuing.");
					},
				}
			}
		} => {}
		_ = cancellation_token.cancelled() => {}
	}
}

async fn keepalive(
	client_tx: Sender<ClientStreamMessage>,
	cancellation_token: CancellationToken,
) {
	tokio::select! {
		_ = async {
			loop {
				let keepalive = ClientStreamMessage { client_stream_enum: None };

				if let Err(e) = client_tx.send_timeout(keepalive, TIMEOUT).await {
					eprintln!("Error sending keepalive ping: {e}");
				};

				sleep(Duration::from_secs(60)).await;
			}
		} => {}
		_ = cancellation_token.cancelled() => {}
	}
}


fn parse_addr<P>(ip: &str, port: P) -> Result<SocketAddr>
where
	P: TryInto<u16>,
	P::Error: std::fmt::Display,
{
	let ip: IpAddr = ip
		.parse()
		.map_err(|e| anyhow!("Unable to parse ip: {e}"))?;
	
	let port: u16 = port
		.try_into()
		.map_err(|e| anyhow!("Unable to coerce port: {e}"))?;
	
	Ok(SocketAddr::new(ip, port))
}