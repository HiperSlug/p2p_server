use std::{net::SocketAddr, sync::{Arc, Mutex}};
use axum::{extract::{ConnectInfo, State}, http::StatusCode, routing::{post}, Json, Router};
use serde_json::Value;
use serde::Serialize;


type SharedData = Arc<Mutex<Vec<Game>>>;

#[derive(Serialize)]
struct Game {
    host_addr: SocketAddr,
}

#[tokio::main]
async fn main() {
    let shared_data: SharedData = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/match", post(new_match).get(get_matches))
        .with_state(shared_data.clone())
        .into_make_service_with_connect_info::<SocketAddr>();
    
    let listener: tokio::net::TcpListener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    
    axum::serve(listener, app).await.unwrap();
}

async fn new_match(
    ConnectInfo(host_addr): ConnectInfo<SocketAddr>,
    State(shared) : State<SharedData>, 
) -> Result<StatusCode, StatusCode> {
    
    let game = Game {
        host_addr,
    };

    let mut games = shared.lock().map_err(|_| {
        eprintln!("Mutex poisoned!");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    games.push(game);

    Ok(StatusCode::CREATED)
}

async fn get_matches(
    State(shared) : State<SharedData>,
) -> Json<Value> {
    let games = shared.lock().unwrap();
    Json(serde_json::to_value(&*games).unwrap())
}