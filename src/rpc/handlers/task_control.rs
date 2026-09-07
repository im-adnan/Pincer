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
use crate::models::{RPCError, RPCRequest};
use serde_json::Value;
use std::sync::Arc;

/// Groups task addition handlers for URIs, torrents, and metalinks.
pub struct TaskControlHandlers;

impl TaskControlHandlers {
    /// Handles `pin.addUri` JSON-RPC method.
    pub async fn handle_add_uri(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        AddUriHandler::handle(req, manager).await
    }

    /// Handles `pin.addTorrent` JSON-RPC method.
    pub async fn handle_add_torrent(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        AddTorrentHandler::handle(req, manager).await
    }

    /// Handles `pin.addMetalink` JSON-RPC method.
    pub async fn handle_add_metalink(
        req: &RPCRequest,
        manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        AddMetalinkHandler::handle(req, manager).await
    }

    /// Handles `pin.mergeFiles` JSON-RPC method.
    pub async fn handle_merge_files(
        req: &RPCRequest,
        _manager: &Arc<DownloadManager>,
    ) -> Result<Value, RPCError> {
        let params = req.params.as_ref().ok_or_else(|| RPCError {
            code: -32602,
            message: "Missing params".to_string(),
        })?;

        let mut idx = 0;
        if let Some(first_str) = params.get(0).and_then(|v| v.as_str()) {
            if first_str.starts_with("token:") {
                idx += 1;
            }
        }

        let video_path_str = params
            .get(idx)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RPCError {
                code: -32602,
                message: "Missing video path".to_string(),
            })?;

        let audio_path_str = params
            .get(idx + 1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RPCError {
                code: -32602,
                message: "Missing audio path".to_string(),
            })?;

        let dest_path_str = params
            .get(idx + 2)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RPCError {
                code: -32602,
                message: "Missing dest path".to_string(),
            })?;

        let video_path = std::path::Path::new(video_path_str);
        let audio_path = std::path::Path::new(audio_path_str);
        let dest_path = std::path::Path::new(dest_path_str);

        let success =
            crate::converter::FfmpegConverter::merge(video_path, audio_path, dest_path).await;

        if success {
            Ok(serde_json::json!("OK"))
        } else {
            Err(RPCError {
                code: -32603,
                message: "Merge failed".to_string(),
            })
        }
    }
}
