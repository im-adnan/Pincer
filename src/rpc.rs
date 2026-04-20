use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
    routing::get,
    Router, Extension,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::manager::DownloadManager;
use crate::models::{RPCRequest, RPCResponse, RPCError};

pub async fn start_server(manager: Arc<DownloadManager>, _rx: broadcast::Receiver<String>) {
    let app = Router::new()
        .route("/jsonrpc", get(ws_handler))
        .layer(Extension(manager));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:6842").await.unwrap();
    println!("Pincer listening on ws://127.0.0.1:6842/jsonrpc");
    
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(manager): Extension<Arc<DownloadManager>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, manager))
}

async fn handle_socket(
    socket: WebSocket,
    manager: Arc<DownloadManager>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut notification_rx = manager.subscribe();

    // We use tokio::select! to handle both inbound RPC and outbound notifications
    loop {
        tokio::select! {
            // Inbound RPC requests
            Some(msg_res) = receiver.next() => {
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
            // Outbound notifications (broadcast from DownloadManager)
            Ok(notification) = notification_rx.recv() => {
                let _ = sender.send(Message::Text(notification)).await;
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
        "pin.tellStatus" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        // Format: ["token:secret", "gid"]
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        // Format: ["gid"]
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let status = manager.get_task(gid).await;
                        Some(serde_json::to_value(status).unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        },
        "pin.getGlobalStat" => {
            let stat = manager.get_global_stat().await;
            Some(serde_json::to_value(stat).unwrap())
        },
        "pin.pause" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let success = manager.pause_task(gid).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        },
        "pin.pauseAll" => {
            manager.pause_all_tasks().await;
            Some(json!("OK"))
        },
        "pin.unpause" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let success = manager.unpause_task(gid).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        },
        "pin.unpauseAll" => {
            manager.unpause_all_tasks().await;
            Some(json!("OK"))
        },
        "pin.remove" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let success = manager.remove_task(gid).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        },
        "pin.removeAndFile" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let success = manager.remove_task_and_file(gid).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        },
        "pin.forceRemove" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let success = manager.force_remove_task(gid).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        },
        "pin.addUri" => {
            let id = uuid::Uuid::new_v4().to_string();
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    if params_array.len() >= 2 {
                        let (uris_val, options_val) = if params_array.len() >= 3 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                            (&params_array[1], &params_array[2])
                        } else {
                            (&params_array[0], &params_array[1])
                        };

                        if let (Some(uris), Some(options)) = (uris_val.as_array(), options_val.as_object()) {
                            if let Some(first_uri) = uris.first().and_then(|v| v.as_str()) {
                                let url = first_uri.to_string();
                                let default_dir = std::env::var("HOME")
                                    .map(|h| format!("{}/Downloads", h))
                                    .unwrap_or_else(|_| "/tmp".to_string());

                                let dir = options.get("dir").and_then(|v| v.as_str()).unwrap_or(&default_dir).to_string();
                                let filename = options.get("out").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| {
                                    url.split('/').last().unwrap_or("download.bin").split('?').next().unwrap_or("download.bin").to_string()
                                });
                                let threads = options.get("split").and_then(|v| v.as_str()).and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
                                
                                let mut headers = Vec::new();
                                if let Some(header_str) = options.get("header").and_then(|v| v.as_str()) {
                                    for line in header_str.split('\n') {
                                        if !line.trim().is_empty() {
                                            headers.push(line.trim().to_string());
                                        }
                                    }
                                }
                                
                                manager.spawn_task(id.clone(), url, filename, dir, threads, 0, headers).await;
                            }
                        }
                    }
                }
            }
            Some(serde_json::to_value(id).unwrap())
        },
        "pin.addTorrent" => {
            // Stub for now, returns error or empty success
            Some(json!("NOT_IMPLEMENTED_YET"))
        },
        "pin.changeGlobalOption" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let options_val = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1)
                    } else {
                        params_array.get(0)
                    };

                    if let Some(options_obj) = options_val.and_then(|v| v.as_object()) {
                        let mut opts = HashMap::new();
                        for (k, v) in options_obj {
                            if let Some(s) = v.as_str() {
                                opts.insert(k.clone(), s.to_string());
                            }
                        }
                        manager.change_global_option(opts).await;
                        Some(json!("OK"))
                    } else { None }
                } else { None }
            } else { None }
        },
        "pin.getGlobalOption" => {
            let opts = manager.get_global_option().await;
            Some(serde_json::to_value(opts).unwrap())
        },
        "pin.changeOption" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let (gid, options_val) = if params_array.len() >= 3 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        (params_array.get(1).and_then(|v| v.as_str()), params_array.get(2))
                    } else {
                        (params_array.get(0).and_then(|v| v.as_str()), params_array.get(1))
                    };

                    if let (Some(gid), Some(options_obj)) = (gid, options_val.and_then(|v| v.as_object())) {
                        let mut opts = HashMap::new();
                        for (k, v) in options_obj {
                            if let Some(s) = v.as_str() {
                                opts.insert(k.clone(), s.to_string());
                            }
                        }
                        let success = manager.change_option(gid, opts).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else { None }
                } else { None }
            } else { None }
        },
        "pin.getOption" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let opts = manager.get_option(gid).await;
                        Some(serde_json::to_value(opts).unwrap())
                    } else { None }
                } else { None }
            } else { None }
        },
        "pin.purgeDownloadResult" => {
            manager.purge_download_result().await;
            Some(json!("OK"))
        },
        "pin.removeDownloadResult" => {
            if let Some(params) = &req.params {
                if let Some(params_array) = params.as_array() {
                    let gid = if params_array.len() >= 2 && params_array[0].is_string() && params_array[0].as_str().unwrap().contains(':') {
                        params_array.get(1).and_then(|v| v.as_str())
                    } else {
                        params_array.get(0).and_then(|v| v.as_str())
                    };

                    if let Some(gid) = gid {
                        let success = manager.remove_download_result(gid).await;
                        Some(serde_json::to_value(success).unwrap())
                    } else { None }
                } else { None }
            } else { None }
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
