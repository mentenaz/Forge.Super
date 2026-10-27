//! Small async HTTP client for package registries, runtime downloads,
//! and Helm GitHub API.
//!
//! Uses `reqwest` configured with `rustls` (no native OpenSSL/Schannel
//! dependencies) and identifies itself using `branding::PRODUCT`.

mod fake;
mod transport;

pub use fake::FakeTransport;
pub use transport::{HttpTransport, HttpTransportExt, Method, ProgressFn, Request, Response};

use std::path::Path;
use std::sync::OnceLock;
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
    Status { status: u16, body: String },

    #[error("JSON decode error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),

    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("forge-http")
            .enable_all()
            .build()
            .expect("the HTTP runtime starts")
    })
}

fn joined<T>(result: Result<Result<T, HttpError>, tokio::task::JoinError>) -> Result<T, HttpError> {
    result.map_err(|error| HttpError::Other(error.to_string()))?
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

    pub(crate) fn client(&self) -> reqwest::Client {
        self.client.clone()
    }

    /// Creates an `HttpClient` with custom headers or custom timeout if needed.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Performs an HTTP GET request and parses the body as JSON.
    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError> {
        let text = self.get_text(url).await?;
        Ok(serde_json::from_str::<T>(&text)?)
    }

    /// Performs an HTTP GET request and returns raw text.
    pub async fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let client = self.client.clone();
        let url = url.to_string();
        joined(
            runtime()
                .spawn(async move {
                    let response = client.get(&url).send().await?;
                    let status = response.status();
                    if !status.is_success() {
                        let body = response.text().await.unwrap_or_default();
                        return Err(HttpError::Status {
                            status: status.as_u16(),
                            body,
                        });
                    }
                    Ok(response.text().await?)
                })
                .await,
        )
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
        let client = self.client.clone();
        let url = url.to_string();
        let destination = destination.to_path_buf();
        joined(
            runtime()
                .spawn(async move {
                    let response = client.get(&url).send().await?;
                    let status = response.status();
                    if !status.is_success() {
                        let body = response.text().await.unwrap_or_default();
                        return Err(HttpError::Status {
                            status: status.as_u16(),
                            body,
                        });
                    }

                    let total_bytes = response.content_length();
                    let mut stream = response.bytes_stream();

                    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
                    std::fs::create_dir_all(parent)?;

                    let temp_path =
                        destination.with_extension(format!("tmp-{}", std::process::id()));
                    let mut file = std::fs::File::create(&temp_path)?;

                    let mut downloaded_bytes: u64 = 0;
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
                    std::fs::rename(&temp_path, &destination)?;

                    Ok(downloaded_bytes)
                })
                .await,
        )
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

    #[test]
    fn works_from_an_executor_without_tokio() {
        let result = futures::executor::block_on(HttpClient::new().get_text("http://127.0.0.1:9"));
        assert!(result.is_err()); // a connection error, not a panic
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[tokio::test]
    async fn fake_serves_json_and_records_requests() {
        let fake = FakeTransport::new().respond("http://x/a", Response::ok(r#"{"n":3}"#));
        let value: serde_json::Value = fake.get_json("http://x/a").await.unwrap();
        assert_eq!(value["n"], 3);
        assert_eq!(fake.requests().len(), 1);
        assert!(matches!(
            fake.get_text("http://x/missing").await,
            Err(HttpError::Other(_))
        ));
    }

    #[tokio::test]
    async fn fake_reports_status_and_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let fake = FakeTransport::new()
            .respond("http://x/bad", Response::ok("nope").with_status(404))
            .respond("http://x/file", Response::ok("data"));
        assert!(matches!(
            fake.get_text("http://x/bad").await,
            Err(HttpError::Status { status: 404, .. })
        ));
        let dest = dir.path().join("sub").join("file.bin");
        let size = fake
            .download("http://x/file", &dest, std::sync::Arc::new(|_| {}))
            .await
            .unwrap();
        assert_eq!((size, std::fs::read(&dest).unwrap()), (4, b"data".to_vec()));
    }
}
