// SPDX-License-Identifier: MIT
//! Demo target with realistic, varying latency and a few errors, used to record the README GIF and the
//! explainer videos: `cargo run --release -p test-utils --bin demo-server -- 8080 [--mode]`
//!
//! Modes (one flag after the port; default is the wave):
//! - `--fast`       answers at once with no errors, a target for comparing load generators.
//! - `--stall`      answers in about 2 ms but freezes for 1 s at the start of every 5 s (coordinated omission).
//! - `--ratelimit`  answers in a few ms, but returns 429 above 1,500 requests per second (a hidden rate limiter).
//! - `--coldstart`  starts slow (caches, pools and JIT still cold) and settles over about 15 s.
//! - `--slowbody`   sends headers at once, then trickles a 64 KiB body in 16 chunks, 20 ms apart.
//! - `--steady`     2-6 ms with no errors (\"the current code\"); `--slower` adds 12 ms (\"the new code\").
//! - `--help`       lists the modes.

mod modes;
mod trickle;

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;

use modes::{Mode, State, handle};

fn print_help() {
    println!("demo-server: a local target for practising load tests with whambam\n");
    println!("usage: demo-server [PORT] [--MODE]      (default 8080, wave)\n\nmodes:");
    for (flag, mode) in Mode::ALL {
        println!("  --{flag:<11} {}", mode.describe());
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }
    let mode = args
        .iter()
        .find_map(|a| a.strip_prefix("--").and_then(Mode::parse))
        .unwrap_or(Mode::Wave);
    let port = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map_or("8080", String::as_str);
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    eprintln!(
        "demo server --{} ({}) on http://127.0.0.1:{port}",
        mode.flag(),
        mode.describe()
    );
    let state = State::new(mode);
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req| handle(state.clone(), req));
            let _ = Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}
