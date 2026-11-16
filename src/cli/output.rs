// SPDX-License-Identifier: MIT
//! Output and display flags.

use std::path::PathBuf;

use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// hey-style summary
    Text,
    /// Machine-readable summary (see schema/report.schema.json)
    Json,
    /// One row per request
    Csv,
    /// No report
    Quiet,
}

#[derive(clap::Args, Debug, Clone)]
#[command(next_help_heading = "Output")]
pub struct OutputArgs {
    /// Do not show the interactive UI; print the report when done
    #[arg(long = "no-ui", alias = "no-tui")]
    pub no_ui: bool,

    /// Report format. `-o csv` matches hey.
    #[arg(
        short = 'o',
        long = "output-format",
        value_enum,
        default_value = "text"
    )]
    pub output_format: OutputFormat,

    /// Write the report to this file instead of stdout
    #[arg(long = "output", value_name = "FILE")]
    pub output_file: Option<PathBuf>,

    /// Send a single request, print the request and response, and exit
    #[arg(long)]
    pub debug: bool,

    /// Disable colors in the UI
    #[arg(long = "no-color", env = "NO_COLOR", value_parser = clap::builder::FalseyValueParser::new())]
    pub no_color: bool,

    /// UI frames per second
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u16).range(1..=60))]
    pub fps: u16,
}
