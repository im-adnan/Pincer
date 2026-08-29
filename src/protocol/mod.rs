//! ProtocolAdapter trait & ResolvedMetadata
//!
//! ### Architectural Overview
//! - **What it does**: Defines the uniform asynchronous `ProtocolAdapter` trait implemented by all transport protocol handlers (HTTP/HTTPS, FTP, SFTP).
//! - **How it does**: Uses `async_trait` to provide polymorphic stream extraction (`get_stream`), content length probing (`get_content_length`), and URL scheme routing.
//! - **Where it comes from**: Instantiated by `engine::DownloadTask` and `engine::DownloadWorker`.
//! - **Where it leads to**: Dispatches network requests to specialized protocol implementations in `http`, `ftp`, and `sftp`.

pub mod ftp;
pub mod http;
pub mod sftp;

pub use ftp::FtpAdapter;
pub use http::HttpAdapter;
pub use sftp::SftpAdapter;

use crate::common::PincerResult;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;

/// Resolved remote resource metadata obtained during connection handshake.
#[derive(Debug, Clone)]
pub struct ResolvedMetadata {
    /// Expected byte length of the resource; `None` if chunked or unknown.
    pub content_length: Option<u64>,
    /// Indicates whether the remote server supports segmented byte-range requests.
    pub resumable: bool,
    /// Inferred or server-advertised filename.
    pub filename: Option<String>,
    /// Inferred MIME type.
    pub file_type: Option<String>,
}

/// Uniform asynchronous trait implemented by all transport protocol adapters.
///
/// This trait abstracts differences between HTTP, FTP, and SFTP endpoints, allowing the
/// download engine workers to stream byte ranges without protocol-specific branching.
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// Probes the remote endpoint for resource length and range support.
    async fn get_content_length(
        &self,
        url: &str,
        headers: &[String],
    ) -> PincerResult<ResolvedMetadata>;

    /// Initiates a segmented byte stream for the specified `[start, end]` range.
    async fn get_stream(
        &self,
        url: &str,
        start: u64,
        end: u64,
        headers: &[String],
    ) -> PincerResult<BoxStream<'static, PincerResult<Bytes>>>;
}

/// Factory function instantiating the appropriate `ProtocolAdapter` based on URL scheme.
pub fn get_adapter(url: &str) -> PincerResult<Box<dyn ProtocolAdapter>> {
    let lower = url.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(Box::new(HttpAdapter::new()))
    } else if lower.starts_with("ftp://") || lower.starts_with("ftps://") {
        Ok(Box::new(FtpAdapter::new()))
    } else if lower.starts_with("sftp://") {
        Ok(Box::new(SftpAdapter::new()))
    } else {
        Err(crate::common::PincerError::InvalidUrl(format!(
            "Unsupported protocol scheme in URL: {}",
            url
        )))
    }
}
