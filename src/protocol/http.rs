//! HttpAdapter range request & streaming
//!
//! ### Architectural Overview
//! - **What it does**: Handles HTTP and HTTPS byte-range downloads and metadata discovery using the asynchronous `reqwest` client.
//! - **How it does**: Sets `Range: bytes=start-end` request headers, verifies `206 Partial Content` range support, parses `Content-Length`, `Content-Disposition`, and MIME headers, and converts response byte streams into pinned Futures streams.
//! - **Where it comes from**: Instantiated by `protocol::get_adapter()` for `http://` and `https://` URLs.
//! - **Where it leads to**: Streams byte chunks to `engine::DownloadWorker` for disk persistence.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{BoxStream, StreamExt};
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, RANGE,
};
use reqwest::Client;
use std::str::FromStr;

use super::{ProtocolAdapter, ResolvedMetadata};
use crate::common::{PincerError, PincerResult};

/// HTTP/HTTPS protocol adapter wrapping `reqwest::Client`.
pub struct HttpAdapter {
    client: Client,
}

impl Default for HttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpAdapter {
    /// Creates a new `HttpAdapter` with default redirect and timeout configurations.
    pub fn new() -> Self {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    /// Converts an array of "Key: Value" strings into a `reqwest::header::HeaderMap`.
    fn build_headers(headers: &[String]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for h in headers {
            if let Some((k, v)) = h.split_once(':') {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::from_str(k.trim()),
                    HeaderValue::from_str(v.trim()),
                ) {
                    map.insert(name, val);
                }
            }
        }
        map
    }
}

#[async_trait]
impl ProtocolAdapter for HttpAdapter {
    /// Performs an initial probe request with Range: bytes=0-0 to verify true Partial Content support.
    async fn get_content_length(
        &self,
        url: &str,
        headers: &[String],
    ) -> PincerResult<ResolvedMetadata> {
        let req_headers = Self::build_headers(headers);
        let resp = self
            .client
            .get(url)
            .headers(req_headers)
            .header(RANGE, "bytes=0-0")
            .send()
            .await?;

        let status = resp.status();
        let headers = resp.headers();

        // Strict range support detection: Server MUST respond with 206 Partial Content to Range: bytes=0-0
        let resumable = status == reqwest::StatusCode::PARTIAL_CONTENT;

        // Parse total content length
        let content_length = if let Some(content_range) =
            headers.get("content-range").and_then(|v| v.to_str().ok())
        {
            if let Some(pos) = content_range.rfind('/') {
                content_range[pos + 1..].trim().parse::<u64>().ok()
            } else {
                None
            }
        } else {
            headers
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
        };

        // Extract filename from Content-Disposition header
        let filename = headers
            .get(CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(|cd| {
                if let Some(pos) = cd.find("filename=") {
                    let part = &cd[pos + 9..];
                    let name = part
                        .split(';')
                        .next()
                        .unwrap_or(part)
                        .trim()
                        .trim_matches('"');
                    Some(name.to_string())
                } else {
                    None
                }
            });

        // Extract Content-Type MIME string
        let file_type = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        Ok(ResolvedMetadata {
            content_length,
            resumable,
            filename,
            file_type,
        })
    }

    /// Requests a specific segmented range `[start, end]` and returns a pinned stream of byte chunks.
    async fn get_stream(
        &self,
        url: &str,
        start: u64,
        end: u64,
        headers: &[String],
    ) -> PincerResult<BoxStream<'static, PincerResult<Bytes>>> {
        let mut req_headers = Self::build_headers(headers);
        let is_range_request = end > 0 && end >= start;

        if is_range_request {
            req_headers.insert(
                RANGE,
                HeaderValue::from_str(&format!("bytes={}-{}", start, end)).unwrap(),
            );
        } else if start > 0 {
            req_headers.insert(
                RANGE,
                HeaderValue::from_str(&format!("bytes={}-", start)).unwrap(),
            );
        }

        let resp = self.client.get(url).headers(req_headers).send().await?;

        let status = resp.status();

        // Safety check: If a range segment starting past byte 0 was requested, server must return 206 Partial Content
        if start > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(PincerError::Protocol(format!(
                "Server ignored Range header and returned non-partial status: {}",
                status
            )));
        }

        if !status.is_success() {
            return Err(PincerError::Protocol(format!(
                "HTTP request returned error status: {}",
                status
            )));
        }

        let stream = resp
            .bytes_stream()
            .map(|res| res.map_err(PincerError::Network));

        Ok(Box::pin(stream))
    }
}
