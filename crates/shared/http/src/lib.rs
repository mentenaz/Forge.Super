//! Small async HTTP client for package registries, runtime downloads,
//! and Helm GitHub API.
//!
//! Uses `reqwest` configured with `rustls` (no native OpenSSL/Schannel
//! dependencies) and identifies itself using `branding::PRODUCT`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde::de::DeserializeOwned;
use thiserror::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("HTTP status {status}: {body}")]
    Status {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("JSON decode error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Download progress event: bytes downloaded so far and total bytes expected
/// (if reported by the server).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient {
    /// Creates a new `HttpClient` with Forge Super's user agent and a 30s timeout.
    pub fn new() -> Self {
        let user_agent = format!("{}/{}", branding::PRODUCT.short, "0.1.0");
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// Creates an `HttpClient` with custom headers or custom timeout if needed.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Performs an HTTP GET request and parses the body as JSON.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(HttpError::Status { status, body });
        }
        let text = response.text().await?;
        let value = serde_json::from_str::<T>(&text)?;
        Ok(value)
    }

    /// Performs an HTTP GET request and returns raw text.
    pub async fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(HttpError::Status { status, body });
        }
        Ok(response.text().await?)
    }

    /// Downloads a file to `destination`, calling `on_progress` as chunks arrive.
    ///
    /// Writes to a temporary sibling file first and atomically renames on completion.
    pub async fn download_file(
        &self,
        url: &str,
        destination: &Path,
        on_progress: impl Fn(Progress) + Send + Sync + 'static,
    ) -> Result<u64, HttpError> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(HttpError::Status { status, body });
        }

        let total_bytes = response.content_length();
        let mut stream = response.bytes_stream();

        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;

        let temp_path = destination.with_extension(format!("tmp-{}", std::process::id()));
        let mut file = std::fs::File::create(&temp_path)?;

        let mut downloaded_bytes: u64 = 0;
        let on_progress = Arc::new(on_progress);

        on_progress(Progress {
            downloaded_bytes: 0,
            total_bytes,
        });

        use std::io::Write;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
            downloaded_bytes += chunk.len() as u64;
            on_progress(Progress {
                downloaded_bytes,
                total_bytes,
            });
        }
        file.flush()?;
        drop(file);

        // Atomic rename / overwrite
        std::fs::rename(&temp_path, destination)?;

        Ok(downloaded_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_client_creates_cleanly() {
        let client = HttpClient::new();
        let _ = client.clone();
    }

    #[tokio::test]
    async fn non_existent_url_errors() {
        let client = HttpClient::new();
        let res: Result<String, _> = client.get_text("http://127.0.0.1:9").await;
        assert!(res.is_err());
    }
}
