//! Models module exports and re-exports
//!
//! ### Architectural Overview
//! - **What it does**: Exposes all data transfer objects, task status structures, RPC wire protocol types, and session serialization models.
//! - **How it does**: Re-exports all submodules (`introspection`, `resolve`, `rpc`, `session`, `task`, `torrent`) at the crate model root.
//! - **Where it comes from**: Imported by all Pincer engine subsystems needing domain models or JSON serialization representations.
//! - **Where it leads to**: Acts as the single point of truth for protocol and state definitions.

pub mod introspection;
pub mod resolve;
pub mod rpc;
pub mod session;
pub mod task;
pub mod torrent;

// Re-export all model types for clean namespace usage
pub use introspection::*;
pub use resolve::*;
pub use rpc::*;
pub use session::*;
pub use task::*;
pub use torrent::*;
