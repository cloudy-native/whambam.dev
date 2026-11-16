// SPDX-License-Identifier: MIT
//! End-to-end tests of the whambam binary against a local mock server.

use assert_cmd::Command;
use predicates::str::contains;
use test_utils::MockServer;

fn whambam() -> Command {
    Command::cargo_bin("whambam").unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn text_report_has_hey_sections() {
    let server = MockServer::start().await;
    whambam()
        .args([&server.url(), "-n", "10", "-c", "2", "--no-ui"])
        .assert()
        .success()
        .stdout(contains("Summary:"))
        .stdout(contains("Requests/sec:"))
        .stdout(contains("Latency distribution:"))
        .stdout(contains("[200]\t10 responses"));
    assert_eq!(server.request_count(), 10);
}

#[tokio::test(flavor = "multi_thread")]
async fn json_report_is_valid() {
    let server = MockServer::start().await;
    let output = whambam()
        .args([&server.url(), "-n", "5", "-c", "1", "--no-ui", "-o", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["summary"]["requests"], 5);
    assert_eq!(report["status_codes"]["200"], 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn csv_rows_go_to_the_output_file() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rows.csv");
    whambam()
        .args([
            &server.url(),
            "-n",
            "4",
            "-c",
            "2",
            "-o",
            "csv",
            "--no-ui",
            "--output",
        ])
        .arg(&path)
        .assert()
        .success();
    let text = std::fs::read_to_string(path).unwrap();
    assert_eq!(text.lines().count(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn piped_stdout_falls_back_to_headless() {
    let server = MockServer::start().await;
    // No --no-ui: stdout is a pipe here, so the UI must not start.
    whambam()
        .args([&server.url(), "-n", "3", "-c", "1"])
        .assert()
        .success()
        .stdout(contains("Summary:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn debug_prints_one_exchange() {
    let server = MockServer::start().await;
    whambam()
        .args([&server.url(), "--debug", "-H", "X-Trace: 1"])
        .assert()
        .success()
        .stdout(contains("> x-trace: 1"))
        .stdout(contains("< HTTP/1.1 200 OK"));
    assert_eq!(server.request_count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn errors_are_reported_by_kind() {
    whambam()
        .args(["http://127.0.0.1:1", "-n", "3", "-c", "1", "--no-ui"])
        .assert()
        .success()
        .stdout(contains("Error distribution:"))
        .stdout(contains("connection refused"));
}

#[test]
fn invalid_arguments_fail_with_a_message() {
    whambam()
        .args(["http://x", "-c", "0", "--no-ui"])
        .assert()
        .failure()
        .stderr(contains("-c must be at least 1"));
    whambam()
        .args(["ftp://x", "--no-ui"])
        .assert()
        .failure()
        .stderr(contains("unsupported URL scheme"));
    whambam()
        .args(["http://x", "-p", "2", "--no-ui"])
        .assert()
        .failure()
        .stderr(contains("requires --http2"));
}

#[test]
fn version_and_help_work() {
    whambam()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
    whambam()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("--qps"))
        .stdout(contains("--connect-to"));
}

#[tokio::test(flavor = "multi_thread")]
async fn failing_assertions_exit_non_zero() {
    let server = MockServer::start().await;
    whambam()
        .args([
            &server.url(),
            "-n",
            "5",
            "-c",
            "1",
            "--no-ui",
            "--assert",
            "rps>1000000,errors<1%",
        ])
        .assert()
        .failure()
        .stdout(contains("✗ rps>1000000"))
        .stdout(contains("✓ errors<1%"))
        .stderr(contains("1 of 2 assertions failed: rps>1000000"));
    whambam()
        .args([
            &server.url(),
            "-n",
            "5",
            "-c",
            "1",
            "--no-ui",
            "--assert",
            "p99<10s",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn baseline_run_and_compare_command() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base.json");
    let next = dir.path().join("next.json");
    whambam()
        .args([
            &server.url(),
            "-n",
            "20",
            "-c",
            "2",
            "--no-ui",
            "-o",
            "json",
            "--output",
        ])
        .arg(&base)
        .assert()
        .success();
    let output = whambam()
        .args([
            &server.url(),
            "-n",
            "20",
            "-c",
            "2",
            "--no-ui",
            "-o",
            "json",
            "--baseline",
        ])
        .arg(&base)
        .args(["--assert", "success>=+0%", "--output"])
        .arg(&next)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(&next).unwrap()).unwrap();
    assert_eq!(report["comparison"].as_array().unwrap().len(), 8);
    assert_eq!(report["assertions"][0]["passed"], true);

    whambam()
        .arg("compare")
        .arg(&base)
        .arg(&next)
        .assert()
        .success()
        .stdout(contains("Compared with baseline:"));
    whambam()
        .args(["http://x", "--no-ui", "--assert", "p99<+10%"])
        .assert()
        .failure()
        .stderr(contains("add --baseline FILE"));
}
