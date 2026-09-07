//! Remove, removeAndFile (Trash), forceRemove
//!
//! ### Architectural Overview
//! - **What it does**: Removes tasks from the active registry and safely moves downloaded files, bundles, and staging directories to Trash with strict directory protection.
//! - **How it does**: Cancels tokens, drops torrent session handles, validates paths against protected system/home directories (`is_protected_directory`), and moves files/folders ONLY to Trash via `trash::delete`. Direct permanent deletion is strictly banned.
//! - **Where it comes from**: Called by `manager.remove_task()`, `manager.remove_task_and_file()`, and `manager.force_remove_task()`.
//! - **Where it leads to**: Removes tasks from memory, safely trashes downloaded files to Recycle Bin, and triggers queue rescheduling.

use super::state::TaskControl;
use librqbit::{api::TorrentIdOrHash, ManagedTorrent, Session};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages task removal and safe filesystem deletion to Trash / Recycle Bin.
pub struct TaskRemovalManager;

impl TaskRemovalManager {
    /// Returns true if the path is a system root, home folder, or standard user directory that must NEVER be deleted.
    pub fn is_protected_directory(path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        if path_str.trim().is_empty() || path_str == "/" || path_str == "." || path_str == ".." {
            return true;
        }

        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                let mut buf = PathBuf::new();
                for comp in path.components() {
                    buf.push(comp);
                }
                buf
            }
        };

        // Root directory
        if canonical.parent().is_none() || canonical == Path::new("/") {
            return true;
        }

        // Top-level system directories
        let system_roots = [
            "/",
            "/System",
            "/Library",
            "/usr",
            "/bin",
            "/sbin",
            "/Applications",
            "/private",
            "/var",
            "/etc",
            "/tmp",
            "/Volumes",
            "/opt",
            "/Users",
            "/dev",
            "/cores",
            "/Network",
        ];
        for sys in &system_roots {
            if canonical == Path::new(sys) {
                return true;
            }
        }

        // User home and standard user directories
        if let Ok(home_str) = std::env::var("HOME") {
            let home = PathBuf::from(&home_str);
            let canonical_home = home.canonicalize().unwrap_or(home);

            if canonical == canonical_home {
                return true;
            }

            let user_dirs = [
                "Desktop",
                "Downloads",
                "Documents",
                "Pictures",
                "Music",
                "Movies",
                "Library",
                "Public",
                "Applications",
                ".Trash",
                ".config",
                ".local",
            ];
            for dir in &user_dirs {
                let user_dir = canonical_home.join(dir);
                let canonical_user_dir =
                    user_dir.canonicalize().unwrap_or_else(|_| user_dir.clone());
                if canonical == user_dir || canonical == canonical_user_dir {
                    return true;
                }
            }
        }

        false
    }

    /// Removes a task from memory and unregisters any associated BitTorrent handle without deleting files.
    pub async fn remove_task(
        id: &str,
        tasks: &RwLock<HashMap<String, TaskControl>>,
        torrent_handles: &RwLock<HashMap<String, Arc<ManagedTorrent>>>,
        session: Option<&Arc<Session>>,
        tx: &tokio::sync::broadcast::Sender<String>,
    ) -> bool {
        let is_torrent = {
            let tasks_guard = tasks.read().await;
            tasks_guard.get(id).and_then(|c| c.status.file_type.clone())
                == Some("torrent".to_string())
        };

        if is_torrent {
            let handle = {
                let mut handles = torrent_handles.write().await;
                handles.remove(id)
            };
            if let (Some(handle), Some(session)) = (handle, session) {
                let _ = session
                    .delete(TorrentIdOrHash::Id(handle.id()), false)
                    .await;
            }
        }

        let mut tasks_guard = tasks.write().await;
        if let Some(control) = tasks_guard.get_mut(id) {
            control.token.cancel();
            control.status.status = "removed".to_string();
            super::notifier::EventNotifier::emit(tx, "pin.onDownloadStop", id);
            tasks_guard.remove(id);
            true
        } else {
            false
        }
    }

    /// Safely moves an individual file or directory to Trash / Recycle Bin.
    /// Direct permanent deletions are strictly forbidden.
    pub fn safe_delete_file(path: &Path) {
        if !path.exists() {
            return;
        }

        // NEVER delete protected system or user root directories
        if Self::is_protected_directory(path) {
            eprintln!(
                "[PINCER SECURITY] Blocked attempted deletion of protected path: {:?}",
                path
            );
            return;
        }

        // Move exclusively to Trash / Recycle Bin
        let res = std::panic::catch_unwind(|| trash::delete(path));
        match res {
            Ok(Ok(_)) => {
                println!("[PINCER] Successfully moved to Trash: {:?}", path);
            }
            Ok(Err(e)) => {
                eprintln!("[PINCER ERR] Failed to move {:?} to Trash: {:?}", path, e);
            }
            Err(_) => {
                eprintln!(
                    "[PINCER ERR] Panic occurred while moving {:?} to Trash",
                    path
                );
            }
        }
    }

    /// Removes a task and safely moves downloaded files/folders to Trash.
    ///
    /// Safety guarantees:
    /// - Under no circumstances will a download container directory (e.g. `~/Desktop`, `~/Downloads`) be deleted.
    /// - All deletions are exclusively moved to the Trash / Recycle Bin (`trash::delete`).
    pub async fn remove_task_and_file(
        id: &str,
        tasks: &RwLock<HashMap<String, TaskControl>>,
        torrent_handles: &RwLock<HashMap<String, Arc<ManagedTorrent>>>,
        session: Option<&Arc<Session>>,
        tx: &tokio::sync::broadcast::Sender<String>,
    ) -> bool {
        let is_torrent = {
            let tasks_guard = tasks.read().await;
            tasks_guard.get(id).and_then(|c| c.status.file_type.clone())
                == Some("torrent".to_string())
        };

        if is_torrent {
            let handle = {
                let mut handles = torrent_handles.write().await;
                handles.remove(id)
            };
            if let (Some(handle), Some(session)) = (handle, session) {
                let _ = session
                    .delete(TorrentIdOrHash::Id(handle.id()), false)
                    .await;
            }
        }

        let mut tasks_guard = tasks.write().await;
        if let Some(control) = tasks_guard.get_mut(id) {
            control.token.cancel();
            control.status.status = "removed".to_string();
            super::notifier::EventNotifier::emit(tx, "pin.onDownloadStop", id);
            let control = tasks_guard.remove(id).unwrap();

            let files = control.status.files.clone();
            let dir = control.status.dir.clone();
            let is_torrent_task = control.status.file_type.as_deref() == Some("torrent");

            tokio::task::spawn_blocking(move || {
                let target_dir = PathBuf::from(&dir);

                if is_torrent_task {
                    // For torrents: move the specific torrent folder to Trash if it is not a protected parent directory
                    if target_dir.exists()
                        && target_dir.is_dir()
                        && !Self::is_protected_directory(&target_dir)
                    {
                        Self::safe_delete_file(&target_dir);
                    }

                    // Also move any explicitly listed file paths to Trash
                    for file in &files {
                        let file_path = PathBuf::from(&file.path);
                        if file_path.exists() && !Self::is_protected_directory(&file_path) {
                            Self::safe_delete_file(&file_path);
                        }
                    }
                } else {
                    // For direct downloads: move the downloaded file and its .download bundle to Trash
                    for file in &files {
                        let file_path = PathBuf::from(&file.path);
                        if !Self::is_protected_directory(&file_path) {
                            Self::safe_delete_file(&file_path);

                            if let Some(filename) = file_path.file_name().and_then(|s| s.to_str()) {
                                let parent = file_path.parent().unwrap_or_else(|| Path::new(&dir));
                                if !Self::is_protected_directory(parent) {
                                    let bundle_path = parent.join(format!("{}.download", filename));
                                    let part_path = parent.join(format!(".{}.pincer", filename));
                                    Self::safe_delete_file(&bundle_path);
                                    Self::safe_delete_file(&part_path);
                                }
                            }
                        }
                    }
                }
            });
            true
        } else {
            false
        }
    }
}
