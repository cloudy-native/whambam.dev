// SPDX-License-Identifier: MIT
//! Write the final report in the chosen format.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};

use super::{Report, write_text};
use crate::cli::OutputFormat;

/// Open `path` for writing, or stdout when `None`.
pub fn open_output(path: Option<&Path>) -> Result<Box<dyn Write + Send>> {
    Ok(match path {
        Some(p) => {
            let file =
                File::create(p).with_context(|| format!("failed to create {}", p.display()))?;
            Box::new(BufWriter::new(file))
        }
        None => Box::new(BufWriter::new(io::stdout())),
    })
}

/// Text and JSON summaries. CSV is streamed by [`super::CsvSink`] instead.
pub fn write_report(format: OutputFormat, report: &Report, path: Option<&Path>) -> Result<()> {
    if matches!(format, OutputFormat::Csv | OutputFormat::Quiet) {
        return Ok(());
    }
    let mut out = open_output(path)?;
    match format {
        OutputFormat::Text => write_text(&mut out, report)?,
        _ => {
            serde_json::to_writer_pretty(&mut out, report)?;
            writeln!(out)?;
        }
    }
    out.flush()?;
    Ok(())
}
