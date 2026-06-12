use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
    routing::get,
    Extension, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::manager::DownloadManager;
use crate::models::{RPCError, RPCRequest, RPCResponse};

pub async fn start_server(
    manager: Arc<DownloadManager>,
    _rx: broadcast::Receiver<String>,
    port: u16,
    rpc_secret: Option<String>,
) {
    let secret = Arc::new(rpc_secret);
    let app = Router::new()
        .route("/jsonrpc", get(ws_handler))
        .layer(Extension(manager))
        .layer(Extension(secret));

    let addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: Failed to bind to port {}: {}. The port might already be in use by another instance or service.", port, e);
            std::process::exit(1);
        }
    };
    println!("Pincer listening on ws://{}/jsonrpc", addr);

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("RPC Server Error: {}", e);
    }
    println!("RPC Server has shut down.");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(manager): Extension<Arc<DownloadManager>>,
    Extension(secret): Extension<Arc<Option<String>>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, manager, secret))
}

/// Core multiplexer for an individual WebSocket connection.
/// It concurrently parses incoming JSON-RPC payloads (routing them to `handle_method`)
/// and pushes outbound system notifications (from the global broadcast channel) back to the client.
async fn handle_socket(
    socket: WebSocket,
    manager: Arc<DownloadManager>,
    secret: Arc<Option<String>>,
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
                            if let Some(expected_secret) = &*secret {
                                let mut authenticated = false;
                                if let Some(params) = &req.params {
                                    if let Some(params_array) = params.as_array() {
                                        if !params_array.is_empty() && params_array[0].is_string() {
                                            let token_str = params_array[0].as_str().unwrap();
                                            if token_str == format!("token:{}", expected_secret) {
                                                authenticated = true;
                                            }
                                        }
                                    }
                                }
                                if !authenticated {
                                    let error_res = RPCResponse::<serde_json::Value> {
                                        jsonrpc: "2.0".to_string(),
                                        id: req.id.clone(),
                                        result: None,
                                        error: Some(RPCError {
                                            code: 1,
                                            message: "Unauthorized".to_string(),
                                        }),
                                    };
                                    let _ = sender.send(Message::Text(serde_json::to_string(&error_res).unwrap())).await;
                                    continue;
                                }
                            }

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

/// The primary JSON-RPC router.
/// Parses the `method` string (e.g., `pin.addUri`) and maps the parameters
/// directly to the corresponding asynchronous operation on the `DownloadManager`.
fn handle_method<'a>(
    req: RPCRequest,
    manager: &'a Arc<DownloadManager>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = RPCResponse<serde_json::Value>> + Send + 'a>>
{
    Box::pin(async move {
        let method = req.method.as_str();

        let result = match method {
            "pin.tellActive" => {
                let active = manager.get_active_tasks().await;
                Some(serde_json::to_value(active).unwrap())
            }
            "pin.tellWaiting" => {
                let waiting = manager.get_waiting_tasks(0, 100).await;
                Some(serde_json::to_value(waiting).unwrap())
            }
            "pin.tellStopped" => {
                let stopped = manager.get_stopped_tasks(0, 100).await;
                Some(serde_json::to_value(stopped).unwrap())
            }
            "pin.tellStatus" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            // Format: ["token:secret", "gid"]
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            // Format: ["gid"]
                            params_array.first().and_then(|v| v.as_str())
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
            }
            "pin.getGlobalStat" => {
                let stat = manager.get_global_stat().await;
                Some(serde_json::to_value(stat).unwrap())
            }
            "pin.pause" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
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
            }
            "pin.pauseAll" => {
                manager.pause_all_tasks().await;
                Some(json!("OK"))
            }
            "pin.unpause" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
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
            }
            "pin.unpauseAll" => {
                manager.unpause_all_tasks().await;
                Some(json!("OK"))
            }
            "pin.remove" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
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
            }
            "pin.removeAndFile" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
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
            }
            "pin.forceRemove" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
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
            }
            "pin.addUri" => {
                let mut return_ids = Vec::new();
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        if params_array.len() >= 2 {
                            let (uris_val, options_val) = if params_array.len() >= 3
                                && params_array[0].is_string()
                                && params_array[0].as_str().unwrap().contains(':')
                            {
                                (&params_array[1], &params_array[2])
                            } else {
                                (&params_array[0], &params_array[1])
                            };

                            if let (Some(uris), Some(options)) =
                                (uris_val.as_array(), options_val.as_object())
                            {
                                if let Some(first_uri) = uris.first().and_then(|v| v.as_str()) {
                                    let expanded_urls = crate::utils::expand_uris(first_uri);
                                    let explicit_out = options.get("out").and_then(|v| v.as_str());

                                    for url in expanded_urls {
                                        let current_id = uuid::Uuid::new_v4().to_string();
                                        return_ids.push(current_id.clone());

                                        let default_dir = std::env::var("HOME")
                                            .map(|h| format!("{}/Downloads", h))
                                            .unwrap_or_else(|_| "/tmp".to_string());

                                        let dir = options
                                            .get("dir")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or(&default_dir)
                                            .to_string();

                                        let filename = explicit_out
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| {
                                                url.split('/')
                                                    .next_back()
                                                    .unwrap_or("download.bin")
                                                    .split('?')
                                                    .next()
                                                    .unwrap_or("download.bin")
                                                    .to_string()
                                            });

                                        let threads = options
                                            .get("split")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| s.parse::<usize>().ok())
                                            .unwrap_or_else(|| {
                                                manager
                                                    .default_split
                                                    .load(std::sync::atomic::Ordering::Relaxed)
                                                    as usize
                                            });

                                        let mut headers = Vec::new();
                                        if let Some(header_str) =
                                            options.get("header").and_then(|v| v.as_str())
                                        {
                                            for line in header_str.split('\n') {
                                                if !line.trim().is_empty() {
                                                    headers.push(line.trim().to_string());
                                                }
                                            }
                                        }

                                        manager
                                            .spawn_task(
                                                current_id,
                                                vec![url],
                                                filename,
                                                dir,
                                                threads,
                                                0,
                                                headers,
                                                None,
                                            )
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                }

                if return_ids.is_empty() {
                    let fallback_id = uuid::Uuid::new_v4().to_string();
                    Some(serde_json::to_value(fallback_id).unwrap())
                } else if return_ids.len() == 1 {
                    Some(serde_json::to_value(&return_ids[0]).unwrap())
                } else {
                    Some(serde_json::to_value(return_ids).unwrap())
                }
            }
            "pin.addTorrent" => {
                // Stub for now, returns error or empty success
                Some(json!("NOT_IMPLEMENTED_YET"))
            }
            "pin.addMetalink" => {
                use base64::{engine::general_purpose, Engine as _};
                let mut return_ids = Vec::new();
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        if !params_array.is_empty() {
                            let (base64_val, options_val) = if params_array.len() >= 2
                                && params_array[0].is_string()
                                && params_array[0].as_str().unwrap().contains(':')
                            {
                                (&params_array[1], params_array.get(2))
                            } else {
                                (&params_array[0], params_array.get(1))
                            };

                            if let Some(base64_str) = base64_val.as_str() {
                                if let Ok(decoded) = general_purpose::STANDARD.decode(base64_str) {
                                    if let Ok(xml_str) = String::from_utf8(decoded) {
                                        if let Ok(metalink_files) =
                                            crate::metalink::parse_metalink(&xml_str)
                                        {
                                            let options = options_val.and_then(|v| v.as_object());

                                            let default_dir = std::env::var("HOME")
                                                .map(|h| format!("{}/Downloads", h))
                                                .unwrap_or_else(|_| "/tmp".to_string());
                                            let dir = options
                                                .and_then(|o| o.get("dir"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(&default_dir)
                                                .to_string();

                                            let split_default = manager
                                                .default_split
                                                .load(std::sync::atomic::Ordering::Relaxed)
                                                .max(1)
                                                as usize;
                                            let threads = options
                                                .and_then(|o| o.get("split"))
                                                .and_then(|v| v.as_str())
                                                .and_then(|v| v.parse::<usize>().ok())
                                                .unwrap_or(split_default);

                                            let explicit_out = options
                                                .and_then(|o| o.get("out"))
                                                .and_then(|v| v.as_str());
                                            let mut headers = Vec::new();
                                            if let Some(header_str) = options
                                                .and_then(|o| o.get("header"))
                                                .and_then(|v| v.as_str())
                                            {
                                                for line in header_str.split('\n') {
                                                    if !line.trim().is_empty() {
                                                        headers.push(line.trim().to_string());
                                                    }
                                                }
                                            }

                                            for (i, mfile) in metalink_files.into_iter().enumerate()
                                            {
                                                let current_id = uuid::Uuid::new_v4().to_string();
                                                return_ids.push(current_id.clone());

                                                let filename = if i == 0 && explicit_out.is_some() {
                                                    if let Some(out) = explicit_out {
                                                        out.to_string()
                                                    } else {
                                                        unreachable!()
                                                    }
                                                } else {
                                                    mfile.name.unwrap_or_else(|| {
                                                        mfile
                                                            .urls
                                                            .first()
                                                            .and_then(|u| u.split('/').next_back())
                                                            .unwrap_or("download.bin")
                                                            .to_string()
                                                    })
                                                };

                                                manager
                                                    .spawn_task(
                                                        current_id,
                                                        mfile.urls,
                                                        filename,
                                                        dir.clone(),
                                                        threads,
                                                        0,
                                                        headers.clone(),
                                                        mfile.hash_sha256,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if return_ids.is_empty() {
                    let fallback_id = uuid::Uuid::new_v4().to_string();
                    Some(serde_json::to_value(fallback_id).unwrap())
                } else if return_ids.len() == 1 {
                    Some(serde_json::to_value(&return_ids[0]).unwrap())
                } else {
                    Some(serde_json::to_value(return_ids).unwrap())
                }
            }
            "pin.changeGlobalOption" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let options_val = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1)
                        } else {
                            params_array.first()
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
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.getGlobalOption" => {
                let opts = manager.get_global_option().await;
                Some(serde_json::to_value(opts).unwrap())
            }
            "pin.changeOption" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let (gid, options_val) = if params_array.len() >= 3
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            (
                                params_array.get(1).and_then(|v| v.as_str()),
                                params_array.get(2),
                            )
                        } else {
                            (
                                params_array.first().and_then(|v| v.as_str()),
                                params_array.get(1),
                            )
                        };

                        if let (Some(gid), Some(options_obj)) =
                            (gid, options_val.and_then(|v| v.as_object()))
                        {
                            let mut opts = HashMap::new();
                            for (k, v) in options_obj {
                                if let Some(s) = v.as_str() {
                                    opts.insert(k.clone(), s.to_string());
                                }
                            }
                            let success = manager.change_option(gid, opts).await;
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
            }
            "pin.getOption" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
                        };

                        if let Some(gid) = gid {
                            let opts = manager.get_option(gid).await;
                            Some(serde_json::to_value(opts).unwrap())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.purgeDownloadResult" => {
                manager.purge_download_result().await;
                Some(json!("OK"))
            }
            "pin.removeDownloadResult" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
                        };

                        if let Some(gid) = gid {
                            let success = manager.remove_download_result(gid).await;
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
            }
            "pin.resolveUrl" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let url = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
                        };

                        if let Some(url) = url {
                            match manager.resolve_url(url.to_string()).await {
                                Ok(res) => Some(serde_json::to_value(res).unwrap()),
                                Err(e) => Some(json!({
                                    "error": e
                                })),
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.changePosition" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let has_token = !params_array.is_empty()
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':');

                        let offset = if has_token { 1 } else { 0 };

                        if params_array.len() >= offset + 3 {
                            Some(json!(0))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.changeUri" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let has_token = !params_array.is_empty()
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':');

                        let offset = if has_token { 1 } else { 0 };

                        if params_array.len() >= offset + 4 {
                            let gid = params_array[offset].as_str().unwrap_or("");
                            let file_index = if let Some(s) = params_array[offset + 1].as_str() {
                                s.parse::<usize>().unwrap_or(0)
                            } else {
                                params_array[offset + 1].as_u64().unwrap_or(0) as usize
                            };

                            let del_uris: Vec<String> = params_array[offset + 2]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();

                            let add_uris: Vec<String> = params_array[offset + 3]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();

                            match manager
                                .change_uri(gid, file_index, del_uris, add_uris)
                                .await
                            {
                                Ok((del, add)) => Some(json!([del, add])),
                                Err(e) => Some(json!({"error": e})),
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.getFiles" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
                        };

                        if let Some(gid) = gid {
                            if let Some(status) = manager.get_task(gid).await {
                                use crate::models::{PincerFile, PincerUri};
                                let mut files = Vec::new();
                                for (i, file_data) in status.files.into_iter().enumerate() {
                                    let uris = file_data
                                        .uris
                                        .into_iter()
                                        .map(|u| PincerUri {
                                            uri: u.uri,
                                            status: "used".to_string(),
                                        })
                                        .collect();

                                    files.push(PincerFile {
                                        index: (i + 1).to_string(),
                                        path: file_data.path,
                                        length: status.total_length.clone(),
                                        completed_length: status.completed_length.clone(),
                                        selected: "true".to_string(),
                                        uris,
                                    });
                                }
                                Some(serde_json::to_value(files).unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.getUris" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
                        };

                        if let Some(gid) = gid {
                            if let Some(status) = manager.get_task(gid).await {
                                use crate::models::PincerUri;
                                let mut uris = Vec::new();
                                for file_data in status.files {
                                    for u in file_data.uris {
                                        uris.push(PincerUri {
                                            uri: u.uri,
                                            status: "used".to_string(),
                                        });
                                    }
                                }
                                Some(serde_json::to_value(uris).unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.getServers" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let gid = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_str())
                        } else {
                            params_array.first().and_then(|v| v.as_str())
                        };

                        if let Some(gid) = gid {
                            if let Some(status) = manager.get_task(gid).await {
                                use crate::models::{PincerServer, PincerServerItem};
                                let mut servers = Vec::new();
                                for (i, file_data) in status.files.into_iter().enumerate() {
                                    let mut items = Vec::new();
                                    for u in file_data.uris {
                                        items.push(PincerServerItem {
                                            uri: u.uri.clone(),
                                            current_uri: u.uri,
                                            download_speed: status.download_speed.clone(),
                                        });
                                    }
                                    servers.push(PincerServer {
                                        index: (i + 1).to_string(),
                                        servers: items,
                                    });
                                }
                                Some(serde_json::to_value(servers).unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "pin.getSessionInfo" => Some(json!({
                "sessionId": uuid::Uuid::new_v4().to_string()
            })),
            "pin.getVersion" => {
                let version = manager.get_version();
                Some(json!({
                    "version": version
                }))
            }
            "pin.saveSession" => {
                manager.save_session().await;
                Some(json!("OK"))
            }
            "pin.shutdown" => {
                println!("Received shutdown command. Gracefully shutting down...");
                tokio::spawn(async {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    std::process::exit(0);
                });
                Some(json!("OK"))
            }
            "system.listMethods" => Some(json!([
                "pin.addUri",
                "pin.addTorrent",
                "pin.remove",
                "pin.removeAndFile",
                "pin.forceRemove",
                "pin.pause",
                "pin.unpause",
                "pin.pauseAll",
                "pin.unpauseAll",
                "pin.resolveUrl",
                "pin.tellStatus",
                "pin.tellActive",
                "pin.tellWaiting",
                "pin.tellStopped",
                "pin.getGlobalStat",
                "pin.changeOption",
                "pin.getOption",
                "pin.changeGlobalOption",
                "pin.getGlobalOption",
                "pin.getFiles",
                "pin.getUris",
                "pin.getServers",
                "pin.getSessionInfo",
                "pin.purgeDownloadResult",
                "pin.removeDownloadResult",
                "pin.getVersion",
                "pin.saveSession",
                "pin.shutdown",
                "system.listMethods",
                "system.listNotifications",
                "system.multicall"
            ])),
            "system.listNotifications" => Some(json!([
                "pin.onDownloadStart",
                "pin.onDownloadPause",
                "pin.onDownloadComplete",
                "pin.onDownloadError"
            ])),
            "system.multicall" => {
                if let Some(params) = &req.params {
                    if let Some(params_array) = params.as_array() {
                        let multicall_array = if params_array.len() >= 2
                            && params_array[0].is_string()
                            && params_array[0].as_str().unwrap().contains(':')
                        {
                            params_array.get(1).and_then(|v| v.as_array())
                        } else {
                            params_array.first().and_then(|v| v.as_array())
                        };

                        if let Some(calls) = multicall_array {
                            let mut results = Vec::new();
                            for call in calls {
                                if let Some(call_obj) = call.as_object() {
                                    let method_name = call_obj
                                        .get("methodName")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let call_params = call_obj.get("params").cloned();

                                    let sub_req = RPCRequest {
                                        jsonrpc: "2.0".to_string(),
                                        id: "1".to_string(),
                                        method: method_name,
                                        params: call_params,
                                    };

                                    let res = handle_method(sub_req, manager).await;
                                    if let Some(r) = res.result {
                                        results.push(json!([r]));
                                    } else if let Some(e) = res.error {
                                        results.push(serde_json::to_value(e).unwrap());
                                    }
                                }
                            }
                            Some(serde_json::to_value(results).unwrap())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        let error = if result.is_none() {
            Some(RPCError {
                code: -32601,
                message: "Method not found".to_string(),
            })
        } else {
            None
        };

        RPCResponse {
            id: req.id,
            jsonrpc: "2.0".to_string(),
            result,
            error,
        }
    })
}
