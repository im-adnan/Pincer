//! addUri, addTorrent, addMetalink dispatcher
//!
//! ### Architectural Overview
//! - **What it does**: Handles RPC endpoints for adding download tasks via direct URIs, BitTorrent payloads, and Metalink files.
//! - **How it does**: Delegates `pin.addUri`, `pin.addTorrent`, and `pin.addMetalink` method calls to specialized sibling handler modules.
//! - **Where it comes from**: Called by `rpc::MethodRouter::dispatch()`.
//! - **Where it leads to**: Dispatches requests to specialized child handlers to register tasks in `DownloadManager`.

use super::add_metalink::AddMetalinkHandler;
use super::add_torrent::AddTorrentHandler;
use super::add_uri::AddUriHandler;
use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use serde_json::Value;
use std::sync::Arc;

/// Groups task addition handlers for URIs, torrents, and metalinks.
pub struct TaskControlHandlers;

impl TaskControlHandlers {
    /// Handles `pin.addUri` JSON-RPC method.
    pub async fn handle_add_uri(req: &RPCRequest, manager: &Arc<DownloadManager>) -> Option<Value> {
        AddUriHandler::handle(req, manager).await
    }

    /// Handles `pin.addTorrent` JSON-RPC method.
    pub async fn handle_add_torrent(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        AddTorrentHandler::handle(req, manager).await
    }

    /// Handles `pin.addMetalink` JSON-RPC method.
    pub async fn handle_add_metalink(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Option<Value> {
        AddMetalinkHandler::handle(req, manager).await
    }
}
