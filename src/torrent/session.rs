//! Torrent session initialization & fallback ports
//!
//! ### Architectural Overview
//! - **What it does**: Initializes and configures the singleton `librqbit::Session` for BitTorrent downloads with automatic port fallback.
//! - **How it does**: Attempts binding to default port range `6881..6891`; on port collision or permission failure, falls back cleanly to ephemeral port allocation (`0..1`) under `~/.pincer/torrents`.
//! - **Where it comes from**: Called lazily via `manager.get_torrent_session()` when the first BitTorrent task or magnet link is submitted.
//! - **Where it leads to**: Returns an `Arc<Session>` managing peer connections, DHT nodes, and torrent handles.

use librqbit::Session;
use std::sync::Arc;

/// Manages the lifecycle of the singleton `librqbit::Session`.
pub struct TorrentSessionManager;

impl TorrentSessionManager {
    /// Creates and configures a new `librqbit::Session` with port fallback.
    ///
    /// Initialization steps:
    /// 1. Prepares scratch directory `~/.pincer/torrents`.
    /// 2. Attempts binding to standard BitTorrent port range `6881..6891`.
    /// 3. If standard ports fail (already in use by another client or blocked by OS),
    ///    falls back cleanly to ephemeral port binding (`0..1`), guaranteeing success.
    pub async fn create_session() -> Result<Arc<Session>, String> {
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = home.join(".pincer").join("torrents");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let opts = librqbit::SessionOptions {
            listen_port_range: Some(6881..6891),
            enable_upnp_port_forwarding: false,
            ..Default::default()
        };

        let session = match Session::new_with_opts(dir.clone(), opts).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "[PINCER INFO] Failed to bind standard torrent ports: {:?}. Falling back to ephemeral port.",
                    e
                );
                let fallback_opts = librqbit::SessionOptions {
                    listen_port_range: Some(0..1),
                    enable_upnp_port_forwarding: false,
                    disable_dht: true,
                    disable_dht_persistence: true,
                    ..Default::default()
                };
                Session::new_with_opts(dir, fallback_opts)
                    .await
                    .map_err(|err| {
                        format!(
                            "Failed to create librqbit session with ephemeral port fallback: {:?}",
                            err
                        )
                    })?
            }
        };

        Ok(session)
    }
}
