//! Next.js / Nuxt JSON script extractor
//!
//! ### Architectural Overview
//! - **What it does**: Discovers media file streams embedded within server-side hydrated JSON script payloads (e.g. Next.js `__NEXT_DATA__` and Nuxt `<script type="application/json">` tags).
//! - **How it does**: Matches JSON script tags using regular expressions, deserializes embedded JSON with `serde_json`, searches for media file URLs, and pairs them with page OpenGraph titles.
//! - **Where it comes from**: Called by `resolver::UniversalResolver::resolve_url()` during HTML web page analysis.
//! - **Where it leads to**: Returns a `ResolveResponse` with the extracted direct media link and inferred filename.

use crate::models::ResolveResponse;
use regex::Regex;
use serde_json::Value;

/// Extracts media URLs embedded inside Next.js and Nuxt server-hydrated JSON script tags.
pub struct ScriptExtractor;

impl ScriptExtractor {
    /// Discovers media URLs embedded in Next.js `__NEXT_DATA__` or Nuxt JSON scripts.
    ///
    /// Extraction steps:
    /// 1. Finds `<script id="__NEXT_DATA__">` or `<script type="application/json">` blocks.
    /// 2. Deserializes embedded JSON to string values.
    /// 3. Searches for media video URLs (`.mp4`, `.mkv`, `.webm`, `.mov`).
    /// 4. Extracts OpenGraph page title for a human-readable destination filename.
    pub fn extract_from_json_scripts(html: &str) -> Option<ResolveResponse> {
        let json_re = Regex::new(
            r##"<script[^>]*type="application/json"[^>]*>(.*?)</script>|<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"##,
        ).ok()?;

        let video_link_re =
            Regex::new(r##"https?://[^\s"'<>]+?\.(?:mp4|mkv|webm|mov)(?:[^\s"'<>]*?)"##).ok()?;

        let og_title_re =
            Regex::new(r##"<meta property="(?:og:title|twitter:title)" content="([^"]*)""##).ok();

        for caps in json_re.captures_iter(html) {
            let json_content = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");

            if let Ok(data) = serde_json::from_str::<Value>(json_content) {
                let json_str = data.to_string();
                let mut found_links: Vec<String> = video_link_re
                    .find_iter(&json_str)
                    .map(|m| m.as_str().to_string())
                    .collect();

                if !found_links.is_empty() {
                    found_links.sort_by_key(|a| a.len());
                    if let Some(link) = found_links.last() {
                        let title = og_title_re
                            .as_ref()
                            .and_then(|re| re.captures(html))
                            .map(|c| c[1].to_string())
                            .unwrap_or_else(|| "download".to_string());

                        let ext = link
                            .split('?')
                            .next()
                            .unwrap_or("")
                            .split('.')
                            .next_back()
                            .unwrap_or("mp4")
                            .to_string();

                        return Some(ResolveResponse {
                            url: link.clone(),
                            filename: Some(format!("{}.{}", title.replace(' ', "-"), ext)),
                            total_size: None,
                            file_type: Some(format!("video/{}", ext)),
                            is_resumable: None,
                            torrent_files: None,
                        });
                    }
                }
            }
        }

        None
    }
}
