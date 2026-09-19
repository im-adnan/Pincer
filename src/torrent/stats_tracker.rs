//! Torrent live stats, speeds, and polling loop
//!
//! ### Architectural Overview
//! - **What it does**: Extracts point-in-time progress snapshots, upload/download speeds, seeder counts, file listings, and tracker announce lists from live `ManagedTorrent` handles.
//! - **How it does**: Reads `handle.stats()` and `handle.metadata.load()`, calculating Mbps to bytes/sec conversions, constructing `FileData` path lists, and packaging `TorrentSnapshot`.
//! - **Where it comes from**: Called on timer ticks by `manager::TorrentOrchestrator::run_stats_loop()`.
//! - **Where it leads to**: Updates `TaskStatus` in the task registry, writes session persistence checkpoints, and emits `pin.onDownloadProgress` notifications.

use crate::models::{FileData, TorrentInfo, TorrentInfoInner};
use librqbit::ManagedTorrent;
use std::path::PathBuf;
use std::sync::Arc;

/// Aggregated point-in-time metric snapshot extracted from a live `ManagedTorrent`.
pub struct TorrentSnapshot {
    pub total_bytes: u64,
    pub progress_bytes: u64,
    pub download_speed: u64,
    pub upload_speed: u64,
    pub peer_count: u32,
    pub finished: bool,
    pub files: Vec<FileData>,
    pub bittorrent: Option<TorrentInfo>,
}

/// Utility for querying and transforming live BitTorrent session handle statistics.
pub struct TorrentStatsTracker;

impl TorrentStatsTracker {
    /// Extracts a point-in-time snapshot from an active `ManagedTorrent` handle.
    ///
    /// Metric processing:
    /// 1. Reads raw byte progress and total expected bytes.
    /// 2. Converts Mbps transfer speeds from `librqbit` into standard bytes/second.
    /// 3. Extracts live peer and seeder counts.
    /// 4. Reconstructs multi-file disk paths from metadata dictionary if available.
    /// 5. Collects active tracker announce URLs into tiered announce lists.
    pub fn snapshot(handle: &Arc<ManagedTorrent>, dir: &str) -> TorrentSnapshot {
        let stats = handle.stats();
        let total_bytes = stats.total_bytes;
        let progress_bytes = stats.progress_bytes;
        let has_metadata = handle.metadata.load().is_some();
        let finished = stats.finished && has_metadata;

        // Convert Mbps from librqbit to bytes/second
        let mut download_speed = 0;
        let mut upload_speed = 0;
        let mut peer_count = 0;
        if let Some(live) = &stats.live {
            download_speed = (live.download_speed.mbps * 1024.0 * 1024.0) as u64;
            upload_speed = (live.upload_speed.mbps * 1024.0 * 1024.0) as u64;
            peer_count = live.snapshot.peer_stats.live as u32;
        }

        // Build file list from metadata
        let files = {
            let mut fls = Vec::new();
            if let Some(meta) = &*handle.metadata.load() {
                let parent_dir = PathBuf::from(dir);
                if let Some(files) = &meta.info.files {
                    for f in files {
                        let mut file_path = parent_dir.clone();
                        for component in &f.path {
                            file_path.push(String::from_utf8_lossy(component.as_ref()).as_ref());
                        }
                        fls.push(FileData {
                            path: file_path.to_string_lossy().to_string(),
                            uris: vec![],
                        });
                    }
                } else {
                    let mut file_path = parent_dir.clone();
                    if let Some(name_buf) = &meta.info.name {
                        file_path.push(String::from_utf8_lossy(name_buf.as_ref()).as_ref());
                    }
                    fls.push(FileData {
                        path: file_path.to_string_lossy().to_string(),
                        uris: vec![],
                    });
                }
            }
            fls
        };

        // Construct TorrentInfo metadata
        let bittorrent = if let Some(meta) = &*handle.metadata.load() {
            let mode = if meta.info.files.is_some() {
                "multi"
            } else {
                "single"
            };
            let name = meta
                .info
                .name
                .as_ref()
                .map(|b| String::from_utf8_lossy(b.as_ref()).to_string())
                .unwrap_or_default();
            let mut announce_list = Vec::new();
            for tracker in &handle.shared.trackers {
                announce_list.push(vec![tracker.to_string()]);
            }
            Some(TorrentInfo {
                announce_list,
                comment: None,
                creation_date: None,
                mode: mode.to_string(),
                info: TorrentInfoInner { name },
            })
        } else {
            None
        };

        TorrentSnapshot {
            total_bytes,
            progress_bytes,
            download_speed,
            upload_speed,
            peer_count,
            finished,
            files,
            bittorrent,
        }
    }
}
