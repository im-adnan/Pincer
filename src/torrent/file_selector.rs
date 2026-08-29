//! Selective file download & unselected file cleanup
//!
//! ### Architectural Overview
//! - **What it does**: Deletes unselected files and prunes empty parent directories when selective multi-file BitTorrent downloads complete.
//! - **How it does**: Parses the `select-files` comma-delimited index set, compares against total torrent file indices, removes unselected files via `std::fs::remove_file`, and safely prunes only non-protected empty ancestor directories.
//! - **Where it comes from**: Called by `manager::TorrentOrchestrator::run_stats_loop()` upon task initialization and completion when `select-files` is configured.
//! - **Where it leads to**: Cleans up disk footprint leaving only the explicitly selected files in the download destination.

use crate::manager::removal::TaskRemovalManager;
use std::collections::HashSet;
use std::path::PathBuf;

/// Manages cleanup for selective BitTorrent downloads.
pub struct TorrentFileSelector;

impl TorrentFileSelector {
    /// Deletes unselected files and prunes any empty ancestor directories created during download.
    pub fn cleanup_unselected_files(
        dir: &str,
        selected_indices_str: &str,
        torrent_meta_files: &[librqbit::TorrentMetaV1File<librqbit::ByteBufOwned>],
    ) {
        let selected_indices: HashSet<usize> = selected_indices_str
            .split(',')
            .filter_map(|s| s.parse::<usize>().ok())
            .collect();

        let parent_dir = PathBuf::from(dir);
        let mut unselected_files = Vec::new();

        for (idx, f) in torrent_meta_files.iter().enumerate() {
            if !selected_indices.contains(&idx) {
                let mut file_path = parent_dir.clone();
                for component in &f.path {
                    file_path.push(String::from_utf8_lossy(component.as_ref()).as_ref());
                }
                unselected_files.push(file_path);
            }
        }

        // Delete unselected files and prune empty ancestor directories
        let torrent_dir = PathBuf::from(dir);
        for path in unselected_files {
            if path.exists() && !TaskRemovalManager::is_protected_directory(&path) {
                if path.is_file() {
                    let _ = std::fs::remove_file(&path);
                }

                // Prune empty directories up to the torrent root folder
                let mut current_parent = path.parent();
                while let Some(p) = current_parent {
                    if p == torrent_dir
                        || !p.starts_with(&torrent_dir)
                        || TaskRemovalManager::is_protected_directory(p)
                    {
                        break;
                    }
                    if let Ok(mut entries) = p.read_dir() {
                        if entries.next().is_none() {
                            let _ = std::fs::remove_dir(p);
                        }
                    }
                    current_parent = p.parent();
                }
            }
        }
    }
}
