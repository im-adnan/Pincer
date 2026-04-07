use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router, Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::manager::DownloadManager;
use crate::models::{RPCRequest, RPCResponse, RPCError};

pub async fn start_server(manager: Arc<DownloadManager>, rx: broadcast::Receiver<String>) {
    let app = Router::new()
        .route("/jsonrpc", get(ws_handler))
        .layer(Extension(manager))
        .layer(Extension(Arc::new(tokio::sync::Mutex::new(rx))));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:6842").await.unwrap();
    println!("Pincer listening on ws://127.0.0.1:6842/jsonrpc");
    
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(manager): Extension<Arc<DownloadManager>>,
    Extension(rx): Extension<Arc<tokio::sync::Mutex<broadcast::Receiver<String>>>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, manager, rx))
}

async fn handle_socket(
    socket: WebSocket,
    manager: Arc<DownloadManager>,
    rx: Arc<tokio::sync::Mutex<broadcast::Receiver<String>>>,
) {
    let (mut sender, mut receiver) = socket.split();
    
    // Broadcast loop: sends global events to this client
    let mut rx_guard = rx.clone();
    let mut rx_chan = rx_guard.lock().await;

    // We can't trivially multiplex tx and rx without tokio::select in a loop, so we'll 
    // setup a straightforward event loop for this socket.
    
    // A simplified loop just to handle inbound RPC
    while let Some(Ok(Message::Text(text))) = receiver.next().await {
        if let Ok(req) = serde_json::from_str::<RPCRequest>(&text) {
            let id = req.id.clone();
            let response = handle_method(req, &manager).await;
            if let Ok(json_res) = serde_json::to_string(&response) {
                let _ = sender.send(Message::Text(json_res)).await;
            }
        }
    }
}

async fn handle_method(req: RPCRequest, manager: &Arc<DownloadManager>) -> RPCResponse<serde_json::Value> {
    let method = req.method.as_str();
    
    let result = match method {
        "pin.tellActive" => {
            let active = manager.get_active_tasks().await;
            Some(serde_json::to_value(active).unwrap())
        },
        "pin.tellWaiting" => {
            let waiting = manager.get_waiting_tasks(0, 100).await;
            Some(serde_json::to_value(waiting).unwrap())
        },
        "pin.tellStopped" => {
            let stopped = manager.get_stopped_tasks(0, 100).await;
            Some(serde_json::to_value(stopped).unwrap())
        },
        "pin.getGlobalStat" => {
            let stat = manager.get_global_stat().await;
            Some(serde_json::to_value(stat).unwrap())
        },
        "pin.addUri" => {
            // Simplified: return an ID
            let id = uuid::Uuid::new_v4().to_string();
            Some(serde_json::to_value(id).unwrap())
        },
        _ => None,
    };

    RPCResponse {
        id: req.id,
        jsonrpc: "2.0".to_string(),
        result,
        error: if result.is_none() {
            Some(RPCError { code: -32601, message: "Method not found".to_string() })
        } else {
            None
        },
    }
}
