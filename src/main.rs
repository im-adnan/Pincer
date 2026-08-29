//! Clean binary entrypoint delegating to CLI / Server
//!
//! ### Architectural Overview
//! - **What it does**: Serves as the binary entrypoint for the `pincer` executable, orchestrating CLI argument evaluation, daemonization, direct downloading, and background RPC server execution.
//! - **How it does**: Initializes the Tokio multi-thread runtime, invokes `cli::CliDispatcher::parse_and_dispatch()`, and loads persisted sessions asynchronously.
//! - **Where it comes from**: Executed directly by the OS process manager when a user runs the `pincer` binary from the command line or as a background service.
//! - **Where it leads to**: Delegates to `pincer::cli` for parsing and direct download flows, or `pincer::rpc::start_server` for JSON-RPC WebSocket daemon operation.

use pincer::cli::CliDispatcher;
use pincer::manager::DownloadManager;

/// Entry point of the Pincer executable.
///
/// This function sets up the Tokio multi-threaded asynchronous runtime and drives the
/// startup lifecycle:
/// 1. Instantiates the central `DownloadManager` and its event notification broadcast channel.
/// 2. Restores any previously persisted session state from disk (`~/.pincer/pincer.session`).
/// 3. Hands off command line arguments to `CliDispatcher` to start either a direct download,
///    a detached background daemon, or the WebSocket JSON-RPC server.
#[tokio::main]
async fn main() {
    // Step 1: Create the DownloadManager instance and obtain its broadcast receiver.
    let (manager, rx) = DownloadManager::new();

    // Step 2: Asynchronously load any saved session tasks and global settings from previous runs.
    manager.load_session().await;

    // Step 3: Parse CLI arguments and dispatch execution to the appropriate operational mode.
    CliDispatcher::parse_and_dispatch(manager, rx).await;
}
