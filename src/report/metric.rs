// SPDX-License-Identifier: MIT
//! Named metrics shared by --assert, --baseline and `whambam compare`.

use std::fmt;

use anyhow::{Result, bail};
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Rps,
    SuccessRate,
    ErrorRate,
    Min,
    Mean,
    Max,
    P50,
    P90,
    P95,
    P99,
    P999,
}

/// How a metric is measured, which decides units and what "better" means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Milliseconds; lower is better.
    Latency,
    /// Requests per second; higher is better.
    Throughput,
    /// Percent; higher is better for success, lower for errors.
    Percent { higher_is_better: bool },
}

impl Metric {
    /// Rows shown by comparisons, in order.
    pub const COMPARED: [Metric; 8] = [
        Metric::Rps,
        Metric::SuccessRate,
        Metric::Mean,
        Metric::P50,
        Metric::P90,
        Metric::P99,
        Metric::P999,
        Metric::Max,
    ];

    pub fn parse(name: &str) -> Result<Self> {
        Ok(match name.trim().to_ascii_lowercase().as_str() {
            "rps" | "throughput" => Metric::Rps,
            "success" | "success_rate" => Metric::SuccessRate,
            "errors" | "error_rate" => Metric::ErrorRate,
            "min" | "fastest" => Metric::Min,
            "mean" | "avg" | "average" => Metric::Mean,
            "max" | "slowest" => Metric::Max,
            "p50" | "median" => Metric::P50,
            "p90" => Metric::P90,
            "p95" => Metric::P95,
            "p99" => Metric::P99,
            "p99.9" | "p999" => Metric::P999,
            other => bail!(
                "unknown metric '{other}': use rps, success, errors, min, mean, max, p50, p90, p95, p99 or p99.9"
            ),
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Metric::Rps => "rps",
            Metric::SuccessRate => "success",
            Metric::ErrorRate => "errors",
            Metric::Min => "min",
            Metric::Mean => "mean",
            Metric::Max => "max",
            Metric::P50 => "p50",
            Metric::P90 => "p90",
            Metric::P95 => "p95",
            Metric::P99 => "p99",
            Metric::P999 => "p99.9",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Metric::Rps => Kind::Throughput,
            Metric::SuccessRate => Kind::Percent {
                higher_is_better: true,
            },
            Metric::ErrorRate => Kind::Percent {
                higher_is_better: false,
            },
            _ => Kind::Latency,
        }
    }

    /// Key in the report's `latency_ms.percentiles` map, for percentiles.
    pub fn percentile_key(self) -> Option<&'static str> {
        match self {
            Metric::P50 => Some("p50"),
            Metric::P90 => Some("p90"),
            Metric::P95 => Some("p95"),
            Metric::P99 => Some("p99"),
            Metric::P999 => Some("p99.9"),
            _ => None,
        }
    }

    pub fn higher_is_better(self) -> bool {
        match self.kind() {
            Kind::Latency => false,
            Kind::Throughput => true,
            Kind::Percent { higher_is_better } => higher_is_better,
        }
    }

    /// Format a value of this metric with its unit.
    pub fn format(self, value: f64) -> String {
        match self.kind() {
            Kind::Latency if value >= 1000.0 => format!("{:.2}s", value / 1e3),
            Kind::Latency => format!("{value:.2}ms"),
            Kind::Throughput => format!("{value:.1} req/s"),
            Kind::Percent { .. } => format!("{value:.2}%"),
        }
    }
}

/// Serialized with the same names `--assert` accepts (`p99.9`, `success`, ...).
impl Serialize for Metric {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Anything that can report a value for each metric: a live report or a
/// baseline loaded from JSON.
pub trait Metrics {
    fn metric(&self, m: Metric) -> f64;
}

/// Percent change from `base` to `current` (`None` when `base` is zero).
pub fn change_pct(base: f64, current: f64) -> Option<f64> {
    (base != 0.0).then(|| (current - base) / base * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for m in Metric::COMPARED
            .iter()
            .chain(&[Metric::ErrorRate, Metric::Min, Metric::P95])
        {
            assert_eq!(Metric::parse(m.name()).unwrap(), *m);
        }
        assert_eq!(Metric::parse("Median").unwrap(), Metric::P50);
        assert!(Metric::parse("p42").is_err());
    }

    #[test]
    fn direction_and_format() {
        assert!(Metric::Rps.higher_is_better());
        assert!(!Metric::P99.higher_is_better());
        assert!(!Metric::ErrorRate.higher_is_better());
        assert_eq!(Metric::P99.format(12.345), "12.35ms");
        assert_eq!(Metric::Max.format(1500.0), "1.50s");
        assert_eq!(Metric::SuccessRate.format(99.5), "99.50%");
        assert_eq!(change_pct(100.0, 110.0), Some(10.0));
        assert_eq!(change_pct(0.0, 5.0), None);
        assert_eq!(serde_json::to_string(&Metric::P999).unwrap(), "\"p99.9\"");
    }
}
