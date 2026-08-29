//! pause, unpause, pauseAll, unpauseAll, remove, removeAndFile, forceRemove
//!
//! ### Architectural Overview
//! - **What it does**: Handles RPC endpoints for pausing, resuming, removing, and trashing download tasks.
//! - **How it does**: Extracts target GIDs from `req.params` (accounting for optional authentication tokens) and delegates to `manager.pause_task()`, `manager.unpause_task()`, `manager.remove_task()`, etc.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for task state modification methods.
//! - **Where it leads to**: Executes lifecycle mutations on active/waiting/paused tasks and returns boolean or status confirmations to the client.

use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use serde_json::{json, Value};
use std::sync::Arc;

/// Handles JSON-RPC lifecycle modification methods (`pause`, `unpause`, `remove`, etc.).
pub struct TaskLifecycleHandlers;

impl TaskLifecycleHandlers {
    /// Extracts the target task GID from request parameters, skipping any leading authentication token.
    fn extract_gid(req: &RPCRequest) -> Option<&str> {
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                if params_array.len() >= 2
                    && params_array[0].is_string()
                    && params_array[0].as_str().unwrap().contains(':')
                {
                    params_array.get(1).and_then(|v| v.as_str())
                } else {
                    params_array.first().and_then(|v| v.as_str())
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Handles `pin.pause` RPC method.
    pub async fn handle_pause(req: &RPCRequest, manager: &Arc<DownloadManager>) -> Option<Value> {
        let gid = Self::extract_gid(req)?;
        let success = manager.pause_task(gid).await;
        Some(serde_json::to_value(success).unwrap())
    }

    /// Handles `pin.pauseAll` RPC method.
    pub async fn handle_pause_all(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        manager.pause_all_tasks().await;
        Some(json!("OK"))
    }

    /// Handles `pin.unpause` RPC method.
    pub async fn handle_unpause(req: &RPCRequest, manager: &Arc<DownloadManager>) -> Option<Value> {
        let gid = Self::extract_gid(req)?;
        let success = manager.unpause_task(gid).await;
        Some(serde_json::to_value(success).unwrap())
    }

    /// Handles `pin.unpauseAll` RPC method.
    pub async fn handle_unpause_all(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        manager.unpause_all_tasks().await;
        Some(json!("OK"))
    }

    /// Handles `pin.remove` RPC method.
    pub async fn handle_remove(req: &RPCRequest, manager: &Arc<DownloadManager>) -> Option<Value> {
        let gid = Self::extract_gid(req)?;
        let success = manager.remove_task(gid).await;
        Some(serde_json::to_value(success).unwrap())
    }

    /// Handles `pin.removeAndFile` RPC method (moving files to Trash).
    pub async fn handle_remove_and_file(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let gid = Self::extract_gid(req)?;
        let success = manager.remove_task_and_file(gid).await;
        Some(serde_json::to_value(success).unwrap())
    }

    /// Handles `pin.forceRemove` RPC method.
    pub async fn handle_force_remove(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let gid = Self::extract_gid(req)?;
        let success = manager.force_remove_task(gid).await;
        Some(serde_json::to_value(success).unwrap())
    }
}
