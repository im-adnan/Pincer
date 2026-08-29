//! changeOption, getOption, changeGlobalOption, getGlobalOption
//!
//! ### Architectural Overview
//! - **What it does**: Handles RPC endpoints querying and modifying configuration options globally or for specific task GIDs.
//! - **How it does**: Extracts stringified option key-value maps from `req.params`, invoking `manager.change_global_option()`, `manager.get_global_option()`, `manager.change_option()`, or `manager.get_option()`.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()` for options management methods.
//! - **Where it leads to**: Updates running configuration, applies speed limits and download directories dynamically, and returns confirmations or option maps to the client.

use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Handles JSON-RPC option inspection and mutation methods.
pub struct OptionsRpcHandlers;

impl OptionsRpcHandlers {
    /// Handles `pin.changeGlobalOption` RPC method.
    pub async fn handle_change_global_option(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
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
                    return Some(json!("OK"));
                }
            }
        }
        None
    }

    /// Handles `pin.getGlobalOption` RPC method.
    pub async fn handle_get_global_option(
        _req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        let opts = manager.get_global_option().await;
        Some(serde_json::to_value(opts).unwrap())
    }

    /// Handles `pin.changeOption` RPC method for a specific task GID.
    pub async fn handle_change_option(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
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
                    return Some(serde_json::to_value(success).unwrap());
                }
            }
        }
        None
    }

    /// Handles `pin.getOption` RPC method for a specific task GID.
    pub async fn handle_get_option(
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
                    let opts = manager.get_option(gid).await;
                    return Some(serde_json::to_value(opts).unwrap());
                }
            }
        }
        None
    }
}
