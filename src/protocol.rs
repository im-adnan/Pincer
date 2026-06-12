use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use std::pin::Pin;
use tokio_stream::Stream;

pub struct ResolvedMetadata {
    pub total_size: Option<u64>,
    pub is_resumable: Option<bool>,
    pub file_type: Option<String>,
}

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    async fn resolve_metadata(
        &self,
        url: &str,
        headers: &[String],
    ) -> Result<ResolvedMetadata, String>;

    async fn download_chunk(
        &self,
        url: &str,
        start: u64,
        end: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>, String>;
}

pub struct HttpAdapter {
    client: Client,
}

impl HttpAdapter {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ProtocolAdapter for HttpAdapter {
    async fn resolve_metadata(
        &self,
        url: &str,
        headers: &[String],
    ) -> Result<ResolvedMetadata, String> {
        let mut req = self.client.get(url);

        for h in headers {
            if let Some((k, v)) = h.split_once(':') {
                if let (Ok(hk), Ok(hv)) = (
                    reqwest::header::HeaderName::from_bytes(k.trim().as_bytes()),
                    reqwest::header::HeaderValue::from_str(v.trim()),
                ) {
                    req = req.header(hk, hv);
                }
            }
        }

        let res = req.send().await.map_err(|e| e.to_string())?;

        let status = res.status();
        if !status.is_success() {
            return Err(format!("Server returned error: {}", status));
        }

        let total_size = res.content_length();
        let file_type = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let supports_ranges = res
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .map(|val| val == "bytes")
            .unwrap_or(false)
            || (status == reqwest::StatusCode::PARTIAL_CONTENT);

        Ok(ResolvedMetadata {
            total_size,
            is_resumable: Some(supports_ranges),
            file_type,
        })
    }

    async fn download_chunk(
        &self,
        url: &str,
        start: u64,
        end: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>, String> {
        let range_val = format!("bytes={}-{}", start, end);
        let mut req = self.client.get(url);

        if let Ok(hv) = reqwest::header::HeaderValue::from_str(&range_val) {
            req = req.header(reqwest::header::RANGE, hv);
        }

        let res = req.send().await.map_err(|e| e.to_string())?;

        if start > 0 && res.status() == reqwest::StatusCode::OK {
            return Err(
                "Server does not support resuming (returned 200 OK instead of 206 Partial Content)"
                    .to_string(),
            );
        }

        if !res.status().is_success() {
            return Err(format!("Server returned error: {}", res.status()));
        }

        let stream = res.bytes_stream().map(|r| r.map_err(|e| e.to_string()));
        Ok(Box::pin(stream))
    }
}

pub struct FtpAdapter;

impl FtpAdapter {
    pub fn new() -> Self {
        Self
    }

    async fn connect(&self, url: &str) -> Result<suppaftp::tokio::AsyncFtpStream, String> {
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        let host = parsed.host_str().unwrap_or("localhost");
        let port = parsed.port().unwrap_or(21);
        let address = format!("{}:{}", host, port);

        let mut stream = suppaftp::tokio::AsyncFtpStream::connect(&address)
            .await
            .map_err(|e| e.to_string())?;
        let user = if parsed.username().is_empty() {
            "anonymous"
        } else {
            parsed.username()
        };
        let pass = parsed.password().unwrap_or("anonymous@");

        stream.login(user, pass).await.map_err(|e| e.to_string())?;
        Ok(stream)
    }
}

#[async_trait]
impl ProtocolAdapter for FtpAdapter {
    async fn resolve_metadata(
        &self,
        url: &str,
        _headers: &[String],
    ) -> Result<ResolvedMetadata, String> {
        let mut stream = self.connect(url).await?;
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        let path = parsed.path();

        stream
            .transfer_type(suppaftp::types::FileType::Binary)
            .await
            .map_err(|e| e.to_string())?;

        let size = stream.size(path).await.ok();

        let _ = stream.quit().await;

        Ok(ResolvedMetadata {
            total_size: size.map(|s| s as u64),
            is_resumable: Some(size.is_some()),
            file_type: None,
        })
    }

    async fn download_chunk(
        &self,
        url: &str,
        start: u64,
        end: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>, String> {
        let mut stream = self.connect(url).await?;
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        let path = parsed.path().to_string();
        let end_limit = end;
        let start_offset = start;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let _ = stream
                .transfer_type(suppaftp::types::FileType::Binary)
                .await;
            if start_offset > 0 {
                let _ = stream.resume_transfer(start_offset as usize).await;
            }

            if let Ok(mut data_stream) = stream.retr_as_stream(&path).await {
                let mut buffer = vec![0; 8192];
                let mut current_pos = start_offset;
                use tokio::io::AsyncReadExt;

                loop {
                    // end is inclusive, so we fetch up to end + 1
                    if current_pos > end_limit {
                        break;
                    }

                    let to_read =
                        std::cmp::min(buffer.len() as u64, (end_limit + 1) - current_pos) as usize;
                    if to_read == 0 {
                        break;
                    }

                    match data_stream.read(&mut buffer[..to_read]).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let bytes = Bytes::copy_from_slice(&buffer[..n]);
                            if tx.send(Ok(bytes)).await.is_err() {
                                break;
                            }
                            current_pos += n as u64;
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e.to_string())).await;
                            break;
                        }
                    }
                }

                // Finalize the stream (some FTP servers require reading the final OK code)
                let _ = stream.finalize_retr_stream(data_stream).await;
            } else {
                let _ = tx
                    .send(Err("Failed to start data stream".to_string()))
                    .await;
            }
            let _ = stream.quit().await;
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }
}

use russh::client::{Config, Handler};
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

struct SftpClientHandler;

#[async_trait]
impl Handler for SftpClientHandler {
    type Error = russh::Error;
}

pub struct SftpAdapter;

impl SftpAdapter {
    pub fn new() -> Self {
        Self
    }

    async fn connect(&self, url: &str) -> Result<SftpSession, String> {
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        let host = parsed.host_str().unwrap_or("localhost");
        let port = parsed.port().unwrap_or(22);
        let address = format!("{}:{}", host, port);

        let user = if parsed.username().is_empty() {
            "root"
        } else {
            parsed.username()
        };
        let pass = parsed.password().unwrap_or("");

        let config = Arc::new(Config::default());
        let mut session = russh::client::connect(config, address, SftpClientHandler)
            .await
            .map_err(|e| e.to_string())?;

        let auth_res = session
            .authenticate_password(user, pass)
            .await
            .map_err(|e| e.to_string())?;

        if !matches!(auth_res, russh::client::AuthResult::Success) {
            return Err("SFTP authentication failed".to_string());
        }

        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| e.to_string())?;

        // Initialize subsystem
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| e.to_string())?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| e.to_string())?;
        Ok(sftp)
    }
}

#[async_trait]
impl ProtocolAdapter for SftpAdapter {
    async fn resolve_metadata(
        &self,
        url: &str,
        _headers: &[String],
    ) -> Result<ResolvedMetadata, String> {
        let sftp = self.connect(url).await?;
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        let path = parsed.path();

        let stat = sftp.metadata(path).await.map_err(|e| e.to_string())?;

        Ok(ResolvedMetadata {
            total_size: stat.size,
            is_resumable: Some(true),
            file_type: None,
        })
    }

    async fn download_chunk(
        &self,
        url: &str,
        start: u64,
        end: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, String>> + Send>>, String> {
        let sftp = self.connect(url).await?;
        let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
        let path = parsed.path().to_string();

        let mut file = sftp.open(&path).await.map_err(|e| e.to_string())?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let end_limit = end;

        tokio::spawn(async move {
            let mut current_pos = start;
            let mut buffer = vec![0; 8192];

            loop {
                if current_pos > end_limit {
                    break;
                }

                let to_read =
                    std::cmp::min(buffer.len() as u64, (end_limit + 1) - current_pos) as u32;
                if to_read == 0 {
                    break;
                }

                match file.read(&mut buffer[..to_read as usize]).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let bytes = Bytes::copy_from_slice(&buffer[..n]);
                        if tx.send(Ok(bytes)).await.is_err() {
                            break;
                        }
                        current_pos += n as u64;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string())).await;
                        break;
                    }
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }
}
