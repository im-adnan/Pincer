//! addTorrent handler
//!
//! ### Architectural Overview
//! - **What it does**: Handles the `pin.addTorrent` JSON-RPC method, decoding Base64 `.torrent` payloads and spawning background BitTorrent tasks.
//! - **How it does**: Decodes Base64 data with `base64::Engine`, validates torrent structure via `librqbit::torrent_from_bytes`, extracts directory and seeding options, and calls `manager.spawn_torrent_task()`.
//! - **Where it comes from**: Called by `rpc::handlers::TaskControlHandlers::handle_add_torrent()`.
//! - **Where it leads to**: Returns the generated torrent GID in a JSON-RPC response value.

use crate::manager::DownloadManager;
use crate::models::{RPCError, RPCRequest};
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Handles JSON-RPC `pin.addTorrent` method invocations.
pub struct AddTorrentHandler;

impl AddTorrentHandler {
    /// Decodes a Base64 `.torrent` file payload and registers the task with `DownloadManager`.
    pub async fn handle(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        let mut return_ids = Vec::new();
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                if !params_array.is_empty() {
                    let (base64_val, options_val) = if params_array.len() >= 2
                        && params_array[0].is_string()
                        && params_array[0].as_str().unwrap_or("").contains(':')
                    {
                        (&params_array[1], params_array.get(2))
                    } else {
                        (&params_array[0], params_array.get(1))
                    };

                    if let Some(base64_str) = base64_val.as_str() {
                        if let Ok(decoded) = general_purpose::STANDARD.decode(base64_str) {
                            let options = options_val.and_then(|v| v.as_object());
                            let default_dir = std::env::var("HOME")
                                .map(|h| format!("{}/Downloads", h))
                                .unwrap_or_else(|_| "/tmp".to_string());
                            let dir = options
                                .and_then(|o| o.get("dir"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&default_dir)
                                .to_string();

                            let mut opts_map = HashMap::new();
                            if let Some(opts) = options {
                                for (k, v) in opts.iter() {
                                    if let Some(s) = v.as_str() {
                                        opts_map.insert(k.clone(), s.to_string());
                                    }
                                }
                            }

                            let current_id = uuid::Uuid::new_v4().to_string();
                            if librqbit::torrent_from_bytes::<&[u8]>(&decoded).is_ok() {
                                let torrent_source = librqbit::AddTorrent::from_bytes(decoded);
                                if let Ok(task_id) = manager
                                    .spawn_torrent_task(current_id, torrent_source, dir, opts_map)
                                    .await
                                {
                                    return_ids.push(task_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        if return_ids.is_empty() {
            Err(RPCError {
                code: -32602,
                message: "Failed to add torrent".to_string(),
            })
        } else if return_ids.len() == 1 {
            Ok(serde_json::to_value(&return_ids[0]).unwrap_or(Value::Null))
        } else {
            Ok(serde_json::to_value(return_ids).unwrap_or(Value::Null))
        }
    }
}
