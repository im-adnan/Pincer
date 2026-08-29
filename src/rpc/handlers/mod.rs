//! Re-exports handler functions
//!
//! ### Architectural Overview
//! - **What it does**: Exposes granular RPC handler groups (task control, task lifecycle, queries, options, introspection, resolution, session, and system reflection).
//! - **How it does**: Declares handler submodules and re-exports their respective structs (`TaskControlHandlers`, `TaskLifecycleHandlers`, `TaskQueryHandlers`, etc.).
//! - **Where it comes from**: Imported by `rpc::MethodRouter`.
//! - **Where it leads to**: Aggregates handler logic separating transport parsing from domain manager business logic.

pub mod add_metalink;
pub mod add_torrent;
pub mod add_uri;
pub mod introspection;
pub mod options;
pub mod resolve;
pub mod session;
pub mod system;
pub mod task_control;
pub mod task_lifecycle;
pub mod task_query;

// Re-export handler structs
pub use add_metalink::AddMetalinkHandler;
pub use add_torrent::AddTorrentHandler;
pub use add_uri::AddUriHandler;
pub use introspection::IntrospectionHandlers;
pub use options::OptionsRpcHandlers;
pub use resolve::ResolveHandlers;
pub use session::SessionRpcHandlers;
pub use system::SystemRpcHandlers;
pub use task_control::TaskControlHandlers;
pub use task_lifecycle::TaskLifecycleHandlers;
pub use task_query::TaskQueryHandlers;
