//! Pause, unpause, pauseAll, unpauseAll
//!
//! ### Architectural Overview
//! - **What it does**: Controls execution lifecycle states of tasks, pausing or unpausing direct downloads and torrent sessions individually or in bulk.
//! - **How it does**: Cancels task `CancellationToken`s, sets statuses to `paused` or `active`, pauses underlying `librqbit` handles, zeroing transfer speeds, and broadcasting `pin.onDownloadPause` events.
//! - **Where it comes from**: Called by `manager.pause_task()`, `manager.pause_all_tasks()`, and `manager.unpause_task()`.
//! - **Where it leads to**: Halts or resumes network workers and schedules waiting tasks in the queue.

use librqbit::{ManagedTorrent, Session};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use super::notifier::EventNotifier;
use super::state::TaskControl;

/// Manages pausing and resuming download tasks across direct transfers and BitTorrent sessions.
pub struct TaskLifecycleManager;

impl TaskLifecycleManager {
    /// Pauses an individual task by cancelling its cancellation token and zeroing transfer speeds.
    pub async fn pause_task(
        id: &str,
        tasks: &RwLock<HashMap<String, TaskControl>>,
        torrent_handles: &RwLock<HashMap<String, Arc<ManagedTorrent>>>,
        session: Option<&Arc<Session>>,
        tx: &broadcast::Sender<String>,
    ) -> bool {
        let (is_torrent, is_completed_torrent) = {
            let tasks_guard = tasks.read().await;
            if let Some(c) = tasks_guard.get(id) {
                let is_tor = c.status.file_type.as_deref() == Some("torrent");
                let completed = c.status.completed_length.parse::<u64>().unwrap_or(0);
                let total = c.status.total_length.parse::<u64>().unwrap_or(0);
                let is_comp = total > 0 && completed >= total;
                (is_tor, is_comp)
            } else {
                (false, false)
            }
        };

        // Pause BitTorrent handle if applicable
        if is_torrent {
            if is_completed_torrent {
                return false; // Don't pause seeding torrent if already complete
            }
            let handle = {
                let handles = torrent_handles.read().await;
                handles.get(id).cloned()
            };
            if let (Some(handle), Some(session)) = (handle, session) {
                let _ = session.pause(&handle).await;
            }
        }

        // Cancel direct worker token and update state to "paused"
        let mut tasks_guard = tasks.write().await;
        if let Some(control) = tasks_guard.get_mut(id) {
            control.token.cancel();
            control.status.status = "paused".to_string();
            control.status.download_speed = "0".to_string();
            control.status.upload_speed = Some("0".to_string());
            EventNotifier::emit(tx, "pin.onDownloadPause", id);
            EventNotifier::emit(tx, "pin.onDownloadStop", id);
            true
        } else {
            false
        }
    }

    /// Forcefully pauses an individual task (acts identically to pause_task in async environment).
    pub async fn force_pause_task(
        id: &str,
        tasks: &RwLock<HashMap<String, TaskControl>>,
        torrent_handles: &RwLock<HashMap<String, Arc<ManagedTorrent>>>,
        session: Option<&Arc<Session>>,
        tx: &broadcast::Sender<String>,
    ) -> bool {
        Self::pause_task(id, tasks, torrent_handles, session, tx).await
    }

    /// Pauses all active and waiting downloads across the entire engine.
    pub async fn pause_all(
        tasks: &RwLock<HashMap<String, TaskControl>>,
        tx: &broadcast::Sender<String>,
    ) {
        let mut tasks_guard = tasks.write().await;
        for (gid, control) in tasks_guard.iter_mut() {
            let is_completed_torrent = control.status.file_type.as_deref() == Some("torrent") && {
                let completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
                let total = control.status.total_length.parse::<u64>().unwrap_or(0);
                total > 0 && completed >= total
            };

            if is_completed_torrent {
                continue;
            }

            if control.status.status == "active"
                || control.status.status == "converting"
                || control.status.status == "waiting"
            {
                control.token.cancel();
                control.status.status = "paused".to_string();
                control.status.download_speed = "0".to_string();
                control.status.upload_speed = Some("0".to_string());
                EventNotifier::emit(tx, "pin.onDownloadPause", gid);
                EventNotifier::emit(tx, "pin.onDownloadStop", gid);
            }
        }
    }

    /// Forcefully pauses all downloads (acts identically to pause_all).
    pub async fn force_pause_all(
        tasks: &RwLock<HashMap<String, TaskControl>>,
        tx: &broadcast::Sender<String>,
    ) {
        Self::pause_all(tasks, tx).await
    }

    /// Resumes a paused BitTorrent task session handle.
    pub async fn unpause_torrent(
        id: &str,
        tasks: &RwLock<HashMap<String, TaskControl>>,
        torrent_handles: &RwLock<HashMap<String, Arc<ManagedTorrent>>>,
        session: &Arc<Session>,
    ) -> Option<(Arc<ManagedTorrent>, CancellationToken)> {
        let handle = {
            let handles = torrent_handles.read().await;
            handles.get(id).cloned()
        };

        if let Some(handle) = handle {
            let _ = session.unpause(&handle).await;
            let token = CancellationToken::new();
            let mut tasks_guard = tasks.write().await;
            if let Some(control) = tasks_guard.get_mut(id) {
                control.status.status = "active".to_string();
                control.token = token.clone();
            }
            return Some((handle, token));
        }
        None
    }
}
