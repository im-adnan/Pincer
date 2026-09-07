//! resolveUrl, resolveTorrent
//!
//! ### Architectural Overview
//! - **What it does**: Handles URL and torrent inspection RPC endpoints (`pin.resolveUrl`, `pin.resolveTorrent`) probing remote endpoints for metadata without downloading files.
//! - **How it does**: Extracts the URL or Base64 torrent string from parameters and delegates to `manager.resolve_url()` or `manager.resolve_torrent_base64()`.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for resolution methods.
//! - **Where it leads to**: Returns structured `ResolveResponse` objects detailing inferred filenames, sizes, and torrent file trees.

use crate::manager::DownloadManager;
use crate::models::{RPCError, RPCRequest};
use serde_json::{json, Value};
use std::sync::Arc;

/// Handles JSON-RPC metadata resolution endpoints (`resolveUrl`, `resolveTorrent`).
pub struct ResolveHandlers;

impl ResolveHandlers {
    /// Handles `pin.resolveUrl` RPC method, probing remote endpoints for metadata.
    pub async fn handle_resolve_url(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                let url = if params_array.len() >= 2
                    && params_array[0].is_string()
                    && params_array[0].as_str().unwrap_or("").contains(':')
                {
                    params_array.get(1).and_then(|v| v.as_str())
                } else {
                    params_array.first().and_then(|v| v.as_str())
                };

                if let Some(url) = url {
                    return match manager.resolve_url(url.to_string()).await {
                        Ok(res) => Ok(serde_json::to_value(res).unwrap_or(Value::Null)),
                        Err(e) => Ok(json!({ "error": e })),
                    };
                }
            }
        }
        Err(RPCError {
            code: -32602,
            message: "Missing URL parameter".to_string(),
        })
    }

    /// Handles `pin.resolveTorrent` RPC method, inspecting Base64 `.torrent` payloads.
    pub async fn handle_resolve_torrent(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                let base64_str = if params_array.len() >= 2
                    && params_array[0].is_string()
                    && params_array[0].as_str().unwrap_or("").contains(':')
                {
                    params_array.get(1).and_then(|v| v.as_str())
                } else {
                    params_array.first().and_then(|v| v.as_str())
                };

                if let Some(base64_str) = base64_str {
                    return match manager.resolve_torrent_base64(base64_str.to_string()).await {
                        Ok(res) => Ok(serde_json::to_value(res).unwrap_or(Value::Null)),
                        Err(e) => Ok(json!({ "error": e })),
                    };
                }
            }
        }
        Err(RPCError {
            code: -32602,
            message: "Missing Base64 payload parameter".to_string(),
        })
    }
}
