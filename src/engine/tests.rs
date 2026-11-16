// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant};

use clap::Parser;
use test_utils::MockServer;

use super::*;
use crate::cli::Cli;
use crate::http::ErrorKind;

fn config(url: &str, extra: &[&str]) -> Arc<TestConfig> {
    let args = ["whambam", url].into_iter().chain(extra.iter().copied());
    Arc::new(TestConfig::from_cli(&Cli::try_parse_from(args).unwrap()).unwrap())
}

async fn run_to_end(config: Arc<TestConfig>) -> SharedState {
    let state = SharedState::default();
    run(config, state.clone(), CancellationToken::new(), None)
        .await
        .unwrap();
    state
}

#[tokio::test]
async fn sends_exactly_the_requested_count() {
    let server = MockServer::start().await;
    let state = run_to_end(config(&server.url(), &["-n", "37", "-c", "5"])).await;
    let s = state.snapshot();
    assert_eq!(s.completed, 37);
    assert_eq!(server.request_count(), 37);
    assert_eq!(s.status_counts[&200], 37);
    assert_eq!(s.bytes_received, 37 * 13);
    assert!(s.is_complete() && !s.interrupted);
}

#[tokio::test]
async fn fewer_requests_than_workers_is_fine() {
    let server = MockServer::start().await;
    let state = run_to_end(config(&server.url(), &["-n", "2", "-c", "10"])).await;
    assert_eq!(state.snapshot().completed, 2);
}

#[tokio::test]
async fn duration_stops_the_run() {
    let server = MockServer::start().await;
    server.set_response_delay(5);
    let started = Instant::now();
    let state = run_to_end(config(&server.url(), &["-z", "300ms", "-c", "4"])).await;
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(300), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(900), "{elapsed:?}");
    assert!(state.snapshot().completed > 10);
}

#[tokio::test]
async fn http_errors_and_transport_errors_are_split() {
    let server = MockServer::start().await;
    server.set_response_status(503);
    let s = run_to_end(config(&server.url(), &["-n", "6", "-c", "2"]))
        .await
        .snapshot();
    assert_eq!(s.status_counts[&503], 6);
    assert_eq!(s.successes, 0);

    let refused = run_to_end(config("http://127.0.0.1:1", &["-n", "4", "-c", "2"]))
        .await
        .snapshot();
    assert_eq!(refused.error_counts[&ErrorKind::ConnectionRefused], 4);
    assert!(refused.status_counts.is_empty());
}

#[tokio::test]
async fn global_rate_is_respected() {
    let server = MockServer::start().await;
    let started = Instant::now();
    let s = run_to_end(config(&server.url(), &["-n", "10", "--qps", "50"]))
        .await
        .snapshot();
    assert_eq!(s.completed, 10);
    // Slot 9 is due at 180 ms.
    assert!(started.elapsed() >= Duration::from_millis(180));
}

#[tokio::test]
async fn global_rate_charges_queueing_time() {
    // One worker, 100 ms per request, asked for 20 req/s: requests fall behind
    // schedule, and corrected latency must include the time spent waiting.
    let server = MockServer::start().await;
    server.set_response_delay(100);
    let corrected = run_to_end(config(
        &server.url(),
        &["-n", "6", "-c", "1", "--qps", "20"],
    ))
    .await
    .snapshot();
    assert!(corrected.max_ms() > 300.0, "max {}", corrected.max_ms());

    let hey = run_to_end(config(&server.url(), &["-n", "6", "-c", "1", "-q", "20"]))
        .await
        .snapshot();
    assert!(hey.max_ms() < 250.0, "max {}", hey.max_ms());
}

#[tokio::test]
async fn per_worker_rate_limits_each_worker() {
    let server = MockServer::start().await;
    let started = Instant::now();
    run_to_end(config(&server.url(), &["-n", "6", "-c", "2", "-q", "10"])).await;
    // Each worker sends 3 at 10/s: ticks at 0, 100, 200 ms.
    assert!(started.elapsed() >= Duration::from_millis(200));
}

#[tokio::test]
async fn cancelling_marks_the_run_interrupted() {
    let server = MockServer::start().await;
    server.set_response_delay(20);
    let state = SharedState::default();
    let stop = CancellationToken::new();
    let cfg = config(&server.url(), &["-n", "0", "-c", "2"]);
    let handle = tokio::spawn(run(cfg, state.clone(), stop.clone(), None));
    tokio::time::sleep(Duration::from_millis(100)).await;
    stop.cancel();
    handle.await.unwrap().unwrap();
    let s = state.snapshot();
    assert!(s.interrupted);
    assert!(s.completed > 0);
}

#[tokio::test]
async fn http2_multiplexes_parallel_streams() {
    let server = MockServer::start().await;
    let s = run_to_end(config(
        &server.url(),
        &["-n", "20", "-c", "2", "--http2", "-p", "3"],
    ))
    .await
    .snapshot();
    assert_eq!(s.completed, 20);
    assert_eq!(server.last_version().as_deref(), Some("HTTP/2.0"));
}

#[tokio::test]
async fn csv_sink_receives_every_request() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.csv");
    let sink = CsvSink::create(Some(&path)).unwrap();
    let state = SharedState::default();
    let cfg = config(&server.url(), &["-n", "5", "-c", "2"]);
    let sink = run(cfg, state, CancellationToken::new(), Some(sink))
        .await
        .unwrap();
    sink.unwrap().finish().unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    assert_eq!(text.lines().count(), 6);
    assert!(text.starts_with("offset_s,latency_ms,status,error,bytes_received"));
}
