//! An in-memory [`HttpTransport`] for tests: canned responses per URL, and a
//! record of what was requested.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use futures::future::BoxFuture;

use crate::{HttpError, HttpTransport, Progress, ProgressFn, Request, Response};

#[derive(Default)]
pub struct FakeTransport {
    responses: Mutex<HashMap<String, Response>>,
    requests: Mutex<Vec<Request>>,
}

impl FakeTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers requests (and downloads) for `url` with `response`.
    pub fn respond(self, url: impl Into<String>, response: Response) -> Self {
        self.responses.lock().unwrap().insert(url.into(), response);
        self
    }

    /// The requests made so far, oldest first. Downloads are recorded as GETs.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    fn answer(&self, request: Request) -> Result<Response, HttpError> {
        let url = request.url.clone();
        self.requests.lock().unwrap().push(request);
        self.responses
            .lock()
            .unwrap()
            .get(&url)
            .cloned()
            .ok_or_else(|| HttpError::Other(format!("no response set for {url}")))
    }
}

impl HttpTransport for FakeTransport {
    fn send(&self, request: Request) -> BoxFuture<'_, Result<Response, HttpError>> {
        Box::pin(async move { self.answer(request) })
    }

    fn download<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
        on_progress: ProgressFn,
    ) -> BoxFuture<'a, Result<u64, HttpError>> {
        Box::pin(async move {
            let response = self.answer(Request::get(url))?;
            if !response.is_success() {
                return Err(HttpError::Status {
                    status: response.status,
                    body: response.text(),
                });
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(destination, &response.body)?;
            let size = response.body.len() as u64;
            on_progress(Progress {
                downloaded_bytes: size,
                total_bytes: Some(size),
            });
            Ok(size)
        })
    }
}
