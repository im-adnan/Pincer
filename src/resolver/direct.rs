//! Direct HTTP HEAD/GET metadata resolver
//!
//! ### Architectural Overview
//! - **What it does**: Parses HTTP response headers to extract final redirected URLs, Content-Type, Content-Length, byte-range capability, and Content-Disposition filename directives.
//! - **How it does**: Reads `reqwest::Response` headers (`content-type`, `content-length`, `accept-ranges`, `content-disposition`) and provides MIME-to-extension fallback mapping via `infer_extension()`.
//! - **Where it comes from**: Called by `resolver::UniversalResolver::resolve_url()` during HTTP probing.
//! - **Where it leads to**: Produces `DirectMetadata` used to construct task descriptors and `ResolveResponse` objects.

use reqwest::Response;

/// Metadata fields extracted from direct HTTP response headers.
pub struct DirectMetadata {
    pub final_url: String,
    pub content_type: String,
    pub total_size: Option<i64>,
    pub is_resumable: Option<bool>,
    pub content_disposition_filename: Option<String>,
}

/// Utility for extracting metadata headers and inferring file extensions from HTTP responses.
pub struct DirectHttpResolver;

impl DirectHttpResolver {
    /// Extracts metadata headers from a `reqwest::Response`.
    ///
    /// Reads:
    /// - `CONTENT_TYPE`: Inferred media MIME type.
    /// - `CONTENT_LENGTH`: Total byte length.
    /// - `ACCEPT_RANGES`: Byte-range support indicator (`"bytes"`).
    /// - `CONTENT_DISPOSITION`: Server-suggested filename (`filename="..."`).
    pub fn extract_metadata(response: &Response) -> DirectMetadata {
        let final_url = response.url().to_string();

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();

        let total_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok());

        let is_resumable = response
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|h| h.to_str().ok())
            .map(|s| s == "bytes");

        let content_disposition_filename = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                if let Some(idx) = s.find("filename=") {
                    let part = &s[idx + 9..];
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

        DirectMetadata {
            final_url,
            content_type,
            total_size,
            is_resumable,
            content_disposition_filename,
        }
    }

    /// Infers file extension based on standard Content-Type MIME strings.
    pub fn infer_extension(content_type: &str) -> &'static str {
        match content_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "application/pdf" => "pdf",
            "application/zip" => "zip",
            "application/x-gzip" => "gz",
            "application/x-tar" => "tar",
            "text/plain" => "txt",
            "text/html" => "html",
            "audio/mpeg" => "mp3",
            "audio/wav" => "wav",
            "audio/ogg" => "ogg",
            "video/mp4" => "mp4",
            "video/x-matroska" => "mkv",
            "video/webm" => "webm",
            _ => "",
        }
    }
}
