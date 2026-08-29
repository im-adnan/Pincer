//! Universal resolve orchestrator
//!
//! ### Architectural Overview
//! - **What it does**: Inspects any incoming URL, Magnet link, or media stream to resolve final URLs, inferred file names, Content-Length, and media types.
//! - **How it does**: Routes through a tiered resolution strategy: (1) magnet link inspection via `librqbit`, (2) `.torrent` file discovery, (3) direct HTTP `GET` with header extraction via `DirectHttpResolver`, and (4) HTML OpenGraph / JSON script scraping via `HtmlScraper` and `ScriptExtractor`.
//! - **Where it comes from**: Called by `manager.resolve_url()` on behalf of `rpc::handlers::ResolveHandlers` and `cli::DirectDownloader`.
//! - **Where it leads to**: Returns a normalized `ResolveResponse` data structure to the caller.

use librqbit::{AddTorrent, AddTorrentOptions, Session};
use std::collections::HashMap;
use std::sync::Arc;

use super::direct::DirectHttpResolver;
use super::html_scraper::HtmlScraper;
use super::script_extractor::ScriptExtractor;
use super::torrent_resolver::TorrentResolver;
use crate::models::ResolveResponse;

/// Central entry point for universal URL, Magnet link, and media stream metadata discovery.
pub struct UniversalResolver;

impl UniversalResolver {
    /// Resolves metadata for any arbitrary URL, Magnet link, or direct media stream.
    ///
    /// Multi-tier resolution strategy:
    /// 1. Magnet links: Resolves metadata via `librqbit` list-only mode (15-second timeout).
    /// 2. Direct `.torrent` links: Fetches torrent bytes and parses inner file tree.
    /// 3. Direct HTTP media streams: Follows redirects, inspects Content-Type, Content-Length, and Accept-Ranges.
    /// 4. HTML web pages: Scrapes OpenGraph tags, Next.js hydration JSON scripts, and direct media URLs.
    /// 5. Extension inference: Infers file extensions from MIME types if missing from filename.
    pub async fn resolve_url(
        url: String,
        session: &Arc<Session>,
        global_opts: &HashMap<String, String>,
    ) -> Result<ResolveResponse, String> {
        let url_trimmed = url.trim().to_string();
        let url_lower = url_trimmed.to_lowercase();

        // Stage 1: Magnet link metadata discovery
        if url_lower.starts_with("magnet:?") {
            let torrent_source = AddTorrent::from_url(url_trimmed.clone());
            let opts = AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };

            let add_res_timeout = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                session.add_torrent(torrent_source, Some(opts)),
            )
            .await;

            let add_res = match add_res_timeout {
                Ok(res) => res.map_err(|e| format!("Failed to resolve magnet: {:?}", e))?,
                Err(_) => {
                    // On timeout, parse static magnet parameters from URL string
                    let parsed = librqbit::Magnet::parse(&url_trimmed)
                        .map_err(|e| format!("Invalid magnet URL: {:?}", e))?;
                    let name = parsed.name.clone();
                    return Ok(ResolveResponse {
                        url: url_trimmed,
                        filename: name,
                        total_size: None,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: None,
                    });
                }
            };

            return match add_res {
                librqbit::AddTorrentResponse::ListOnly(res) => {
                    let (name, total_size, files) = TorrentResolver::parse_info(&res.info);
                    Ok(ResolveResponse {
                        url: url_trimmed,
                        filename: name,
                        total_size,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: Some(files),
                    })
                }
                _ => Err("Expected ListOnly response from magnet link resolver".to_string()),
            };
        }

        // Build HTTP client with custom User-Agent and Proxy support if configured
        let mut builder = reqwest::Client::builder();
        if let Some(ua) = global_opts.get("user-agent").filter(|s| !s.is_empty()) {
            builder = builder.user_agent(ua);
        } else {
            builder = builder.user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        }
        if let Some(proxy_url) = global_opts.get("all-proxy").filter(|s| !s.is_empty()) {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
        let client = builder.build().map_err(|e| e.to_string())?;

        // Stage 2: Direct .torrent file URL
        let is_direct_torrent = url_lower
            .split('?')
            .next()
            .unwrap_or("")
            .ends_with(".torrent");
        if is_direct_torrent {
            let response = client
                .get(&url_trimmed)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let final_url = response.url().to_string();
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;

            let torrent_source = AddTorrent::from_bytes(bytes.to_vec());
            let opts = AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };
            let add_res = session
                .add_torrent(torrent_source, Some(opts))
                .await
                .map_err(|e| format!("Failed to resolve torrent link: {:?}", e))?;

            return match add_res {
                librqbit::AddTorrentResponse::ListOnly(res) => {
                    let (name, total_size, files) = TorrentResolver::parse_info(&res.info);
                    Ok(ResolveResponse {
                        url: final_url,
                        filename: name,
                        total_size,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: Some(files),
                    })
                }
                _ => Err("Expected ListOnly response from torrent link resolution".to_string()),
            };
        }

        // Stage 3: Direct HTTP probe
        let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let meta = DirectHttpResolver::extract_metadata(&response);

        // Check if response returned a torrent MIME type
        if meta.content_type.contains("application/x-bittorrent")
            || meta
                .final_url
                .split('?')
                .next()
                .unwrap_or("")
                .ends_with(".torrent")
        {
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
            let torrent_source = AddTorrent::from_bytes(bytes.to_vec());
            let opts = AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };
            let add_res = session
                .add_torrent(torrent_source, Some(opts))
                .await
                .map_err(|e| format!("Failed to resolve torrent link: {:?}", e))?;

            return match add_res {
                librqbit::AddTorrentResponse::ListOnly(res) => {
                    let (name, total_size, files) = TorrentResolver::parse_info(&res.info);
                    Ok(ResolveResponse {
                        url: meta.final_url,
                        filename: name,
                        total_size,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: Some(files),
                    })
                }
                _ => Err("Expected ListOnly response from torrent link resolver".to_string()),
            };
        }

        // Direct media files (video streams)
        if meta.content_type.contains("video/")
            || meta
                .final_url
                .split('?')
                .next()
                .unwrap_or("")
                .ends_with(".mp4")
            || meta
                .final_url
                .split('?')
                .next()
                .unwrap_or("")
                .ends_with(".mkv")
        {
            let filename = meta.content_disposition_filename.unwrap_or_else(|| {
                meta.final_url
                    .split('/')
                    .next_back()
                    .unwrap_or("download.bin")
                    .split('?')
                    .next()
                    .unwrap_or("download.bin")
                    .to_string()
            });

            return Ok(ResolveResponse {
                url: meta.final_url,
                filename: Some(filename),
                total_size: meta.total_size,
                file_type: Some(meta.content_type),
                is_resumable: meta.is_resumable,
                torrent_files: None,
            });
        }

        // Stage 4: HTML Fallback scraping for media streams
        if meta.content_type.contains("text/html") {
            let html = response.text().await.map_err(|e| e.to_string())?;

            if let Some(res) = HtmlScraper::scrape_opengraph(&html) {
                return Ok(res);
            }
            if let Some(res) = ScriptExtractor::extract_from_json_scripts(&html) {
                return Ok(res);
            }
            if let Some(res) = HtmlScraper::scrape_media_links(&html) {
                return Ok(res);
            }
        }

        // Stage 5: Final fallback with extension inference
        let mut filename = meta.content_disposition_filename.unwrap_or_else(|| {
            meta.final_url
                .split('/')
                .next_back()
                .unwrap_or("download.bin")
                .split('?')
                .next()
                .unwrap_or("download.bin")
                .to_string()
        });

        if !filename.contains('.') {
            let ext = DirectHttpResolver::infer_extension(&meta.content_type);
            if !ext.is_empty() {
                filename = format!("{}.{}", filename, ext);
            }
        }

        Ok(ResolveResponse {
            url: meta.final_url,
            filename: Some(filename),
            total_size: meta.total_size,
            file_type: if meta.content_type.is_empty() {
                None
            } else {
                Some(meta.content_type)
            },
            is_resumable: meta.is_resumable,
            torrent_files: None,
        })
    }
}
