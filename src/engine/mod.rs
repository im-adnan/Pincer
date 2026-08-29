//! Download engine core
//!
//! ### Architectural Overview
//! - **What it does**: Exposes core segmented downloading subsystems including disk pre-allocation, macOS `.download` staging bundles, byte-range partitioning, worker concurrency, and active rate limiting.
//! - **How it does**: Re-exports all submodules (`allocator`, `bundle`, `chunker`, `task`, `throttler`, `worker`) providing high-level abstractions for multi-threaded transfers.
//! - **Where it comes from**: Imported by `manager::TaskRunner`, `cli::DirectDownloader`, and format converters.
//! - **Where it leads to**: Executes high-throughput network stream processing and direct zero-copy disk writes.

pub mod allocator;
pub mod bundle;
pub mod chunker;
pub mod task;
pub mod throttler;
pub mod worker;

// Re-export core engine components
pub use allocator::DiskAllocator;
pub use bundle::DownloadBundle;
pub use chunker::{RangeChunk, RangeChunker};
pub use task::DownloadTask;
pub use throttler::{RateThrottler, ThreadGuard};
pub use worker::DownloadWorker;
