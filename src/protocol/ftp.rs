//! FtpAdapter via suppaftp
//!
//! ### Architectural Overview
//! - **What it does**: Implements the `ProtocolAdapter` trait for downloading files over FTP/FTPS endpoints using `suppaftp`.
//! - **How it does**: Connects to FTP servers in passive mode, queries remote file size via `SIZE` command, sets byte restart positions via `REST`, and reads streaming data chunks asynchronously.
//! - **Where it comes from**: Instantiated by `protocol::get_adapter()` for `ftp://` and `ftps://` URLs.
//! - **Where it leads to**: Streams downloaded byte chunks to `engine::DownloadWorker`.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use url::Url;

use super::{ProtocolAdapter, ResolvedMetadata};
use crate::common::{PincerError, PincerResult};

/// FTP/FTPS protocol adapter.
pub struct FtpAdapter;

impl Default for FtpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FtpAdapter {
    /// Creates a new `FtpAdapter`.
    pub fn new() -> Self {
        Self
    }

    /// Parses host, port, username, password, and file path from an FTP URL.
    fn parse_url(url_str: &str) -> PincerResult<(String, u16, String, String, String)> {
        let url = Url::parse(url_str).map_err(|e| PincerError::InvalidUrl(e.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| PincerError::InvalidUrl("Missing FTP host".into()))?
            .to_string();
        let port = url.port().unwrap_or(21);
        let username = if url.username().is_empty() {
            "anonymous"
        } else {
            url.username()
        }
        .to_string();
        let password = url
            .password()
            .unwrap_or("anonymous@pincer.local")
            .to_string();
        let path = url.path().to_string();
        Ok((host, port, username, password, path))
    }
}

#[async_trait]
impl ProtocolAdapter for FtpAdapter {
    /// Connects to FTP server and issues `SIZE <path>` to determine file length.
    async fn get_content_length(
        &self,
        url: &str,
        _headers: &[String],
    ) -> PincerResult<ResolvedMetadata> {
        let (host, port, user, pass, path) = Self::parse_url(url)?;

        let filename = path.split('/').next_back().map(|s| s.to_string());

        let res = tokio::task::spawn_blocking(move || -> PincerResult<Option<u64>> {
            let addr = format!("{}:{}", host, port);
            let mut ftp_stream = suppaftp::FtpStream::connect(addr)
                .map_err(|e| PincerError::Protocol(format!("FTP connect failed: {}", e)))?;

            ftp_stream
                .login(&user, &pass)
                .map_err(|e| PincerError::Protocol(format!("FTP login failed: {}", e)))?;

            let size = ftp_stream.size(&path).map(|s| s as u64).ok();

            let _ = ftp_stream.quit();
            Ok(size)
        })
        .await
        .map_err(|e| PincerError::Other(e.to_string()))??;

        Ok(ResolvedMetadata {
            content_length: res,
            resumable: true,
            filename,
            file_type: None,
        })
    }

    /// Connects to FTP server, sets resume offset via REST, and streams bytes.
    async fn get_stream(
        &self,
        url: &str,
        start: u64,
        end: u64,
        _headers: &[String],
    ) -> PincerResult<BoxStream<'static, PincerResult<Bytes>>> {
        let (host, port, user, pass, path) = Self::parse_url(url)?;
        let expected_bytes = if end >= start && end > 0 {
            end - start + 1
        } else {
            u64::MAX
        };

        let (tx, rx) = tokio::sync::mpsc::channel::<PincerResult<Bytes>>(32);

        tokio::task::spawn_blocking(move || {
            let addr = format!("{}:{}", host, port);
            let mut ftp_stream = match suppaftp::FtpStream::connect(addr) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.blocking_send(Err(PincerError::Protocol(format!(
                        "FTP connect error: {}",
                        e
                    ))));
                    return;
                }
            };

            if let Err(e) = ftp_stream.login(&user, &pass) {
                let _ = tx.blocking_send(Err(PincerError::Protocol(format!(
                    "FTP login error: {}",
                    e
                ))));
                return;
            }

            if start > 0 {
                if let Err(e) = ftp_stream.resume_transfer(start as usize) {
                    let _ = tx.blocking_send(Err(PincerError::Protocol(format!(
                        "FTP resume failed: {}",
                        e
                    ))));
                    return;
                }
            }

            match ftp_stream.retr_as_stream(&path) {
                Ok(mut data_stream) => {
                    use std::io::Read;
                    let mut buffer = [0u8; 65536];
                    let mut bytes_read_total = 0u64;

                    loop {
                        let to_read = if expected_bytes != u64::MAX {
                            let rem = expected_bytes - bytes_read_total;
                            if rem == 0 {
                                break;
                            }
                            std::cmp::min(buffer.len(), rem as usize)
                        } else {
                            buffer.len()
                        };

                        match data_stream.read(&mut buffer[..to_read]) {
                            Ok(0) => break,
                            Ok(n) => {
                                bytes_read_total += n as u64;
                                if tx
                                    .blocking_send(Ok(Bytes::copy_from_slice(&buffer[..n])))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx.blocking_send(Err(PincerError::Io(e)));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(PincerError::Protocol(format!(
                        "FTP RETR error: {}",
                        e
                    ))));
                }
            }

            let _ = ftp_stream.quit();
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }
}
