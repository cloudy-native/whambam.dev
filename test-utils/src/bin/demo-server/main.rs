// SPDX-License-Identifier: MIT
//! Demo target with realistic, varying latency and a few errors, used to
//! record the README/Terminal Trove GIF: `cargo run -p test-utils --bin demo-server -- 8080`

use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;

static SEED: AtomicU64 = AtomicU64::new(0x2545_f491_4f6c_dd1d);

/// xorshift: good enough for jitter, no extra dependency.
fn random() -> f64 {
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x % 10_000) as f64 / 10_000.0
}

async fn handle(_req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    // A slow wave every ~10 s plus jitter and an occasional long tail.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let wave = 1.0 + (secs * 0.6).sin().max(0.0) * 6.0;
    let tail = if random() < 0.01 { 40.0 } else { 0.0 };
    let ms = 2.0 + wave * (1.0 + random() * 2.0) + tail;
    tokio::time::sleep(Duration::from_secs_f64(ms / 1e3)).await;
    let status = if random() < 0.004 { 503 } else { 200 };
    let body = Bytes::from_static(b"{\"ok\":true,\"service\":\"demo\"}");
    Ok(Response::builder()
        .status(status)
        .body(Full::new(body))
        .expect("response"))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::args().nth(1).unwrap_or_else(|| "8080".into());
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    eprintln!("demo server on http://127.0.0.1:{port}");
    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let _ = Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(stream), service_fn(handle))
                .await;
        });
    }
}
