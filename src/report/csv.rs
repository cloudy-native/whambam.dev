// SPDX-License-Identifier: MIT
//! Per-request CSV rows, streamed as requests finish.

use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;

use super::output::open_output;
use crate::stats::RequestMetric;

pub const CSV_HEADER: [&str; 5] = [
    "offset_s",
    "latency_ms",
    "status",
    "error",
    "bytes_received",
];

pub struct CsvSink {
    writer: csv::Writer<Box<dyn Write + Send>>,
    error: Option<csv::Error>,
}

impl CsvSink {
    /// Write to `path`, or stdout when `None`.
    pub fn create(path: Option<&Path>) -> Result<Self> {
        let mut writer = csv::Writer::from_writer(open_output(path)?);
        writer.write_record(CSV_HEADER)?;
        Ok(CsvSink {
            writer,
            error: None,
        })
    }

    /// Append one row. The first write error is kept and returned by `finish`.
    pub fn write(&mut self, m: &RequestMetric) {
        if self.error.is_some() {
            return;
        }
        let (status, error) = match m.outcome.status {
            Ok(code) => (code.to_string(), String::new()),
            Err(kind) => (String::new(), kind.label().to_string()),
        };
        let row = [
            format!("{:.6}", m.start_offset.as_secs_f64()),
            format!("{:.3}", m.latency.as_secs_f64() * 1e3),
            status,
            error,
            m.outcome.bytes_received.to_string(),
        ];
        if let Err(e) = self.writer.write_record(&row) {
            self.error = Some(e);
        }
    }

    pub fn finish(mut self) -> Result<()> {
        if let Some(e) = self.error.take() {
            return Err(e.into());
        }
        self.writer.flush().map_err(io::Error::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{ErrorKind, Outcome};
    use std::time::Duration;

    #[test]
    fn rows_include_status_or_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.csv");
        let mut sink = CsvSink::create(Some(&path)).unwrap();
        let ok = RequestMetric {
            start_offset: Duration::from_millis(1500),
            latency: Duration::from_micros(2500),
            outcome: Outcome {
                status: Ok(200),
                bytes_received: 13,
            },
            bytes_sent: 0,
        };
        let err = RequestMetric {
            outcome: Outcome {
                status: Err(ErrorKind::Timeout),
                bytes_received: 0,
            },
            ..ok
        };
        sink.write(&ok);
        sink.write(&err);
        sink.finish().unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], CSV_HEADER.join(","));
        assert_eq!(lines[1], "1.500000,2.500,200,,13");
        assert_eq!(lines[2], "1.500000,2.500,,timeout,0");
    }
}
