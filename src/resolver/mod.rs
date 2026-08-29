//! Universal resolve orchestrator
//!
//! ### Architectural Overview
//! - **What it does**: Exposes multi-format metadata resolvers across direct HTTP streams, torrents, magnet links, HTML OpenGraph tags, and embedded Next.js/Nuxt script payloads.
//! - **How it does**: Re-exports `direct`, `html_scraper`, `script_extractor`, `torrent_resolver`, and `universal` submodules.
//! - **Where it comes from**: Imported by `manager::DownloadManager` and `rpc::handlers::ResolveHandlers`.
//! - **Where it leads to**: Returns structured `ResolveResponse` objects to client applications to pre-populate filenames, sizes, and file lists before download initiation.

pub mod direct;
pub mod html_scraper;
pub mod script_extractor;
pub mod torrent_resolver;
pub mod universal;

// Re-export resolver components
pub use direct::DirectHttpResolver;
pub use html_scraper::HtmlScraper;
pub use script_extractor::ScriptExtractor;
pub use torrent_resolver::TorrentResolver;
pub use universal::UniversalResolver;
