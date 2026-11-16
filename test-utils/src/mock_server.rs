// SPDX-License-Identifier: MIT
//! A small HTTP/1.1 + HTTP/2 (h2c) server for tests. It records what it
//! receives and answers "Hello, World!" with a configurable status and delay.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;

pub const BODY: &str = "Hello, World!";

#[derive(Default)]
struct Recorded {
    headers: HashMap<String, Vec<String>>,
    body: Option<Vec<u8>>,
    method: Option<String>,
    version: Option<String>,
}

struct State {
    requests: AtomicUsize,
    status: AtomicU16,
    delay_ms: AtomicU64,
    recorded: Mutex<Recorded>,
}

pub struct MockServer {
    port: u16,
    state: Arc<State>,
    task: tokio::task::JoinHandle<()>,
}

impl MockServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let port = listener.local_addr().expect("local addr").port();
        let state = Arc::new(State {
            requests: AtomicUsize::new(0),
            status: AtomicU16::new(200),
            delay_ms: AtomicU64::new(0),
            recorded: Mutex::new(Recorded::default()),
        });
        let accept_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = Arc::clone(&accept_state);
                tokio::spawn(async move {
                    let service = service_fn(move |req| handle(req, Arc::clone(&state)));
                    let _ = Builder::new(TokioExecutor::new())
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        MockServer { port, state, task }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn request_count(&self) -> usize {
        self.state.requests.load(Ordering::SeqCst)
    }

    pub fn set_response_status(&self, status: u16) {
        self.state.status.store(status, Ordering::SeqCst);
    }

    pub fn set_response_delay(&self, delay_ms: u64) {
        self.state.delay_ms.store(delay_ms, Ordering::SeqCst);
    }

    /// Headers seen across all requests, keyed by lower-case name.
    pub fn received_headers(&self) -> HashMap<String, Vec<String>> {
        self.state.recorded.lock().unwrap().headers.clone()
    }

    pub fn last_body(&self) -> Option<Vec<u8>> {
        self.state.recorded.lock().unwrap().body.clone()
    }

    pub fn last_method(&self) -> Option<String> {
        self.state.recorded.lock().unwrap().method.clone()
    }

    pub fn last_version(&self) -> Option<String> {
        self.state.recorded.lock().unwrap().version.clone()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handle(
    req: Request<Incoming>,
    state: Arc<State>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let (parts, body) = req.into_parts();
    let body = body
        .collect()
        .await
        .map(|b| b.to_bytes().to_vec())
        .unwrap_or_default();
    {
        let mut rec = state.recorded.lock().unwrap();
        for (name, value) in &parts.headers {
            let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
            rec.headers
                .entry(name.as_str().to_string())
                .or_default()
                .push(value);
        }
        rec.method = Some(parts.method.to_string());
        rec.version = Some(format!("{:?}", parts.version));
        rec.body = (!body.is_empty()).then_some(body);
    }
    state.requests.fetch_add(1, Ordering::SeqCst);
    let delay = state.delay_ms.load(Ordering::SeqCst);
    if delay > 0 {
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    let response = Response::builder()
        .status(state.status.load(Ordering::SeqCst))
        .header("content-type", "text/plain")
        .body(Full::new(Bytes::from_static(BODY.as_bytes())))
        .expect("valid response");
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_requests_and_serves_status() {
        let server = MockServer::start().await;
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::new();
        let resp = client.get(server.url()).send().await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(resp.text().await.unwrap(), BODY);

        server.set_response_status(404);
        let resp = client
            .post(server.url())
            .header("X-Test", "v")
            .body("data")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 404);
        assert_eq!(server.request_count(), 2);
        assert_eq!(server.received_headers()["x-test"], vec!["v".to_string()]);
        assert_eq!(server.last_body().unwrap(), b"data");
        assert_eq!(server.last_method().unwrap(), "POST");
    }
}
