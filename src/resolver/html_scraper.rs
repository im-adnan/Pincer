//! OpenGraph & HTML media link scraper
//!
//! ### Architectural Overview
//! - **What it does**: Scrapes HTML web pages for embedded media streams via OpenGraph/Twitter meta tags or body media link patterns.
//! - **How it does**: Runs regular expressions to extract `og:video`, `twitter:player`, `og:title`, and direct `.mp4`/`.mkv`/`.webm` URL patterns.
//! - **Where it comes from**: Called as a fallback resolution step in `resolver::UniversalResolver::resolve_url()` when Content-Type is `text/html`.
//! - **Where it leads to**: Returns an inferred `ResolveResponse` pointing directly to the underlying media stream.

use crate::models::ResolveResponse;
use regex::Regex;

/// Scrapes OpenGraph meta tags and direct media streams from raw HTML pages.
pub struct HtmlScraper;

impl HtmlScraper {
    /// Scrapes OpenGraph (`og:video`) or Twitter card (`twitter:player`) video tags and titles from HTML.
    pub fn scrape_opengraph(html: &str) -> Option<ResolveResponse> {
        let og_video_re =
            Regex::new(r##"<meta property="(?:og:video|twitter:player)" content="([^"]*)""##)
                .ok()?;
        let og_title_re =
            Regex::new(r##"<meta property="(?:og:title|twitter:title)" content="([^"]*)""##)
                .ok()?;

        if let Some(caps) = og_video_re.captures(html) {
            let video_url = caps[1].to_string();
            let title = og_title_re
                .captures(html)
                .map(|c| c[1].to_string())
                .unwrap_or_else(|| "download".to_string());

            return Some(ResolveResponse {
                url: video_url,
                filename: Some(format!("{}.mp4", title.replace(' ', "-"))),
                total_size: None,
                file_type: Some("video/mp4".to_string()),
                is_resumable: None,
                torrent_files: None,
            });
        }

        None
    }

    /// Scrapes raw media file links (`.mp4`, `.mkv`, `.webm`, `.mov`) via regex heuristic in HTML body.
    pub fn scrape_media_links(html: &str) -> Option<ResolveResponse> {
        let video_link_re =
            Regex::new(r##"https?://[^\s"'<>]+?\.(?:mp4|mkv|webm|mov)(?:[^\s"'<>]*?)"##).ok()?;

        let mut links: Vec<String> = video_link_re
            .find_iter(html)
            .map(|m| m.as_str().to_string())
            .collect();

        if !links.is_empty() {
            // Sort by length to pick the most descriptive URL
            links.sort_by_key(|a| a.len());
            let best_link = links.last().unwrap();
            let name = best_link
                .split('/')
                .next_back()
                .unwrap_or("download.bin")
                .split('?')
                .next()
                .unwrap_or("download.bin");

            return Some(ResolveResponse {
                url: best_link.to_string(),
                filename: Some(name.to_string()),
                total_size: None,
                file_type: None,
                is_resumable: None,
                torrent_files: None,
            });
        }

        None
    }
}
