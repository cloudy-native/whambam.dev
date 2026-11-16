// SPDX-License-Identifier: MIT
//! Send one request and measure it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::{Client, RequestBuilder};

use super::errors::{ErrorKind, classify};
use crate::config::{RequestSpec, TestConfig};

/// Result of a single request: an HTTP status or a transport error kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub status: Result<u16, ErrorKind>,
    pub bytes_received: u64,
}

/// Everything a worker needs to send the configured request.
#[derive(Clone)]
pub struct Sender {
    client: Client,
    spec: Arc<RequestSpec>,
    timeout: Option<Duration>,
}

impl Sender {
    pub fn new(client: Client, config: &TestConfig) -> Self {
        Sender {
            client,
            spec: Arc::new(config.request.clone()),
            timeout: config.timeout,
        }
    }

    /// Send the request and read the whole body. The returned latency covers
    /// sending through the last body byte (end to end, not time to first byte).
    pub async fn send(&self) -> (Outcome, Duration) {
        let started = Instant::now();
        let outcome = match self.builder().send().await {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let mut bytes = 0u64;
                loop {
                    match resp.chunk().await {
                        Ok(Some(chunk)) => bytes += chunk.len() as u64,
                        Ok(None) => {
                            break Outcome {
                                status: Ok(status),
                                bytes_received: bytes,
                            };
                        }
                        Err(e) => {
                            break Outcome {
                                status: Err(classify(&e)),
                                bytes_received: bytes,
                            };
                        }
                    }
                }
            }
            Err(e) => Outcome {
                status: Err(classify(&e)),
                bytes_received: 0,
            },
        };
        (outcome, started.elapsed())
    }

    pub fn builder(&self) -> RequestBuilder {
        build_request(&self.client, &self.spec, self.timeout)
    }
}

pub fn build_request(
    client: &Client,
    spec: &RequestSpec,
    timeout: Option<Duration>,
) -> RequestBuilder {
    let mut req = client
        .request(spec.method.clone(), spec.url.clone())
        .headers(spec.headers.clone());
    if let Some(body) = &spec.body {
        req = req.body(body.clone());
    }
    if let Some((user, pass)) = &spec.basic_auth {
        req = req.basic_auth(user, Some(pass));
    }
    if let Some(t) = timeout {
        req = req.timeout(t);
    }
    req
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::http::build_clients;
    use clap::Parser;
    use test_utils::MockServer;

    fn sender(url: &str, extra: &[&str]) -> Sender {
        let args = ["whambam", url].into_iter().chain(extra.iter().copied());
        let config = TestConfig::from_cli(&Cli::try_parse_from(args).unwrap()).unwrap();
        let client = build_clients(&config).unwrap().remove(0);
        Sender::new(client, &config)
    }

    #[tokio::test]
    async fn successful_request_reports_status_and_bytes() {
        let server = MockServer::start().await;
        let (outcome, latency) = sender(&server.url(), &[]).send().await;
        assert_eq!(outcome.status, Ok(200));
        assert_eq!(outcome.bytes_received, 13);
        assert!(latency > Duration::ZERO);
    }

    #[tokio::test]
    async fn body_auth_and_headers_are_sent() {
        let server = MockServer::start().await;
        let s = sender(
            &server.url(),
            &["-m", "POST", "-d", "hello", "-a", "u:p", "-H", "X-A: 1"],
        );
        s.send().await;
        let headers = server.received_headers();
        assert_eq!(headers["x-a"][0], "1");
        assert!(headers["authorization"][0].starts_with("Basic "));
        assert_eq!(server.last_body().as_deref(), Some(&b"hello"[..]));
        assert_eq!(server.last_method().as_deref(), Some("POST"));
    }

    #[tokio::test]
    async fn timeout_is_classified() {
        let server = MockServer::start().await;
        server.set_response_delay(500);
        let (outcome, _) = sender(&server.url(), &["-t", "50ms"]).send().await;
        assert_eq!(outcome.status, Err(ErrorKind::Timeout));
    }

    #[tokio::test]
    async fn http2_prior_knowledge_works() {
        let server = MockServer::start().await;
        let (outcome, _) = sender(&server.url(), &["--http2"]).send().await;
        assert_eq!(outcome.status, Ok(200));
        assert_eq!(server.last_version().as_deref(), Some("HTTP/2.0"));
    }
}
