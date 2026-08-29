//! CLI module exports and mode dispatcher
//!
//! ### Architectural Overview
//! - **What it does**: Parses terminal options and dispatches Pincer between daemon mode, interactive direct download mode, and headless RPC server mode.
//! - **How it does**: Reads arguments via `args::CliArgs`, detects flags (`--daemon`, `--enable-rpc`, positional URLs), optionally detaches into the background via `daemon::DaemonManager`, and orchestrates execution.
//! - **Where it comes from**: Called by `src/main.rs` via `CliDispatcher::parse_and_dispatch()`.
//! - **Where it leads to**: Hands off execution to `direct_download::DirectDownloader` for CLI downloads or `crate::rpc::start_server` for WebSocket RPC handling.

pub mod args;
pub mod daemon;
pub mod direct_download;
pub mod formatters;
pub mod progress_bar;

// Re-export CLI components for easy consumption
pub use args::CliArgs;
pub use daemon::DaemonManager;
pub use direct_download::DirectDownloader;
pub use formatters::{format_bytes, format_duration};
pub use progress_bar::TerminalProgressBar;

use crate::manager::DownloadManager;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Dispatcher responsible for inspecting parsed arguments and launching the requested operational mode.
pub struct CliDispatcher;

impl CliDispatcher {
    /// Parses CLI arguments from environment and routes execution to:
    /// 1. Background daemon spawning (if `--daemon` / `-D` was specified).
    /// 2. Direct CLI download with interactive progress bar (if a URL positional argument was provided).
    /// 3. WebSocket JSON-RPC 2.0 server (default mode or when `--enable-rpc` is set).
    pub async fn parse_and_dispatch(
        manager: Arc<DownloadManager>,
        rx: broadcast::Receiver<String>,
    ) {
        // Step 1: Parse arguments using lexopt.
        let args = match CliArgs::parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        };

        // Step 2: Handle daemonization if requested.
        if args.daemon {
            DaemonManager::spawn_background_process();
            return;
        }

        // Step 3: Check if a direct download URL was provided.
        if let Some(url) = args.url.clone() {
            // Direct download mode: downloads file to disk with live ANSI progress bar.
            DirectDownloader::run(url, args, manager).await;
        } else {
            // RPC Server mode: start the Axum WebSocket listener on the configured port.
            let port = args.rpc_listen_port.unwrap_or(6800);
            crate::rpc::start_server(manager, rx, port, args.rpc_secret).await;
        }
    }
}
