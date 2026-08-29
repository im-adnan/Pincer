//! MIME type & extension detection
//!
//! ### Architectural Overview
//! - **What it does**: Infers the original source file format and extension from remote URLs, Content-Type headers, or file path heuristics.
//! - **How it does**: Extracts extensions from raw URL paths (stripping query strings), inspects server Content-Type headers, and falls back to filename extension strings.
//! - **Where it comes from**: Called by `converter::FormatTranscoder::convert_format()`.
//! - **Where it leads to**: Supplies normalized source extension strings to determine conversion pathways.

use std::path::Path;

/// Detects the source format of a downloaded file from multiple heuristic signals.
pub struct FormatDetector;

impl FormatDetector {
    /// Determines the source file extension using a multi-tiered fallback strategy:
    /// 1. Inspects raw URL path extension (e.g. `http://site.com/video.webm?token=123` -> `"webm"`).
    /// 2. Inspects MIME Content-Type header if provided (e.g. `"image/png"` -> `"png"`).
    /// 3. Inspects current destination file path extension.
    pub fn detect_src_extension(dest_path: &Path, url: &str, file_type: Option<&str>) -> String {
        // Strategy 1: Check URL path
        if let Some(pos) = url.find('?') {
            let path_part = &url[..pos];
            if let Some(dot_pos) = path_part.rfind('.') {
                let ext = &path_part[dot_pos + 1..];
                if ext.len() <= 5 && ext.chars().all(|c| c.is_alphanumeric()) {
                    return ext.to_lowercase();
                }
            }
        } else if let Some(dot_pos) = url.rfind('.') {
            let ext = &url[dot_pos + 1..];
            if ext.len() <= 5 && ext.chars().all(|c| c.is_alphanumeric()) {
                return ext.to_lowercase();
            }
        }

        // Strategy 2: Check MIME Content-Type
        if let Some(ft) = file_type {
            let ext = match ft.to_lowercase().as_str() {
                "image/png" => "png",
                "image/jpeg" | "image/jpg" => "jpg",
                "image/webp" => "webp",
                "image/gif" => "gif",
                "image/heic" => "heic",
                "application/pdf" => "pdf",
                "video/mp4" => "mp4",
                "video/webm" => "webm",
                "video/x-matroska" => "mkv",
                "audio/mpeg" | "audio/mp3" => "mp3",
                "audio/wav" => "wav",
                "audio/x-m4a" | "audio/m4a" => "m4a",
                _ => "",
            };
            if !ext.is_empty() {
                return ext.to_string();
            }
        }

        // Strategy 3: Fall back to existing file extension
        dest_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
    }
}
