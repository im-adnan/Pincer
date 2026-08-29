//! Torrent & Magnet link resolver
//!
//! ### Architectural Overview
//! - **What it does**: Inspects and resolves metadata for Base64-encoded `.torrent` binary files and magnet link dictionaries prior to download initiation.
//! - **How it does**: Decodes Base64 payloads, invokes `session.add_torrent(..., list_only: true)`, and unpacks `TorrentMetaV1Info` to compute aggregate size and list contained files.
//! - **Where it comes from**: Called by `manager.resolve_torrent_base64()` and `resolver::UniversalResolver`.
//! - **Where it leads to**: Returns a `ResolveResponse` populated with `TorrentResolveFile` records to client UIs.

use crate::models::{ResolveResponse, TorrentResolveFile};
use librqbit::{AddTorrent, AddTorrentOptions, Session};
use std::sync::Arc;

/// Handles pre-download metadata inspection for `.torrent` payloads and Magnet URIs.
pub struct TorrentResolver;

impl TorrentResolver {
    /// Parses name, aggregate byte length, and file lists from a `TorrentMetaV1Info` dictionary.
    ///
    /// Handles:
    /// - Multi-file torrents: Joins path components (e.g. `["folder", "file.mp4"]` -> `"folder/file.mp4"`).
    /// - Single-file torrents: Extracts the root filename and length directly.
    pub fn parse_info(
        info: &librqbit::TorrentMetaV1Info<librqbit::ByteBufOwned>,
    ) -> (Option<String>, Option<i64>, Vec<TorrentResolveFile>) {
        let name = info
            .name
            .as_ref()
            .map(|n| String::from_utf8_lossy(n.as_ref()).into_owned());

        let mut files_resolved = Vec::new();
        let mut total_size: u64 = 0;

        if let Some(files) = &info.files {
            for (idx, f) in files.iter().enumerate() {
                let relative_path = f
                    .path
                    .iter()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).into_owned())
                    .collect::<Vec<String>>()
                    .join("/");
                files_resolved.push(TorrentResolveFile {
                    index: idx,
                    path: relative_path,
                    length: f.length,
                });
                total_size += f.length;
            }
        } else {
            let file_name = name.clone().unwrap_or_else(|| "download".to_string());
            let length = info.length.unwrap_or(0);
            files_resolved.push(TorrentResolveFile {
                index: 0,
                path: file_name,
                length,
            });
            total_size = length;
        }

        (name, Some(total_size as i64), files_resolved)
    }

    /// Resolves file list and total size from a Base64-encoded `.torrent` file payload.
    pub async fn resolve_base64(
        session: &Arc<Session>,
        base64_str: &str,
    ) -> Result<ResolveResponse, String> {
        use base64::{engine::general_purpose, Engine as _};
        let decoded = general_purpose::STANDARD
            .decode(base64_str.trim())
            .map_err(|e| format!("Invalid base64: {:?}", e))?;

        let torrent_source = AddTorrent::from_bytes(decoded);
        let opts = AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };
        let add_res = session
            .add_torrent(torrent_source, Some(opts))
            .await
            .map_err(|e| format!("Failed to resolve torrent: {:?}", e))?;

        match add_res {
            librqbit::AddTorrentResponse::ListOnly(res) => {
                let (name, total_size, files) = Self::parse_info(&res.info);
                Ok(ResolveResponse {
                    url: "".to_string(),
                    filename: name,
                    total_size,
                    file_type: Some("torrent".to_string()),
                    is_resumable: Some(true),
                    torrent_files: Some(files),
                })
            }
            _ => Err("Expected ListOnly response from base64 torrent resolver".to_string()),
        }
    }
}
