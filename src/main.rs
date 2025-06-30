use std::{collections::HashMap, error::Error, net::SocketAddr, sync::{Arc, Mutex}};
use axum::{extract::{ConnectInfo, State}, http::StatusCode, routing::post, Json, Router};
use serde_json::Value;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, net::{TcpListener, TcpStream}};
use uuid::Uuid;


type SharedData = Arc<Mutex<HashMap<Uuid, InternalGame>>>;


#[derive(Deserialize)]
struct CreateGamePayload {
    name: String,
}

#[derive(Serialize, Clone)]
struct Game {
    name: String,
    uuid: Uuid,
}

struct InternalGame {
    host: SocketAddr,
    clients: Vec<SocketAddr>,
}

#[tokio::main]
async fn main() {
    let shared_data: SharedData = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/matches", post(create_match).get(get_matches))
        .route("/matches/clients", get(get_clients))
        .route("/matches/join", post(join_match))
        .with_state(shared_data.clone())
        .into_make_service_with_connect_info::<SocketAddr>();
    
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    
    axum::serve(listener, app).await.unwrap();
}

async fn create_match(
    ConnectInfo(host): ConnectInfo<SocketAddr>,
    State(shared): State<SharedData>, 
    Json(payload): Json<CreateGamePayload>,
) -> Result<Game, StatusCode> {

    // Create game object
    let game = Game {
        name: payload.name,
        uuid: Uuid::new_v4(),
        host,
        clients: Vec::new(),
    };

    // Add to list
    let mut games = shared.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    games.insert(game.uuid, game.clone());

    Ok(game)
}

async fn get_matches(
    State(shared): State<SharedData>,
) -> Result<Json<HashMap<Uuid, Game>>, StatusCode> {
    let games = shared.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(games.clone()))
}

async fn get_clients(
    Json(uuid): Json<Uuid>,
) -> Json<Value> {

}

async fn join_match(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    State(shared): State<SharedData>,
    Json(payload): Json<Value>
) -> Result<StatusCode, StatusCode> {

    let Value::Number(match_index) = payload else {
        return Err(StatusCode::BAD_REQUEST)
    };
    let match_index = match_index.as_u64().unwrap() as usize;
    
    let games = shared.lock().unwrap();
    let host_addr = games.get(match_index).unwrap().host_addr;

    
    let (to_host, to_client) = tokio::join!(
        send_addr(&host_addr, &client_addr),
        send_addr(&client_addr, &host_addr),
    );

    if let Err(e) = to_host {
        eprintln!("Error when sending client address to host: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
    if let Err(e) = to_client {
        eprintln!("Error when sending host address to client: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
    
    Ok(StatusCode::ACCEPTED)
}

async fn send_addr(to: &SocketAddr, address: &SocketAddr) -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect(to).await?;
    stream.write_all(address.to_string().as_bytes()).await?;
    Ok(())
}
