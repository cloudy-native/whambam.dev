// SPDX-License-Identifier: MIT
//! `--assert` checks such as `p99<200ms`, `rps>=1000`, `errors<0.5%` or,
//! against a baseline, `p99<+10%`.

use anyhow::{Result, anyhow, bail};
use serde::Serialize;

use super::metric::{Kind, Metric, Metrics};
use crate::cli::parse::parse_duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
}

impl Op {
    fn holds(self, actual: f64, threshold: f64) -> bool {
        match self {
            Op::Lt => actual < threshold,
            Op::Le => actual <= threshold,
            Op::Gt => actual > threshold,
            Op::Ge => actual >= threshold,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Threshold {
    Absolute(f64),
    /// Percent change from the baseline value, e.g. `+10%`.
    Relative(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assertion {
    pub text: String,
    pub metric: Metric,
    pub op: Op,
    pub threshold: Threshold,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssertResult {
    pub assertion: String,
    pub metric: Metric,
    pub actual: f64,
    pub threshold: f64,
    pub passed: bool,
}

impl Assertion {
    pub fn parse(s: &str) -> Result<Self> {
        let text = s.trim().replace(' ', "");
        let (pos, op, len) = ["<=", ">=", "<", ">"]
            .iter()
            .find_map(|o| text.find(o).map(|p| (p, *o, o.len())))
            .ok_or_else(|| anyhow!("invalid assertion '{s}': expected e.g. p99<200ms"))?;
        let op = match op {
            "<=" => Op::Le,
            ">=" => Op::Ge,
            "<" => Op::Lt,
            _ => Op::Gt,
        };
        let metric = Metric::parse(&text[..pos])?;
        let value = &text[pos + len..];
        let threshold =
            parse_threshold(metric, value).map_err(|e| anyhow!("invalid assertion '{s}': {e}"))?;
        Ok(Assertion {
            text,
            metric,
            op,
            threshold,
        })
    }

    pub fn needs_baseline(&self) -> bool {
        matches!(self.threshold, Threshold::Relative(_))
    }

    pub fn check(
        &self,
        current: &dyn Metrics,
        baseline: Option<&dyn Metrics>,
    ) -> Result<AssertResult> {
        let threshold = match self.threshold {
            Threshold::Absolute(v) => v,
            Threshold::Relative(pct) => {
                let base = baseline.ok_or_else(|| {
                    anyhow!(
                        "'{}' compares with a baseline: add --baseline FILE",
                        self.text
                    )
                })?;
                base.metric(self.metric) * (1.0 + pct / 100.0)
            }
        };
        let actual = current.metric(self.metric);
        Ok(AssertResult {
            assertion: self.text.clone(),
            metric: self.metric,
            actual,
            threshold,
            passed: self.op.holds(actual, threshold),
        })
    }
}

fn parse_threshold(metric: Metric, value: &str) -> Result<Threshold> {
    if value.is_empty() {
        bail!("missing value");
    }
    let number = |v: &str| {
        v.parse::<f64>()
            .map_err(|_| anyhow!("'{v}' is not a number"))
    };
    if (value.starts_with('+') || value.starts_with('-')) && value.ends_with('%') {
        return Ok(Threshold::Relative(number(&value[..value.len() - 1])?));
    }
    let v = match metric.kind() {
        Kind::Percent { .. } => number(value.trim_end_matches('%'))?,
        Kind::Throughput => number(value)?,
        Kind::Latency if value.ends_with(|c: char| c.is_ascii_alphabetic()) => {
            parse_duration(value)?.as_secs_f64() * 1e3
        }
        Kind::Latency => number(value)?,
    };
    Ok(Threshold::Absolute(v))
}

/// Check every assertion; the first configuration error (a relative check
/// with no baseline) is returned as `Err`.
pub fn check_all(
    asserts: &[Assertion],
    current: &dyn Metrics,
    baseline: Option<&dyn Metrics>,
) -> Result<Vec<AssertResult>> {
    asserts.iter().map(|a| a.check(current, baseline)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(f64);
    impl Metrics for Fixed {
        fn metric(&self, _: Metric) -> f64 {
            self.0
        }
    }

    #[test]
    fn parses_units_and_operators() {
        let a = Assertion::parse("p99 < 200ms").unwrap();
        assert_eq!(
            (a.metric, a.op, a.threshold),
            (Metric::P99, Op::Lt, Threshold::Absolute(200.0))
        );
        assert_eq!(
            Assertion::parse("mean<=1.5s").unwrap().threshold,
            Threshold::Absolute(1500.0)
        );
        assert_eq!(
            Assertion::parse("max<250").unwrap().threshold,
            Threshold::Absolute(250.0)
        );
        assert_eq!(Assertion::parse("rps>=1000").unwrap().op, Op::Ge);
        assert_eq!(
            Assertion::parse("errors<0.5%").unwrap().threshold,
            Threshold::Absolute(0.5)
        );
        let rel = Assertion::parse("p99<+10%").unwrap();
        assert_eq!(rel.threshold, Threshold::Relative(10.0));
        assert!(rel.needs_baseline());
        assert_eq!(
            Assertion::parse("rps>-5%").unwrap().threshold,
            Threshold::Relative(-5.0)
        );
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["p99", "p99<", "p42<1ms", "rps>fast", "p99<10parsecs"] {
            assert!(Assertion::parse(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn checks_absolute_and_relative_thresholds() {
        let now = Fixed(105.0);
        let base = Fixed(100.0);
        let pass = Assertion::parse("p99<+10%")
            .unwrap()
            .check(&now, Some(&base))
            .unwrap();
        assert!(pass.passed);
        assert!((pass.threshold - 110.0).abs() < 1e-9);
        let fail = Assertion::parse("rps>=110")
            .unwrap()
            .check(&now, None)
            .unwrap();
        assert!(!fail.passed);
        assert!(
            Assertion::parse("p99<+10%")
                .unwrap()
                .check(&now, None)
                .is_err()
        );
        let all = check_all(&[Assertion::parse("p50<200").unwrap()], &now, None).unwrap();
        assert!(all[0].passed);
    }
}
