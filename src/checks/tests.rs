// SPDX-License-Identifier: MIT

use std::path::Path;

use clap::Parser;

use super::*;
use crate::cli::{Cli, Command};

const BASE: &str = r#"{"summary":{"rps":1000.0,"success_rate":100.0},
  "latency_ms":{"min":1.0,"max":50.0,"mean":10.0,
  "percentiles":{"p50":9.0,"p90":15.0,"p95":20.0,"p99":30.0,"p99.9":45.0}}}"#;

fn write(dir: &Path, name: &str, json: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, json).unwrap();
    path.to_str().unwrap().to_string()
}

fn compare_args(args: &[&str]) -> CompareArgs {
    let cli = Cli::try_parse_from(
        ["whambam", "compare"]
            .into_iter()
            .chain(args.iter().copied()),
    )
    .unwrap();
    match cli.command {
        Some(Command::Compare(a)) => a,
        _ => panic!("expected compare"),
    }
}

#[test]
fn relative_asserts_need_a_baseline() {
    let cli = Cli::try_parse_from(["whambam", "http://x", "--assert", "p99<+10%"]).unwrap();
    let err = Checks::load(&cli.check).unwrap_err().to_string();
    assert!(err.contains("--baseline"), "{err}");
    let cli = Cli::try_parse_from(["whambam", "http://x", "--assert", "p99<10ms"]).unwrap();
    assert!(Checks::load(&cli.check).is_ok());
}

#[test]
fn verdict_lists_failures() {
    assert!(verdict(None).is_ok());
    let failed = AssertResult {
        assertion: "p99<1ms".into(),
        metric: crate::report::Metric::P99,
        actual: 5.0,
        threshold: 1.0,
        passed: false,
    };
    let err = verdict(Some(&[failed])).unwrap_err().to_string();
    assert_eq!(err, "1 of 1 assertions failed: p99<1ms");
}

#[test]
fn compare_prints_a_table_and_applies_asserts() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.json", BASE);
    let b = write(
        dir.path(),
        "b.json",
        &BASE.replace("\"p99\":30.0", "\"p99\":36.0"),
    );

    let mut out = Vec::new();
    run_compare(&compare_args(&[&a, &b, "--assert", "p99<+10%"]), &mut out).unwrap_err();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Compared with baseline:"));
    assert!(text.contains("+20.0% ▲"), "{text}");
    assert!(text.contains("✗ p99<+10%"));

    let mut out = Vec::new();
    run_compare(
        &compare_args(&[&a, &b, "--assert", "p99<+25%", "-o", "json"]),
        &mut out,
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(json["comparison"].as_array().unwrap().len(), 8);
    assert_eq!(json["assertions"][0]["passed"], true);
}
