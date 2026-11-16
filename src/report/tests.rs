// SPDX-License-Identifier: MIT

use std::time::Duration;

use clap::Parser;

use super::*;
use crate::cli::{Cli, OutputFormat};
use crate::http::Outcome;
use crate::stats::RequestMetric;

fn config(extra: &[&str]) -> TestConfig {
    let args = ["whambam", "http://example.com/x"]
        .into_iter()
        .chain(extra.iter().copied());
    TestConfig::from_cli(&Cli::try_parse_from(args).unwrap()).unwrap()
}

fn state(latencies_ms: &[u64], errors: usize) -> TestState {
    let mut s = TestState::default();
    for &ms in latencies_ms {
        s.record(&RequestMetric {
            start_offset: Duration::ZERO,
            latency: Duration::from_millis(ms),
            outcome: Outcome {
                status: Ok(200),
                bytes_received: 100,
            },
            bytes_sent: 10,
        });
    }
    for _ in 0..errors {
        s.record(&RequestMetric {
            start_offset: Duration::ZERO,
            latency: Duration::from_millis(1),
            outcome: Outcome {
                status: Err(ErrorKind::Timeout),
                bytes_received: 0,
            },
            bytes_sent: 10,
        });
    }
    s.mark_complete(false);
    s
}

#[test]
fn histogram_counts_every_sample_once() {
    let latencies: Vec<u64> = (1..=100).collect();
    let r = Report::new(&state(&latencies, 0), &config(&[]));
    assert_eq!(r.histogram.len(), HISTOGRAM_BUCKETS);
    assert_eq!(r.histogram.iter().map(|b| b.count).sum::<u64>(), 100);
    assert!((r.histogram.last().unwrap().le_ms - 100.0).abs() < 0.2);
}

#[test]
fn histogram_handles_identical_and_empty_samples() {
    let same = Report::new(&state(&[5, 5, 5], 0), &config(&[]));
    assert_eq!(same.histogram.len(), 1);
    assert_eq!(same.histogram[0].count, 3);
    assert!(
        Report::new(&TestState::default(), &config(&[]))
            .histogram
            .is_empty()
    );
}

#[test]
fn summary_and_load_reflect_state_and_config() {
    let r = Report::new(&state(&[10, 20], 2), &config(&["--qps", "50", "-c", "4"]));
    assert_eq!(r.summary.requests, 4);
    assert_eq!(r.summary.successes, 2);
    assert_eq!(r.summary.success_rate, 50.0);
    assert_eq!(r.summary.size_per_request, 50);
    assert_eq!(r.errors[&ErrorKind::Timeout], 2);
    assert_eq!(r.load.pacing, "global");
    assert_eq!(r.load.rate, Some(50.0));
    assert!(r.load.latency_corrected);
    assert_eq!(r.target.url, "http://example.com/x");
    assert_eq!(r.latency_ms.percentiles.len(), PERCENTILES.len());
}

#[test]
fn json_output_matches_schema_keys() {
    let r = Report::new(&state(&[10], 1), &config(&[]));
    let value = serde_json::to_value(&r).unwrap();
    for key in [
        "schema_version",
        "target",
        "load",
        "summary",
        "latency_ms",
        "histogram",
        "status_codes",
        "errors",
    ] {
        assert!(value.get(key).is_some(), "missing {key}");
    }
    assert_eq!(value["status_codes"]["200"], 1);
    assert_eq!(value["errors"]["timeout"], 1);
    assert!(value["latency_ms"]["percentiles"]["p99.9"].is_number());

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../schema/report.schema.json")).unwrap();
    let required = schema["required"].as_array().unwrap();
    for key in required {
        assert!(
            value.get(key.as_str().unwrap()).is_some(),
            "schema requires {key}"
        );
    }
}

#[test]
fn text_output_has_hey_sections() {
    let r = Report::new(&state(&[10, 20, 30], 1), &config(&["--qps", "10"]));
    let mut out = Vec::new();
    write_text(&mut out, &r).unwrap();
    let text = String::from_utf8(out).unwrap();
    for section in [
        "Summary:",
        "Requests/sec:",
        "Response time histogram:",
        "Latency distribution:",
        "99% in",
        "Status code distribution:",
        "[200]\t3 responses",
        "Error distribution:",
        "[1]\ttimeout",
        "coordinated omission",
    ] {
        assert!(text.contains(section), "missing {section:?} in\n{text}");
    }
}

#[test]
fn write_report_writes_files_and_skips_quiet() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("r.json");
    let s = state(&[1], 0);
    write_report(OutputFormat::Json, &s, &config(&[]), Some(&path)).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(parsed["summary"]["requests"], 1);

    let quiet = dir.path().join("q.txt");
    write_report(OutputFormat::Quiet, &s, &config(&[]), Some(&quiet)).unwrap();
    assert!(!quiet.exists());
}
