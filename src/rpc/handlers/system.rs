//! system.listMethods, system.listNotifications, system.multicall, shutdown
//!
//! ### Architectural Overview
//! - **What it does**: Implements XML-RPC/JSON-RPC standard system reflection endpoints (`system.listMethods`, `system.listNotifications`, `system.multicall`) and server shutdown.
//! - **How it does**: Returns static arrays of supported RPC methods and notifications, executes batch multi-call arrays recursively via a higher-order dispatch closure, and schedules graceful exit via `std::process::exit(0)`.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for `system.*` reflection and shutdown methods.
//! - **Where it leads to**: Returns introspection arrays or batch multicall results to clients and terminates the server process when shutdown is invoked.

use crate::manager::DownloadManager;
use crate::models::{RPCRequest, RPCResponse};
use serde_json::{json, Value};
use std::sync::Arc;

/// Handles system-level reflection, batch multicall executions, and graceful server shutdown.
pub struct SystemRpcHandlers;

impl SystemRpcHandlers {
    /// Returns the complete array of supported JSON-RPC 2.0 method strings.
    pub fn list_methods() -> Value {
        json!([
            "pin.addUri",
            "pin.addTorrent",
            "pin.addMetalink",
            "pin.remove",
            "pin.removeAndFile",
            "pin.forceRemove",
            "pin.pause",
            "pin.unpause",
            "pin.pauseAll",
            "pin.unpauseAll",
            "pin.resolveUrl",
            "pin.resolveTorrent",
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
        ])
    }

    /// Returns the array of real-time JSON-RPC 2.0 lifecycle notification strings.
    pub fn list_notifications() -> Value {
        json!([
            "pin.onDownloadStart",
            "pin.onDownloadPause",
            "pin.onDownloadComplete",
            "pin.onDownloadError",
            "pin.onDownloadProgress"
        ])
    }

    /// Handles `pin.shutdown` RPC method, terminating the server process gracefully after a short delay.
    pub async fn handle_shutdown() -> Option<Value> {
        println!("Received shutdown command. Gracefully shutting down...");
        tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            std::process::exit(0);
        });
        Some(json!("OK"))
    }

    /// Handles `system.multicall` RPC method, executing multiple JSON-RPC calls in a single batch request.
    pub async fn handle_multicall<F, Fut>(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
        dispatch_fn: F,
    ) -> Option<Value>
    where
        F: Fn(RPCRequest, Arc<DownloadManager>) -> Fut,
        Fut: std::future::Future<Output = RPCResponse<Value>>,
    {
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

                            let res = dispatch_fn(sub_req, manager.clone()).await;
                            if let Some(r) = res.result {
                                results.push(json!([r]));
                            } else if let Some(e) = res.error {
                                results.push(serde_json::to_value(e).unwrap());
                            }
                        }
                    }
                    return Some(serde_json::to_value(results).unwrap());
                }
            }
        }
        None
    }
}
