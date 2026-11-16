// SPDX-License-Identifier: MIT
//! Baseline and assertion flags, and the `compare` subcommand.

use std::path::PathBuf;

use super::OutputFormat;
use crate::report::Assertion;

fn parse_assertion(s: &str) -> anyhow::Result<Assertion> {
    Assertion::parse(s)
}

#[derive(clap::Args, Debug, Clone, Default)]
#[command(next_help_heading = "Checks")]
pub struct CheckArgs {
    /// Compare with a saved report (from `-o json`). Shown in the UI and the
    /// report, and lets --assert use changes like p99<+10%
    #[arg(long, value_name = "FILE")]
    pub baseline: Option<PathBuf>,

    /// Exit with status 1 unless the run meets this check, e.g. "p99<200ms",
    /// "rps>=1000", "errors<1%", "p99<+10%" (vs --baseline). Repeatable or
    /// comma-separated. Metrics: rps, success, errors, min, mean, max, p50,
    /// p90, p95, p99, p99.9
    #[arg(long = "assert", value_name = "CHECK", value_delimiter = ',', value_parser = parse_assertion)]
    pub asserts: Vec<Assertion>,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// Compare two saved reports (from `-o json`), optionally with --assert
    Compare(CompareArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct CompareArgs {
    /// Baseline report
    pub baseline: PathBuf,

    /// Report to compare with the baseline
    pub current: PathBuf,

    /// Exit with status 1 unless CURRENT meets this check (see `whambam --help`)
    #[arg(long = "assert", value_name = "CHECK", value_delimiter = ',', value_parser = parse_assertion)]
    pub asserts: Vec<Assertion>,

    /// text or json
    #[arg(
        short = 'o',
        long = "output-format",
        value_enum,
        default_value = "text"
    )]
    pub output_format: OutputFormat,
}
