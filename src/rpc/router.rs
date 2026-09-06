//! Method routing table dispatch
//!
//! ### Architectural Overview
//! - **What it does**: Dispatches incoming JSON-RPC 2.0 method strings to their respective specialized handler implementations.
//! - **How it does**: Matches method strings against known RPC methods (`pin.addUri`, `pin.pause`, `pin.tellStatus`, etc.) and system reflection methods (`system.multicall`, `system.listMethods`), returning standard `RPCResponse` with result or code -32601 on unknown methods.
//! - **Where it comes from**: Called by `rpc::WebSocketHandler::handle()` for each incoming parsed request.
//! - **Where it leads to**: Delegates to modular handler classes under `rpc::handlers::*` and packages responses to send back to the client.

use super::handlers::{
    IntrospectionHandlers, OptionsRpcHandlers, ResolveHandlers, SessionRpcHandlers,
    SystemRpcHandlers, TaskControlHandlers, TaskLifecycleHandlers, TaskQueryHandlers,
};
use crate::manager::DownloadManager;
use crate::models::{RPCError, RPCRequest, RPCResponse};
use std::sync::Arc;

/// Method router table mapping JSON-RPC method strings to domain handler executions.
pub struct MethodRouter;

impl MethodRouter {
    /// Routes an incoming parsed `RPCRequest` to the matching handler function.
    pub async fn dispatch(
        req: RPCRequest,
        manager: Arc<DownloadManager>,
    ) -> RPCResponse<serde_json::Value> {
        let method = req.method.as_str();

        let result = match method {
            // Task Query
            "pin.tellActive" => TaskQueryHandlers::handle_tell_active(&req, &manager).await,
            "pin.tellWaiting" => TaskQueryHandlers::handle_tell_waiting(&req, &manager).await,
            "pin.tellStopped" => TaskQueryHandlers::handle_tell_stopped(&req, &manager).await,
            "pin.tellStatus" => TaskQueryHandlers::handle_tell_status(&req, &manager).await,
            "pin.getGlobalStat" => TaskQueryHandlers::handle_get_global_stat(&req, &manager).await,

            // Task Lifecycle
            "pin.pause" => TaskLifecycleHandlers::handle_pause(&req, &manager).await,
            "pin.pauseAll" => TaskLifecycleHandlers::handle_pause_all(&req, &manager).await,
            "pin.unpause" => TaskLifecycleHandlers::handle_unpause(&req, &manager).await,
            "pin.unpauseAll" => TaskLifecycleHandlers::handle_unpause_all(&req, &manager).await,
            "pin.remove" => TaskLifecycleHandlers::handle_remove(&req, &manager).await,
            "pin.removeAndFile" => {
                TaskLifecycleHandlers::handle_remove_and_file(&req, &manager).await
            }
            "pin.forceRemove" => TaskLifecycleHandlers::handle_force_remove(&req, &manager).await,

            // Task Control & Adding
            "pin.addUri" => TaskControlHandlers::handle_add_uri(&req, &manager).await,
            "pin.addTorrent" => TaskControlHandlers::handle_add_torrent(&req, &manager).await,
            "pin.addMetalink" => TaskControlHandlers::handle_add_metalink(&req, &manager).await,
            "pin.mergeFiles" => TaskControlHandlers::handle_merge_files(&req, &manager).await,

            // Options
            "pin.changeGlobalOption" => {
                OptionsRpcHandlers::handle_change_global_option(&req, &manager).await
            }
            "pin.getGlobalOption" => {
                OptionsRpcHandlers::handle_get_global_option(&req, &manager).await
            }
            "pin.changeOption" => OptionsRpcHandlers::handle_change_option(&req, &manager).await,
            "pin.getOption" => OptionsRpcHandlers::handle_get_option(&req, &manager).await,

            // Introspection
            "pin.changePosition" => IntrospectionHandlers::handle_change_position(&req).await,
            "pin.changeUri" => IntrospectionHandlers::handle_change_uri(&req, &manager).await,
            "pin.getFiles" => IntrospectionHandlers::handle_get_files(&req, &manager).await,
            "pin.getUris" => IntrospectionHandlers::handle_get_uris(&req, &manager).await,
            "pin.getServers" => IntrospectionHandlers::handle_get_servers(&req, &manager).await,

            // Metadata Resolution
            "pin.resolveUrl" => ResolveHandlers::handle_resolve_url(&req, &manager).await,
            "pin.resolveTorrent" => ResolveHandlers::handle_resolve_torrent(&req, &manager).await,

            // Session & Info
            "pin.getSessionInfo" => SessionRpcHandlers::handle_get_session_info(&req).await,
            "pin.getVersion" => SessionRpcHandlers::handle_get_version(&req, &manager).await,
            "pin.saveSession" => SessionRpcHandlers::handle_save_session(&req, &manager).await,
            "pin.purgeDownloadResult" => {
                SessionRpcHandlers::handle_purge_download_result(&req, &manager).await
            }
            "pin.removeDownloadResult" => {
                SessionRpcHandlers::handle_remove_download_result(&req, &manager).await
            }
            "pin.shutdown" => SystemRpcHandlers::handle_shutdown().await,

            // System Reflection
            "system.listMethods" => Some(SystemRpcHandlers::list_methods()),
            "system.listNotifications" => Some(SystemRpcHandlers::list_notifications()),
            "system.multicall" => {
                SystemRpcHandlers::handle_multicall(&req, &manager, |sub_req, mgr| {
                    Box::pin(Self::dispatch(sub_req, mgr))
                })
                .await
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
    }
}
