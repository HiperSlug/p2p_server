use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};
use axum::{extract::{ConnectInfo, Path, State}, http::StatusCode, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use uuid::Uuid;


type SharedData = Arc<Mutex<HashMap<Uuid, Game>>>;


#[derive(Deserialize)]
struct CreateGamePayload {
    name: String,
}

struct Game {
    external: ExternalGame,
    internal: InternalGame,
}

#[derive(Serialize, Clone)]
struct ExternalGame {
    uuid: Uuid,
    name: String,
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
        .route("/matches/:uuid/clients", get(get_clients))
        .route("/matches/:uuid/join", post(join_match))
        .with_state(shared_data.clone())
        .into_make_service_with_connect_info::<SocketAddr>();
    
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    
    axum::serve(listener, app).await.unwrap();
}

async fn create_match(
    ConnectInfo(host): ConnectInfo<SocketAddr>,
    State(shared): State<SharedData>, 
    Json(payload): Json<CreateGamePayload>,
) -> Result<Json<Uuid>, StatusCode> {

    let uuid = Uuid::new_v4();

    // Create game object
    let game = Game {
        internal: InternalGame { 
            host, 
            clients: Vec::new(),
        },
        external: ExternalGame { 
            uuid, 
            name: payload.name, 
        },
    };

    // Add to list
    let mut games = shared.lock().map_err(|e| {
        eprintln!("Lock error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    games.insert(game.external.uuid, game);

    Ok(Json(uuid))
}

async fn get_matches(
    State(shared): State<SharedData>,
) -> Result<Json<HashMap<Uuid, ExternalGame>>, StatusCode> {
    let games = shared.lock().map_err(|e| {
        eprintln!("Lock error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    let externals  = games
        .iter()
        .map(|(uuid, game)| (*uuid, game.external.clone()))
        .collect();

    Ok(Json(externals))
}

#[axum::debug_handler]
async fn get_clients(
    State(shared): State<SharedData>,
    Path(uuid): Path<Uuid>,
) -> Result<Json<Vec<SocketAddr>>, StatusCode> {
    let games = shared.lock().map_err(|e| {
        eprintln!("Lock error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let game = games.get(&uuid).ok_or_else(|| StatusCode::BAD_REQUEST)?;
    
    Ok(Json(game.internal.clients.clone()))
}

async fn join_match(
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    State(shared): State<SharedData>,
    Path(uuid): Path<Uuid>,
) -> Result<Json<SocketAddr>, StatusCode> {
    let mut games = shared.lock().map_err(|e| {
        eprintln!("Lock error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let game = games.get_mut(&uuid).ok_or_else(|| StatusCode::BAD_REQUEST)?;
    
    game.internal.clients.push(client);

    Ok(Json(game.internal.host))
}