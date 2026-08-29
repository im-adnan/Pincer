//! addUri handler
//!
//! ### Architectural Overview
//! - **What it does**: Handles the `pin.addUri` JSON-RPC method, expanding parameterized URIs, detecting magnet links and `.torrent` URLs, and initiating task creation.
//! - **How it does**: Parses `req.params` (supporting optional token prefixes), performs URI brace expansion via `expand_uris()`, extracts directory/thread/header options, and invokes `manager.spawn_task()` or `manager.spawn_torrent_task()`.
//! - **Where it comes from**: Called by `rpc::handlers::TaskControlHandlers::handle_add_uri()`.
//! - **Where it leads to**: Returns the generated GID (or list of GIDs) in a JSON-RPC response value.

use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Handles JSON-RPC `pin.addUri` method invocations.
pub struct AddUriHandler;

impl AddUriHandler {
    /// Parses parameters and spawns one or more download tasks.
    ///
    /// Request handling pipeline:
    /// 1. Extracts URI array and options object from `req.params` (handling optional auth token).
    /// 2. Performs parameterized URI expansion (e.g. `{1,2}`, `[01-10]`).
    /// 3. Checks each URL:
    ///    - If Magnet link or `.torrent` file, delegates to `manager.spawn_torrent_task()`.
    ///    - Otherwise, extracts headers/threads and spawns standard segmented task via `manager.spawn_task()`.
    /// 4. Returns single GID or array of GIDs to client.
    pub async fn handle(req: &RPCRequest, manager: &Arc<DownloadManager>) -> Option<Value> {
        let mut return_ids = Vec::new();
        if let Some(params) = &req.params {
            if let Some(params_array) = params.as_array() {
                if params_array.len() >= 2 {
                    let (uris_val, options_val) = if params_array.len() >= 3
                        && params_array[0].is_string()
                        && params_array[0].as_str().unwrap().contains(':')
                    {
                        (&params_array[1], &params_array[2])
                    } else {
                        (&params_array[0], &params_array[1])
                    };

                    if let (Some(uris), Some(options)) =
                        (uris_val.as_array(), options_val.as_object())
                    {
                        if let Some(first_uri) = uris.first().and_then(|v| v.as_str()) {
                            let expanded_urls = crate::common::expand_uris(first_uri);
                            let explicit_out = options.get("out").and_then(|v| v.as_str());

                            for url in expanded_urls {
                                let url = url.trim().to_string();
                                let current_id = uuid::Uuid::new_v4().to_string();
                                return_ids.push(current_id.clone());

                                let default_dir = std::env::var("HOME")
                                    .map(|h| format!("{}/Downloads", h))
                                    .unwrap_or_else(|_| "/tmp".to_string());

                                let dir = options
                                    .get("dir")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&default_dir)
                                    .to_string();

                                let mut opts_map = HashMap::new();
                                for (k, v) in options.iter() {
                                    if let Some(s) = v.as_str() {
                                        opts_map.insert(k.clone(), s.to_string());
                                    }
                                }

                                let url_lower = url.to_lowercase();
                                let is_forced_torrent =
                                    options.get("file-type").and_then(|v| v.as_str())
                                        == Some("torrent");
                                if url_lower.starts_with("magnet:?")
                                    || url_lower.contains(".torrent")
                                    || is_forced_torrent
                                {
                                    let torrent_source =
                                        librqbit::AddTorrent::from_url(url.clone());
                                    let _ = manager
                                        .spawn_torrent_task(
                                            current_id,
                                            torrent_source,
                                            dir,
                                            opts_map,
                                        )
                                        .await;
                                    continue;
                                }

                                let filename =
                                    explicit_out.map(|s| s.to_string()).unwrap_or_else(|| {
                                        url.split('/')
                                            .next_back()
                                            .unwrap_or("download.bin")
                                            .split('?')
                                            .next()
                                            .unwrap_or("download.bin")
                                            .to_string()
                                    });

                                let threads = options
                                    .get("split")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| s.parse::<usize>().ok())
                                    .unwrap_or_else(|| {
                                        manager
                                            .default_split
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                            as usize
                                    });

                                let mut headers = Vec::new();
                                if let Some(header_str) =
                                    options.get("header").and_then(|v| v.as_str())
                                {
                                    for line in header_str.split('\n') {
                                        if !line.trim().is_empty() {
                                            headers.push(line.trim().to_string());
                                        }
                                    }
                                }

                                manager
                                    .spawn_task(
                                        current_id,
                                        vec![url],
                                        filename,
                                        dir,
                                        threads,
                                        0,
                                        headers,
                                        None,
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        if return_ids.is_empty() {
            let fallback_id = uuid::Uuid::new_v4().to_string();
            Some(serde_json::to_value(fallback_id).unwrap())
        } else if return_ids.len() == 1 {
            Some(serde_json::to_value(&return_ids[0]).unwrap())
        } else {
            Some(serde_json::to_value(return_ids).unwrap())
        }
    }
}
