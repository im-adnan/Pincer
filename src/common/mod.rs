//! Common utilities module exports
//!
//! ### Architectural Overview
//! - **What it does**: Exposes shared error types, filename sanitization routines, and parameterized URI expansion algorithms across the engine.
//! - **How it does**: Declares `error`, `sanitize`, and `uri_expansion` submodules, and re-exports their primary functions (`PincerError`, `sanitize_filename`, `expand_uris`).
//! - **Where it comes from**: Imported by all subsystems across the Pincer codebase (`cli`, `engine`, `manager`, `resolver`, `rpc`).
//! - **Where it leads to**: Provides foundational utility primitives consumed during URL parsing, path validation, and error propagation.

pub mod error;
pub mod sanitize;
pub mod uri_expansion;

// Re-export core error handling types
pub use error::{PincerError, PincerResult};

// Re-export path security and sanitization helpers
pub use sanitize::sanitize_filename;

// Re-export parameterized URL expansion logic
pub use uri_expansion::expand_uris;
