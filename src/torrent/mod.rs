//! Torrent engine exports
//!
//! ### Architectural Overview
//! - **What it does**: Exposes BitTorrent session creation, torrent task spawning, live stats tracking, and selective file download pruning.
//! - **How it does**: Re-exports `file_selector`, `session`, `stats_tracker`, and `task_spawner` submodules wrapping `librqbit`.
//! - **Where it comes from**: Imported by `manager::TorrentOrchestrator` and `resolver::TorrentResolver`.
//! - **Where it leads to**: Drives the peer-to-peer BitTorrent download pipeline, DHT lookups, piece verification, and seeding.

pub mod file_selector;
pub mod session;
pub mod stats_tracker;
pub mod task_spawner;

// Re-export BitTorrent components
pub use file_selector::TorrentFileSelector;
pub use session::TorrentSessionManager;
pub use stats_tracker::{TorrentSnapshot, TorrentStatsTracker};
pub use task_spawner::TorrentTaskSpawner;
