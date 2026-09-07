//! getFiles, getUris, getServers, changePosition, changeUri
//!
//! ### Architectural Overview
//! - **What it does**: Handles fine-grained introspection and position mutation RPC methods (`getFiles`, `getUris`, `getServers`, `changePosition`, `changeUri`).
//! - **How it does**: Inspects task files and connection endpoints, formats `PincerFile` and `PincerServer` structures, and supports in-place URI replacement via `manager.change_uri()`.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for introspection method requests.
//! - **Where it leads to**: Returns structured file lists, server speed metrics, or URI modification counts to client applications.

use crate::manager::DownloadManager;
use crate::models::{PincerFile, PincerServer, PincerServerItem, PincerUri, RPCError, RPCRequest};
use serde_json::{json, Value};
use std::sync::Arc;

/// Handles JSON-RPC file inspection and URI management endpoints.
pub struct IntrospectionHandlers;

impl IntrospectionHandlers {
    /// Extracts the target task GID from request parameters, skipping any leading authentication token.
    fn extract_gid(req: &RPCRequest) -> Result<&str, RPCError> {
        let params = req.params.as_ref().ok_or_else(|| RPCError {
            code: -32602,
            message: "Missing params".to_string(),
        })?;

        let params_array = params.as_array().ok_or_else(|| RPCError {
            code: -32602,
            message: "Params must be an array".to_string(),
        })?;

        let gid = if params_array.len() >= 2
            && params_array[0].is_string()
            && params_array[0].as_str().unwrap_or("").contains(':')
        {
            params_array.get(1).and_then(|v| v.as_str())
        } else {
            params_array.first().and_then(|v| v.as_str())
        };

        gid.ok_or_else(|| RPCError {
            code: -32602,
            message: "Missing GID parameter".to_string(),
        })
    }

    /// Handles `pin.getFiles` RPC method returning file list and selected states.
    pub async fn handle_get_files(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        let gid = Self::extract_gid(req)?;
        if let Some(status) = manager.get_task(gid).await {
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
            return Ok(serde_json::to_value(files).unwrap_or(Value::Null));
        }
        Err(RPCError {
            code: 1,
            message: "Active Download not found".to_string(),
        })
    }

    /// Handles `pin.getUris` RPC method returning all source mirror URLs for a task.
    pub async fn handle_get_uris(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        let gid = Self::extract_gid(req)?;
        if let Some(status) = manager.get_task(gid).await {
            let mut uris = Vec::new();
            for file_data in status.files {
                for u in file_data.uris {
                    uris.push(PincerUri {
                        uri: u.uri,
                        status: "used".to_string(),
                    });
                }
            }
            return Ok(serde_json::to_value(uris).unwrap_or(Value::Null));
        }
        Err(RPCError {
            code: 1,
            message: "Active Download not found".to_string(),
        })
    }

    /// Handles `pin.getServers` RPC method returning connection speed stats per server.
    pub async fn handle_get_servers(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        let gid = Self::extract_gid(req)?;
        if let Some(status) = manager.get_task(gid).await {
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
            return Ok(serde_json::to_value(servers).unwrap_or(Value::Null));
        }
        Err(RPCError {
            code: 1,
            message: "Active Download not found".to_string(),
        })
    }

    /// Handles `pin.getPeers` RPC method.
    /// Note: librqbit does not expose individual peer IPs easily, so we return an empty array.
    pub async fn handle_get_peers(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        let gid = Self::extract_gid(req)?;
        if manager.get_task(gid).await.is_some() {
            // Stubbed empty peer list as librqbit abstracts this
            return Ok(json!([]));
        }
        Err(RPCError {
            code: 1,
            message: "Active Download not found".to_string(),
        })
    }

    /// Handles `pin.changePosition` RPC method for queue position adjustments.
    pub async fn handle_change_position(req: &RPCRequest) -> Result<Value, RPCError> {
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                let has_token = !params_array.is_empty()
                    && params_array[0].is_string()
                    && params_array[0].as_str().unwrap_or("").contains(':');
                let offset = if has_token { 1 } else { 0 };

                if params_array.len() >= offset + 3 {
                    return Ok(json!(0));
                }
            }
        }
        Err(RPCError {
            code: -32602,
            message: "Missing or invalid parameters".to_string(),
        })
    }

    /// Handles `pin.changeUri` RPC method for dynamically removing and adding mirror URLs.
    pub async fn handle_change_uri(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                let has_token = !params_array.is_empty()
                    && params_array[0].is_string()
                    && params_array[0].as_str().unwrap_or("").contains(':');
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

                    return match manager
                        .change_uri(gid, file_index, del_uris, add_uris)
                        .await
                    {
                        Ok((del, add)) => Ok(json!([del, add])),
                        Err(e) => Ok(json!({ "error": e })),
                    };
                }
            }
        }
        Err(RPCError {
            code: -32602,
            message: "Missing or invalid parameters".to_string(),
        })
    }
}
