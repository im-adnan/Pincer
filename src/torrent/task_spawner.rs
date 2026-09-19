//! Torrent task creation & folder naming
//!
//! ### Architectural Overview
//! - **What it does**: Parses torrent sources (Magnet URIs, raw `.torrent` bytes) to extract display names, info hashes, and creates sanitized output directory structures.
//! - **How it does**: Decodes percent-encoded `dn=` magnet parameters, parses BTIH hash strings, inspects multi-file torrent dictionaries, and creates structured target folders on disk.
//! - **Where it comes from**: Called by `manager::TorrentOrchestrator::spawn_task()`.
//! - **Where it leads to**: Sets up target output directories and provides task metadata to initialize `AddTorrentOptions`.

use librqbit::{AddTorrent, AddTorrentOptions, Session};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Handles parsing of Magnet links and `.torrent` payloads to set up task folders and names.
pub struct TorrentTaskSpawner;

impl TorrentTaskSpawner {
    /// Extracts initial display name, BTIH info hash, and source URL from an `AddTorrent` source.
    ///
    /// - For Magnet links: Decodes percent-encoded `dn=` display name and `urn:btih:` hash string.
    /// - For `.torrent` byte arrays: Deserializes the Bencoded info dictionary via `librqbit::torrent_from_bytes`.
    pub fn extract_source_info(
        source: &AddTorrent<'_>,
    ) -> (String, Option<String>, Option<String>) {
        match source {
            AddTorrent::Url(url) => {
                let name = if url.starts_with("magnet:?") {
                    if let Some(pos) = url.find("dn=") {
                        let rest = &url[pos + 3..];
                        let end = rest.find('&').unwrap_or(rest.len());
                        percent_encoding::percent_decode_str(&rest[..end])
                            .decode_utf8()
                            .map(|s| s.into_owned())
                            .unwrap_or_else(|_| "Magnet Link".to_string())
                    } else {
                        "Magnet Link".to_string()
                    }
                } else {
                    url.split('/')
                        .next_back()
                        .and_then(|s| s.split('?').next())
                        .unwrap_or("Torrent Link")
                        .to_string()
                };

                let info_hash = if let Some(pos) = url.find("urn:btih:") {
                    let start = pos + 9;
                    let rest = &url[start..];
                    let end = rest.find('&').unwrap_or(rest.len());
                    Some(rest[..end].to_lowercase())
                } else {
                    None
                };

                (name, info_hash, Some(url.to_string()))
            }
            AddTorrent::TorrentFileBytes(bytes) => {
                if let Ok(t) = librqbit::torrent_from_bytes::<&[u8]>(bytes.as_ref()) {
                    let name = t
                        .info
                        .name
                        .as_ref()
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .unwrap_or_else(|| "Torrent File".to_string());
                    let info_hash = Some(t.info_hash.as_string());
                    (name, info_hash, None)
                } else {
                    ("Torrent File".to_string(), None, None)
                }
            }
        }
    }

    /// Resolves target directory folder path and cleans up whitespace/dots for torrent files.
    pub async fn prepare_output_directory(
        session: &Arc<Session>,
        torrent_source: &AddTorrent<'static>,
        initial_name: &str,
        dir: &str,
    ) -> (String, String) {
        let mut resolved_name = initial_name.to_string();
        let mut is_multi = false;

        match torrent_source {
            AddTorrent::TorrentFileBytes(bytes) => {
                if let Ok(t) = librqbit::torrent_from_bytes::<&[u8]>(bytes.as_ref()) {
                    is_multi = t.info.files.is_some();
                    if let Some(name_buf) = &t.info.name {
                        resolved_name = String::from_utf8_lossy(name_buf).into_owned();
                    }
                }
            }
            AddTorrent::Url(url) => {
                let list_opts = AddTorrentOptions {
                    list_only: true,
                    ..Default::default()
                };
                let source_for_add = AddTorrent::Url(url.clone());
                if let Ok(librqbit::AddTorrentResponse::ListOnly(res)) =
                    session.add_torrent(source_for_add, Some(list_opts)).await
                {
                    is_multi = res.info.files.is_some();
                    if let Some(name_buf) = &res.info.name {
                        resolved_name = String::from_utf8_lossy(name_buf).into_owned();
                    }
                }
            }
        }

        let folder_name = if !is_multi {
            let path = Path::new(&resolved_name);
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| resolved_name.clone())
        } else {
            resolved_name.clone()
        };

        let folder_name = folder_name.replace(' ', "-").replace('.', "_");
        let final_path = PathBuf::from(dir).join(&folder_name);
        let _ = std::fs::create_dir_all(&final_path);
        let final_dir = final_path.to_string_lossy().to_string();

        (final_dir, resolved_name)
    }
}
