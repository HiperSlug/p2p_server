use std::{error::Error, net::{IpAddr, Ipv4Addr, SocketAddr}, sync::{Arc, Mutex}};
use axum::{body::to_bytes, extract::{ConnectInfo, State}, http::StatusCode, routing::post, Json, Router};
use serde_json::Value;
use serde::Serialize;
use tokio::{io::AsyncWriteExt, net::{TcpListener, TcpSocket, TcpStream}};

type SharedData = Arc<Mutex<Vec<Game>>>;

#[derive(Serialize)]
struct Game {
    host_addr: SocketAddr,
}

#[tokio::main]
async fn main() {
    let shared_data: SharedData = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/matches", post(create_match).get(get_matches))
        .with_state(shared_data.clone())
        .into_make_service_with_connect_info::<SocketAddr>();
    
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    
    axum::serve(listener, app).await.unwrap();
}

async fn create_match(
    ConnectInfo(host_addr): ConnectInfo<SocketAddr>,
    State(shared): State<SharedData>, 
) -> Result<StatusCode, StatusCode> {
    
    let game = Game {
        host_addr,
    };

    let mut games = shared.lock().unwrap();
    games.push(game);

    Ok(StatusCode::CREATED)
}

async fn get_matches(
    State(shared): State<SharedData>,
) -> Json<Value> {
    let games = shared.lock().unwrap();
    Json(serde_json::to_value(&*games).unwrap())
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
