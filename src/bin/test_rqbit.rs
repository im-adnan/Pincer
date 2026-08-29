//! librqbit test harness binary
//!
//! ### Architectural Overview
//! - **What it does**: Provides an isolated test harness to verify `librqbit` BitTorrent engine initialization, magnet parsing, and session statistics.
//! - **How it does**: Initializes a standalone temporary session, constructs an `AddTorrent` descriptor from a magnet link, and inspects handle metadata.
//! - **Where it comes from**: Executed manually via `cargo run --bin test_rqbit` during development and sanity verification.
//! - **Where it leads to**: Verifies compatibility with the underlying BitTorrent engine and outputs diagnostics directly to standard output.

use librqbit::{AddTorrent, Session};
use std::path::PathBuf;

/// Test binary entry point for validating the BitTorrent engine in isolation.
///
/// New contributors can use this binary to quickly test that `librqbit` compiles,
/// creates a temporary session directory, and parses magnet links correctly.
#[tokio::main]
async fn main() {
    println!("Testing librqbit...");

    // Create a temporary session directory for torrent scratch files.
    let session = Session::new(PathBuf::from("/tmp/pincer-test"))
        .await
        .unwrap();

    // Construct an AddTorrent instance from a well-formed magnet URI.
    let torrent =
        AddTorrent::from_url("magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862");

    // Add the torrent descriptor to the session without actively downloading all payload data.
    let res = session.add_torrent(torrent, None).await.unwrap();
    let handle = res.into_handle().unwrap();

    // Inspect the parsed torrent metadata.
    let name = handle.name();
    println!("Torrent name: {:?}", name);
    println!("Info hash: {}", handle.shared.info_hash.as_string());

    // Print initial statistics snapshot.
    let stats = handle.stats();
    println!("Progress bytes: {}", stats.progress_bytes);
}
