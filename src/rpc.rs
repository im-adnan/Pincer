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
    
    // Handle inbound RPC messages (Text or Binary)
    while let Some(msg_res) = receiver.next().await {
        if let Ok(msg) = msg_res {
            let text = match msg {
                Message::Text(t) => Some(t),
                Message::Binary(b) => String::from_utf8(b).ok(),
                Message::Close(_) => break,
                _ => None,
            };
            
            if let Some(text) = text {
                println!("Received RPC: {}", text);
                if let Ok(req) = serde_json::from_str::<RPCRequest>(&text) {
                    let response = handle_method(req, &manager).await;
                    if let Ok(json_res) = serde_json::to_string(&response) {
                        // Ignore frequent polling methods for logging purposes
                        if !text.contains("tellActive") && !text.contains("tellWaiting") && !text.contains("tellStopped") {
                            println!("Sending Response: {}", json_res);
                        }
                        let _ = sender.send(Message::Text(json_res)).await;
                    }
                } else {
                    println!("Failed to parse RPC request");
                }
            }
        } else {
            break; // Socket error
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
            let id = uuid::Uuid::new_v4().to_string();
            
            // Extract the URL and options from the JSONRPC params
            // params is typically: [["url1"], {"dir": "...", "out": "...", "split": "64"}]
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    if params_array.len() >= 2 {
                        let (uris_val, options_val) = if params_array.len() >= 3 && params_array[0].is_string() {
                            // Format: ["token:secret", ["url"], {options}]
                            (&params_array[1], &params_array[2])
                        } else {
                            // Format: [["url"], {options}]
                            (&params_array[0], &params_array[1])
                        };

                        if let (Some(uris), Some(options)) = (uris_val.as_array(), options_val.as_object()) {
                            if let Some(first_uri) = uris.first().and_then(|v| v.as_str()) {
                                let url = first_uri.to_string();
                                let default_dir = std::env::var("HOME")
                                    .map(|h| format!("{}/Downloads", h))
                                    .unwrap_or_else(|_| "/tmp".to_string());

                                let dir = options.get("dir")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&default_dir)
                                    .to_string();
                                
                                // Determine filename (from "out" option, or fallback to URL leaf)
                                let filename = options.get("out")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| {
                                        url.split('/').last().unwrap_or("download.bin").split('?').next().unwrap_or("download.bin").to_string()
                                    });

                                let threads = options.get("split")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<usize>().ok())
                                    .unwrap_or(4); // Default to 4 threads if not specified
                                
                                // Spawn the task in the background via the manager
                                manager.spawn_task(id.clone(), url, filename, dir, threads).await;
                            }
                        }
                    }
                }
            }
            
            Some(serde_json::to_value(id).unwrap())
        },
        _ => None,
    };

    let error = if result.is_none() {
        Some(RPCError { code: -32601, message: "Method not found".to_string() })
    } else {
        None
    };

    RPCResponse {
        id: req.id,
        jsonrpc: "2.0".to_string(),
        result,
        error,
    }
}
