//! Error types and formatting
//!
//! ### Architectural Overview
//! - **What it does**: Defines the central error taxonomy (`PincerError`) and type alias (`PincerResult<T>`) used across the Pincer codebase.
//! - **How it does**: Implements `std::fmt::Display`, `std::error::Error`, and `From` conversions for standard library and crate error types.
//! - **Where it comes from**: Instantiated across domain modules whenever operations encounter failure conditions (e.g. invalid URLs, task not found, failed checksum).
//! - **Where it leads to**: Propagates up the call stack to RPC error responses, CLI error loggers, and task recovery routines.

use std::fmt;

/// The central error enumeration for all failure conditions in Pincer.
#[derive(Debug)]
pub enum PincerError {
    /// Network transport errors originating from HTTP client requests.
    Network(reqwest::Error),

    /// Local filesystem I/O failures (file creation, writing, moving).
    Io(std::io::Error),

    /// Serialization/deserialization errors for JSON-RPC payloads and session files.
    Json(serde_json::Error),

    /// Protocol-level errors from FTP, SFTP, or custom transport adapters.
    Protocol(String),

    /// Attempted to query or manipulate a task GID that does not exist in the registry.
    TaskNotFound(String),

    /// Attempted to create a task with an ID that already exists.
    TaskAlreadyExists(String),

    /// Malformed or unsupported URL scheme provided.
    InvalidUrl(String),

    /// SHA256 checksum mismatch after file download completion.
    IntegrityCheckFailed { expected: String, actual: String },

    /// Operation was aborted via cancellation token.
    Cancelled,

    /// RPC authentication failed or required secret token was omitted.
    Unauthorized,

    /// Generic unclassified error.
    Other(String),
}

impl fmt::Display for PincerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PincerError::Network(e) => write!(f, "Network I/O error: {}", e),
            PincerError::Io(e) => write!(f, "File system I/O error: {}", e),
            PincerError::Json(e) => write!(f, "JSON serialization error: {}", e),
            PincerError::Protocol(msg) => write!(f, "Protocol error: {}", msg),
            PincerError::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            PincerError::TaskAlreadyExists(id) => write!(f, "Task already exists: {}", id),
            PincerError::InvalidUrl(url) => write!(f, "Invalid URL: {}", url),
            PincerError::IntegrityCheckFailed { expected, actual } => {
                write!(
                    f,
                    "Integrity check failed: expected {}, got {}",
                    expected, actual
                )
            }
            PincerError::Cancelled => write!(f, "Operation cancelled"),
            PincerError::Unauthorized => write!(f, "Authentication required or invalid token"),
            PincerError::Other(msg) => write!(f, "Generic error: {}", msg),
        }
    }
}

impl std::error::Error for PincerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PincerError::Network(e) => Some(e),
            PincerError::Io(e) => Some(e),
            PincerError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for PincerError {
    fn from(err: reqwest::Error) -> Self {
        PincerError::Network(err)
    }
}

impl From<std::io::Error> for PincerError {
    fn from(err: std::io::Error) -> Self {
        PincerError::Io(err)
    }
}

impl From<serde_json::Error> for PincerError {
    fn from(err: serde_json::Error) -> Self {
        PincerError::Json(err)
    }
}

impl From<String> for PincerError {
    fn from(msg: String) -> Self {
        PincerError::Other(msg)
    }
}

impl From<&str> for PincerError {
    fn from(msg: &str) -> Self {
        PincerError::Other(msg.to_string())
    }
}

/// Standard Result type alias using `PincerError`.
pub type PincerResult<T> = Result<T, PincerError>;
