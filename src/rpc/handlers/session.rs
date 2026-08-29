//! getSessionInfo, getVersion, saveSession, purgeDownloadResult, removeDownloadResult
//!
//! ### Architectural Overview
//! - **What it does**: Handles session management and result maintenance RPC methods (`getSessionInfo`, `getVersion`, `saveSession`, `purgeDownloadResult`, `removeDownloadResult`).
//! - **How it does**: Generates unique session UUIDs, retrieves package versions via `manager.get_version()`, writes sessions via `manager.save_session()`, and purges completed/error results.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for session-related requests.
//! - **Where it leads to**: Performs housekeeping on task registries and returns session metadata to client applications.

use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use serde_json::{json, Value};
use std::sync::Arc;

/// Handles JSON-RPC session information and result maintenance methods.
pub struct SessionRpcHandlers;

impl SessionRpcHandlers {
    /// Handles `pin.getSessionInfo` RPC method returning a unique session UUID.
    pub async fn handle_get_session_info(_req: &RPCRequest) -> Option<Value> {
        Some(json!({
            "sessionId": uuid::Uuid::new_v4().to_string()
        }))
    }

    /// Handles `pin.getVersion` RPC method returning compiled engine version.
    pub async fn handle_get_version(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let version = manager.get_version();
        Some(json!({
            "version": version
        }))
    }

    /// Handles `pin.saveSession` RPC method writing tasks and options to disk.
    pub async fn handle_save_session(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        manager.save_session().await;
        Some(json!("OK"))
    }

    /// Handles `pin.purgeDownloadResult` RPC method clearing completed/error tasks from memory.
    pub async fn handle_purge_download_result(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        manager.purge_download_result().await;
        Some(json!("OK"))
    }

    /// Handles `pin.removeDownloadResult` RPC method removing an individual stopped task.
    pub async fn handle_remove_download_result(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
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
                    return Some(serde_json::to_value(success).unwrap());
                }
            }
        }
        None
    }
}
