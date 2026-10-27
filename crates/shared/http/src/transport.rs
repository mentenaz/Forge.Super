//! The seam backends depend on instead of `HttpClient`: an object-safe trait
//! over "send a request, get a response" and "download a file", so tests can
//! swap in [`crate::FakeTransport`].

use std::path::Path;
use std::sync::Arc;

use futures::future::BoxFuture;
use serde::de::DeserializeOwned;

use crate::{HttpClient, HttpError, Progress};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn with_method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

pub trait HttpTransport: Send + Sync {
    /// Sends `request`. A non-2xx status is a normal [`Response`], not an
    /// error; callers decide what it means.
    fn send(&self, request: Request) -> BoxFuture<'_, Result<Response, HttpError>>;

    /// Downloads `url` to `destination` (atomically), returning the size.
    fn download<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
        on_progress: ProgressFn,
    ) -> BoxFuture<'a, Result<u64, HttpError>>;
}

impl HttpTransport for HttpClient {
    fn send(&self, request: Request) -> BoxFuture<'_, Result<Response, HttpError>> {
        let client = self.client();
        Box::pin(async move {
            crate::joined(
                crate::runtime()
                    .spawn(Self::send_on_runtime(client, request))
                    .await,
            )
        })
    }

    fn download<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
        on_progress: ProgressFn,
    ) -> BoxFuture<'a, Result<u64, HttpError>> {
        Box::pin(self.download_file(url, destination, move |progress| on_progress(progress)))
    }
}

impl HttpClient {
    async fn send_on_runtime(
        client: reqwest::Client,
        request: Request,
    ) -> Result<Response, HttpError> {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder.send().await?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| Some((name.to_string(), value.to_str().ok()?.to_string())))
            .collect();
        let body = response.bytes().await?.to_vec();
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}

/// `get_text` / `get_json` on any transport, including `Arc<dyn HttpTransport>`.
pub trait HttpTransportExt: HttpTransport {
    fn get_text<'a>(&'a self, url: &'a str) -> BoxFuture<'a, Result<String, HttpError>> {
        Box::pin(async move {
            let response = self.send(Request::get(url)).await?;
            if !response.is_success() {
                return Err(HttpError::Status {
                    status: response.status,
                    body: response.text(),
                });
            }
            Ok(response.text())
        })
    }

    fn get_json<'a, T: DeserializeOwned + 'a>(
        &'a self,
        url: &'a str,
    ) -> BoxFuture<'a, Result<T, HttpError>> {
        Box::pin(async move {
            let text = self.get_text(url).await?;
            Ok(serde_json::from_str(&text)?)
        })
    }
}

impl<T: HttpTransport + ?Sized> HttpTransportExt for T {}
