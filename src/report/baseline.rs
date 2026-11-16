// SPDX-License-Identifier: MIT
//! Load a saved JSON report as a baseline and compare against it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::metric::{Metric, Metrics, change_pct};

/// The parts of a saved `-o json` report that comparisons need.
#[derive(Debug, Clone, Deserialize)]
pub struct Baseline {
    #[serde(skip)]
    pub source: String,
    pub summary: BaseSummary,
    pub latency_ms: BaseLatency,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BaseSummary {
    pub rps: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BaseLatency {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub percentiles: BTreeMap<String, f64>,
}

impl Baseline {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read baseline {}", path.display()))?;
        Self::parse(&text, &path.display().to_string())
    }

    pub fn parse(json: &str, source: &str) -> Result<Self> {
        let mut baseline: Baseline = serde_json::from_str(json)
            .with_context(|| format!("{source} is not a whambam JSON report (-o json)"))?;
        baseline.source = source.to_string();
        Ok(baseline)
    }
}

impl Metrics for Baseline {
    fn metric(&self, m: Metric) -> f64 {
        let l = &self.latency_ms;
        match m {
            Metric::Rps => self.summary.rps,
            Metric::SuccessRate => self.summary.success_rate,
            Metric::ErrorRate => 100.0 - self.summary.success_rate,
            Metric::Min => l.min,
            Metric::Mean => l.mean,
            Metric::Max => l.max,
            p => p
                .percentile_key()
                .and_then(|k| l.percentiles.get(k).copied())
                .unwrap_or(0.0),
        }
    }
}

/// One row of a comparison.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonRow {
    pub metric: Metric,
    pub baseline: f64,
    pub current: f64,
    /// Percent change; `null` when the baseline is zero.
    pub change_pct: Option<f64>,
    /// True when the change is in the good direction (or no change).
    pub improved: bool,
}

/// Compare the standard metrics of `current` with `baseline`.
pub fn compare(baseline: &impl Metrics, current: &impl Metrics) -> Vec<ComparisonRow> {
    Metric::COMPARED
        .iter()
        .map(|&metric| {
            let (b, c) = (baseline.metric(metric), current.metric(metric));
            let improved = if metric.higher_is_better() {
                c >= b
            } else {
                c <= b
            };
            ComparisonRow {
                metric,
                baseline: b,
                current: c,
                change_pct: change_pct(b, c),
                improved,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const SAMPLE: &str = r#"{
        "schema_version": 1,
        "summary": { "rps": 1000.0, "success_rate": 99.0, "requests": 10 },
        "latency_ms": { "min": 1.0, "max": 90.0, "mean": 10.0, "stdev": 2.0,
            "percentiles": { "p50": 8.0, "p90": 20.0, "p95": 30.0, "p99": 50.0, "p99.9": 80.0 } }
    }"#;

    #[test]
    fn parses_a_saved_report() {
        let b = Baseline::parse(SAMPLE, "base.json").unwrap();
        assert_eq!(b.source, "base.json");
        assert_eq!(b.metric(Metric::Rps), 1000.0);
        assert_eq!(b.metric(Metric::ErrorRate), 1.0);
        assert_eq!(b.metric(Metric::P999), 80.0);
        assert!(Baseline::parse("{}", "x").is_err());
        assert!(Baseline::load(Path::new("/no/such/report.json")).is_err());
    }

    #[test]
    fn comparison_marks_direction() {
        let base = Baseline::parse(SAMPLE, "a").unwrap();
        let mut better = base.clone();
        better.summary.rps = 1100.0;
        better.latency_ms.percentiles.insert("p99".into(), 60.0);
        let rows = compare(&base, &better);
        let rps = rows.iter().find(|r| r.metric == Metric::Rps).unwrap();
        assert!(rps.improved);
        assert_eq!(rps.change_pct, Some(10.0));
        let p99 = rows.iter().find(|r| r.metric == Metric::P99).unwrap();
        assert!(!p99.improved);
        assert_eq!(p99.change_pct, Some(20.0));
    }
}
