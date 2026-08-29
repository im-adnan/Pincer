//! tellActive, tellWaiting, tellStopped, tellStatus, getGlobalStat
//!
//! ### Architectural Overview
//! - **What it does**: Handles RPC query endpoints inspecting active tasks, waiting queues, stopped results, individual task GID status, and global aggregate bandwidth statistics.
//! - **How it does**: Extracts optional GID query arguments, calls `manager.get_active_tasks()`, `manager.get_waiting_tasks()`, `manager.get_stopped_tasks()`, `manager.get_task()`, or `manager.get_global_stat()`, and converts results to JSON values.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for read-only inspection methods.
//! - **Where it leads to**: Returns structured task arrays and bandwidth records to UI dashboards and polling clients.

use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use serde_json::Value;
use std::sync::Arc;

/// Handles JSON-RPC read-only query methods (`tellActive`, `tellStatus`, `getGlobalStat`, etc.).
pub struct TaskQueryHandlers;

impl TaskQueryHandlers {
    /// Handles `pin.tellActive` RPC method.
    pub async fn handle_tell_active(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let active = manager.get_active_tasks().await;
        Some(serde_json::to_value(active).unwrap())
    }

    /// Handles `pin.tellWaiting` RPC method.
    pub async fn handle_tell_waiting(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let waiting = manager.get_waiting_tasks(0, 100).await;
        Some(serde_json::to_value(waiting).unwrap())
    }

    /// Handles `pin.tellStopped` RPC method.
    pub async fn handle_tell_stopped(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let stopped = manager.get_stopped_tasks(0, 100).await;
        Some(serde_json::to_value(stopped).unwrap())
    }

    /// Handles `pin.tellStatus` RPC method for a specific task GID.
    pub async fn handle_tell_status(
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
                    let status = manager.get_task(gid).await;
                    return Some(serde_json::to_value(status).unwrap());
                }
            }
        }
        None
    }

    /// Handles `pin.getGlobalStat` RPC method returning total speeds and task counts.
    pub async fn handle_get_global_stat(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let stat = manager.get_global_stat().await;
        Some(serde_json::to_value(stat).unwrap())
    }
}
