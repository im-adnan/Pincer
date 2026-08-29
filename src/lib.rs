//! Library crate root exposing public modules
//!
//! ### Architectural Overview
//! - **What it does**: Exposes Pincer's public domain modules (`cli`, `common`, `converter`, `engine`, `manager`, `metalink`, `models`, `protocol`, `resolver`, `rpc`, `torrent`) to binaries, integration tests, and external crates.
//! - **How it does**: Re-exports top-level subsystems as public Rust modules and provides crate-level exports for common structs and managers.
//! - **Where it comes from**: Imported by `src/main.rs`, integration test suites, and external tools interfacing with the Pincer engine.
//! - **Where it leads to**: Delegates to all core subsystem crates and submodules across `src/`.

// ---------------------------------------------------------------------------
// Subsystem Declarations
// ---------------------------------------------------------------------------

/// Command-Line Interface, terminal arg parsing, and direct download execution.
pub mod cli;

/// Common utilities, error taxonomy (`PincerError`), filename sanitization, and URI bracket expansion.
pub mod common;

/// Format conversion and media transcoding subsystem (sips, cupsfilter, ffmpeg, afconvert).
pub mod converter;

/// Core high-performance chunked download engine, worker threads, rate limiting, and disk allocation.
pub mod engine;

/// Central Download Manager facade, task scheduler, state registry, lifecycle, and session persistence.
pub mod manager;

/// Quick-XML streaming parser for Metalink 3.0 / 4.0 download manifests.
pub mod metalink;

/// Strongly-typed Serde data transfer objects for JSON-RPC 2.0 requests, responses, tasks, and sessions.
pub mod models;

/// Network transport protocol adapters implementing the uniform `ProtocolAdapter` trait (HTTP, FTP, SFTP).
pub mod protocol;

/// Universal media link, magnet, and metadata resolution pipelines.
pub mod resolver;

/// Axum WebSocket server, JSON-RPC 2.0 method router, and granular request handlers.
pub mod rpc;

/// BitTorrent engine integration via `librqbit`, peer session management, and selective file download.
pub mod torrent;

// ---------------------------------------------------------------------------
// Convenience Re-exports
// ---------------------------------------------------------------------------

// Re-export the primary DownloadManager so consumers can access it directly via `pincer::DownloadManager`.
pub use manager::DownloadManager;

// Re-export all model types for clean imports in consuming crates and integration tests.
pub use models::*;
