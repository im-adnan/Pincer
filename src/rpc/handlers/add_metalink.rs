//! addMetalink handler
//!
//! ### Architectural Overview
//! - **What it does**: Handles the `pin.addMetalink` JSON-RPC method, decoding Base64 Metalink XML payloads and spawning multi-source download tasks with SHA256 integrity checks.
//! - **How it does**: Decodes Base64 data, parses XML via `metalink::parse_metalink()`, iterates through contained file records, and invokes `manager.spawn_task()` with mirror URLs and target checksums.
//! - **Where it comes from**: Called by `rpc::handlers::TaskControlHandlers::handle_add_metalink()`.
//! - **Where it leads to**: Returns the list of spawned task GIDs in a JSON-RPC response value.

use crate::manager::DownloadManager;
use crate::models::{RPCError, RPCRequest};
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::sync::Arc;

/// Handles JSON-RPC `pin.addMetalink` method invocations.
pub struct AddMetalinkHandler;

impl AddMetalinkHandler {
    /// Decodes Base64 XML, parses Metalink manifests, and spawns corresponding download tasks.
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

                                    let explicit_out =
                                        options.and_then(|o| o.get("out")).and_then(|v| v.as_str());
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

                                    for (i, mfile) in metalink_files.into_iter().enumerate() {
                                        let current_id = uuid::Uuid::new_v4().to_string();
                                        return_ids.push(current_id.clone());

                                        let filename = if i == 0 {
                                            if let Some(out) = explicit_out {
                                                out.to_string()
                                            } else {
                                                mfile.name.unwrap_or_else(|| {
                                                    mfile
                                                        .urls
                                                        .first()
                                                        .and_then(|u| u.split('/').next_back())
                                                        .unwrap_or("download.bin")
                                                        .to_string()
                                                })
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
            Err(RPCError {
                code: -32602,
                message: "Failed to add metalink".to_string(),
            })
        } else if return_ids.len() == 1 {
            Ok(serde_json::to_value(&return_ids[0]).unwrap_or(Value::Null))
        } else {
            Ok(serde_json::to_value(return_ids).unwrap_or(Value::Null))
        }
    }
}
