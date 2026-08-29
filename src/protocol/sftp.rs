//! SftpAdapter via russh & russh-sftp
//!
//! ### Architectural Overview
//! - **What it does**: Implements the `ProtocolAdapter` trait for downloading files securely over SSH File Transfer Protocol (`sftp://`).
//! - **How it does**: Connects to SSH servers via `russh`, establishes SFTP subsystem channels via `russh_sftp::client::SftpSession`, queries file attributes (`stat`), and streams byte ranges using async seek and read.
//! - **Where it comes from**: Instantiated by `protocol::get_adapter()` for `sftp://` URLs.
//! - **Where it leads to**: Feeds downloaded SFTP byte buffers to `engine::DownloadWorker`.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use russh::*;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use url::Url;

use super::{ProtocolAdapter, ResolvedMetadata};
use crate::common::{PincerError, PincerResult};

/// SSH client callback handler implementing `russh::client::Handler`.
struct SshClientHandler;

#[async_trait]
impl client::Handler for SshClientHandler {
    type Error = russh::Error;
}

/// SFTP protocol adapter supporting secure file transfers over SSH.
pub struct SftpAdapter;

impl Default for SftpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SftpAdapter {
    /// Creates a new `SftpAdapter`.
    pub fn new() -> Self {
        Self
    }

    /// Parses host, port, username, password, and file path from an SFTP URL.
    fn parse_url(url_str: &str) -> PincerResult<(String, u16, String, Option<String>, String)> {
        let url = Url::parse(url_str).map_err(|e| PincerError::InvalidUrl(e.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| PincerError::InvalidUrl("Missing SFTP host".into()))?
            .to_string();
        let port = url.port().unwrap_or(22);
        let username = if url.username().is_empty() {
            "anonymous"
        } else {
            url.username()
        }
        .to_string();
        let password = url.password().map(|s| s.to_string());
        let path = url.path().to_string();
        Ok((host, port, username, password, path))
    }

    /// Establishes an authenticated SSH session and opens the SFTP subsystem channel.
    async fn connect_sftp(
        host: &str,
        port: u16,
        user: &str,
        pass: Option<&str>,
    ) -> PincerResult<(
        client::Handle<SshClientHandler>,
        russh_sftp::client::SftpSession,
    )> {
        let config = Arc::new(client::Config::default());
        let mut handle = client::connect(config, (host, port), SshClientHandler)
            .await
            .map_err(|e| PincerError::Protocol(format!("SSH connect failed: {}", e)))?;

        if let Some(password) = pass {
            let auth_res = handle
                .authenticate_password(user, password)
                .await
                .map_err(|e| PincerError::Protocol(format!("SSH auth error: {}", e)))?;
            if !auth_res.success() {
                return Err(PincerError::Protocol(
                    "SSH password authentication rejected".into(),
                ));
            }
        }

        let channel = handle.channel_open_session().await.map_err(|e| {
            PincerError::Protocol(format!("Failed to open SSH session channel: {}", e))
        })?;

        channel.request_subsystem(true, "sftp").await.map_err(|e| {
            PincerError::Protocol(format!("Failed to request SFTP subsystem: {}", e))
        })?;

        let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| {
                PincerError::Protocol(format!("Failed to initialize SFTP session: {}", e))
            })?;

        Ok((handle, sftp))
    }
}

#[async_trait]
impl ProtocolAdapter for SftpAdapter {
    /// Queries file attributes (`stat`) on the remote SFTP server to determine file length.
    async fn get_content_length(
        &self,
        url: &str,
        _headers: &[String],
    ) -> PincerResult<ResolvedMetadata> {
        let (host, port, user, pass, path) = Self::parse_url(url)?;
        let (_handle, sftp) = Self::connect_sftp(&host, port, &user, pass.as_deref()).await?;

        let stat = sftp
            .metadata(&path)
            .await
            .map_err(|e| PincerError::Protocol(format!("SFTP metadata error: {}", e)))?;

        let filename = path.split('/').next_back().map(|s| s.to_string());

        Ok(ResolvedMetadata {
            content_length: stat.size,
            resumable: true,
            filename,
            file_type: None,
        })
    }

    /// Reads byte blocks starting from `start` up to `end` via SFTP stream.
    async fn get_stream(
        &self,
        url: &str,
        start: u64,
        end: u64,
        _headers: &[String],
    ) -> PincerResult<BoxStream<'static, PincerResult<Bytes>>> {
        let (host, port, user, pass, path) = Self::parse_url(url)?;
        let (_handle, sftp) = Self::connect_sftp(&host, port, &user, pass.as_deref()).await?;

        let mut file = sftp
            .open(&path)
            .await
            .map_err(|e| PincerError::Protocol(format!("SFTP open error: {}", e)))?;
        if start > 0 {
            file.seek(SeekFrom::Start(start))
                .await
                .map_err(PincerError::Io)?;
        }

        let (tx, rx) = tokio::sync::mpsc::channel::<PincerResult<Bytes>>(32);
        let expected_bytes = if end >= start && end > 0 {
            end - start + 1
        } else {
            u64::MAX
        };

        tokio::spawn(async move {
            let mut bytes_read_total = 0u64;
            let chunk_size = 65536usize;

            loop {
                let to_read = if expected_bytes != u64::MAX {
                    let rem = (expected_bytes - bytes_read_total) as usize;
                    if rem == 0 {
                        break;
                    }
                    std::cmp::min(chunk_size, rem)
                } else {
                    chunk_size
                };

                let mut buf = vec![0u8; to_read];
                match file.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.truncate(n);
                        bytes_read_total += n as u64;
                        if tx.send(Ok(Bytes::from(buf))).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(PincerError::Io(e))).await;
                        break;
                    }
                }
            }
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}
