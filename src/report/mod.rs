// SPDX-License-Identifier: MIT
//! The report model. Every output (text, JSON, TUI summary) renders from it.

mod csv;
mod output;
mod text;

use std::collections::BTreeMap;

use serde::Serialize;

pub use csv::CsvSink;
pub use output::write_report;
pub use text::write_text;

use crate::cli::HttpVersion;
use crate::config::{Pacing, TestConfig};
use crate::http::ErrorKind;
use crate::stats::TestState;

/// Bump when the JSON shape changes incompatibly.
pub const SCHEMA_VERSION: u32 = 1;

/// Quantiles reported everywhere, as (label, q).
pub const PERCENTILES: [(&str, f64); 8] = [
    ("p10", 0.10),
    ("p25", 0.25),
    ("p50", 0.50),
    ("p75", 0.75),
    ("p90", 0.90),
    ("p95", 0.95),
    ("p99", 0.99),
    ("p99.9", 0.999),
];

const HISTOGRAM_BUCKETS: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub whambam_version: &'static str,
    pub target: Target,
    pub load: Load,
    pub summary: Summary,
    pub latency_ms: Latency,
    pub histogram: Vec<Bucket>,
    pub status_codes: BTreeMap<u16, usize>,
    pub errors: BTreeMap<ErrorKind, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub url: String,
    pub method: String,
    pub http_version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct Load {
    pub concurrency: usize,
    pub workers: usize,
    pub requests: Option<usize>,
    pub duration_secs: Option<f64>,
    pub pacing: &'static str,
    pub rate: Option<f64>,
    pub latency_corrected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub total_secs: f64,
    pub requests: usize,
    pub successes: usize,
    pub failures: usize,
    pub success_rate: f64,
    pub rps: f64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub size_per_request: u64,
    pub interrupted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Latency {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub stdev: f64,
    pub percentiles: BTreeMap<&'static str, f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Bucket {
    /// Upper edge of the bucket in milliseconds.
    pub le_ms: f64,
    pub count: u64,
}

impl Report {
    pub fn new(state: &TestState, config: &TestConfig) -> Self {
        let (pacing, rate) = match config.pacing {
            Pacing::Unlimited => ("unlimited", None),
            Pacing::PerWorker(r) => ("per_worker", Some(r)),
            Pacing::Global(r) => ("global", Some(r)),
        };
        Report {
            schema_version: SCHEMA_VERSION,
            whambam_version: env!("CARGO_PKG_VERSION"),
            target: Target {
                url: config.request.display_url.clone(),
                method: config.request.method.to_string(),
                http_version: match config.net.http_version {
                    HttpVersion::Http11 => "1.1",
                    HttpVersion::Http2 => "2",
                },
            },
            load: Load {
                concurrency: config.concurrency,
                workers: config.workers(),
                requests: config.requests,
                duration_secs: config.duration.map(|d| d.as_secs_f64()),
                pacing,
                rate,
                latency_corrected: config.pacing.is_corrected(),
            },
            summary: Summary {
                total_secs: state.elapsed().as_secs_f64(),
                requests: state.completed,
                successes: state.successes,
                failures: state.failures(),
                success_rate: state.success_rate(),
                rps: state.rps(),
                bytes_received: state.bytes_received,
                bytes_sent: state.bytes_sent,
                size_per_request: state
                    .bytes_received
                    .checked_div(state.completed as u64)
                    .unwrap_or(0),
                interrupted: state.interrupted,
            },
            latency_ms: Latency {
                min: state.min_ms(),
                max: state.max_ms(),
                mean: state.mean_ms(),
                stdev: state.stdev_ms(),
                percentiles: PERCENTILES
                    .iter()
                    .map(|&(l, q)| (l, state.percentile_ms(q)))
                    .collect(),
            },
            histogram: histogram(state),
            status_codes: state.status_counts.clone(),
            errors: state.error_counts.clone(),
        }
    }
}

/// Linear buckets between the fastest and slowest sample, built from the real
/// recorded counts.
fn histogram(state: &TestState) -> Vec<Bucket> {
    let h = &state.histogram;
    if h.is_empty() {
        return Vec::new();
    }
    let (lo, hi) = (h.min() as f64, h.max() as f64);
    let buckets = if h.equivalent(h.min(), h.max()) {
        1
    } else {
        HISTOGRAM_BUCKETS
    };
    let step = (hi - lo) / buckets as f64;
    let mut counts = vec![0u64; buckets];
    for v in h.iter_recorded() {
        let value = h.highest_equivalent(v.value_iterated_to()) as f64;
        let idx = if step > 0.0 {
            ((value - lo) / step) as usize
        } else {
            0
        };
        counts[idx.min(buckets - 1)] += v.count_at_value();
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(i, count)| Bucket {
            le_ms: if buckets == 1 {
                hi / 1e3
            } else {
                (lo + step * (i + 1) as f64) / 1e3
            },
            count,
        })
        .collect()
}

#[cfg(test)]
mod tests;
