// SPDX-License-Identifier: MIT
//! Baselines, assertions and the `compare` command.

use std::io::{self, Write};
use std::sync::Arc;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::cli::{CheckArgs, CompareArgs, OutputFormat};
use crate::report::{
    AssertResult, Assertion, Baseline, ComparisonRow, Metrics, Report, check_all, compare,
    write_assertions, write_comparison,
};

/// What to check a finished run against.
#[derive(Debug, Default)]
pub struct Checks {
    pub baseline: Option<Arc<Baseline>>,
    asserts: Vec<Assertion>,
}

impl Checks {
    /// Load the baseline and validate assertions before the run starts.
    pub fn load(args: &CheckArgs) -> Result<Self> {
        let baseline = args.baseline.as_deref().map(Baseline::load).transpose()?;
        if baseline.is_none()
            && let Some(a) = args.asserts.iter().find(|a| a.needs_baseline())
        {
            bail!(
                "--assert '{}' compares with a baseline: add --baseline FILE",
                a.text
            );
        }
        Ok(Checks {
            baseline: baseline.map(Arc::new),
            asserts: args.asserts.clone(),
        })
    }

    /// Add the comparison and assertion results to the report.
    pub fn apply(&self, report: &mut Report) -> Result<()> {
        let base = self.baseline.as_deref();
        if let Some(b) = base {
            report.comparison = Some(compare(b, report));
        }
        if !self.asserts.is_empty() {
            let base: Option<&dyn Metrics> = base.map(|b| b as &dyn Metrics);
            report.assertions = Some(check_all(&self.asserts, report, base)?);
        }
        Ok(())
    }
}

/// Fail (exit status 1) when any assertion failed.
pub fn verdict(results: Option<&[AssertResult]>) -> Result<()> {
    let results = results.unwrap_or_default();
    let failed: Vec<&str> = results
        .iter()
        .filter(|r| !r.passed)
        .map(|r| r.assertion.as_str())
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        bail!(
            "{} of {} assertions failed: {}",
            failed.len(),
            results.len(),
            failed.join(", ")
        )
    }
}

#[derive(Serialize)]
struct CompareOutput<'a> {
    baseline: String,
    current: String,
    comparison: &'a [ComparisonRow],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    assertions: &'a [AssertResult],
}

/// `whambam compare BASELINE CURRENT`.
pub fn run_compare(args: &CompareArgs, out: &mut impl Write) -> Result<()> {
    let base = Baseline::load(&args.baseline)?;
    let current = Baseline::load(&args.current)?;
    let rows = compare(&base, &current);
    let results = check_all(&args.asserts, &current, Some(&base))?;
    match args.output_format {
        OutputFormat::Json => {
            let output = CompareOutput {
                baseline: base.source.clone(),
                current: current.source.clone(),
                comparison: &rows,
                assertions: &results,
            };
            serde_json::to_writer_pretty(&mut *out, &output)?;
            writeln!(out)?;
        }
        OutputFormat::Quiet => {}
        _ => {
            writeln!(
                out,
                "Baseline: {}\nCurrent:  {}",
                base.source, current.source
            )?;
            write_comparison(out, &rows)?;
            if !results.is_empty() {
                write_assertions(out, &results)?;
            }
        }
    }
    out.flush()?;
    verdict(Some(&results))
}

/// Convenience for the binary: compare to stdout.
pub fn run_compare_stdout(args: &CompareArgs) -> Result<()> {
    run_compare(args, &mut io::stdout().lock())
}

#[cfg(test)]
mod tests;
