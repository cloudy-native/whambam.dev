// SPDX-License-Identifier: MIT
//! hey-style text summary. Accuracy wins over matching hey's numbers: there is
//! no fake per-phase (DNS/dial/write) breakdown because it is not measured.

use std::io::{self, Write};

use super::{AssertResult, ComparisonRow, Report};

const BAR_WIDTH: usize = 40;

pub fn write_text<W: Write>(w: &mut W, r: &Report) -> io::Result<()> {
    let s = &r.summary;
    let l = &r.latency_ms;
    writeln!(w, "\nSummary:")?;
    writeln!(w, "  Total:\t{:.4} secs", s.total_secs)?;
    writeln!(w, "  Slowest:\t{:.4} secs", l.max / 1e3)?;
    writeln!(w, "  Fastest:\t{:.4} secs", l.min / 1e3)?;
    writeln!(w, "  Average:\t{:.4} secs", l.mean / 1e3)?;
    writeln!(w, "  Requests/sec:\t{:.4}", s.rps)?;
    writeln!(w, "  Success rate:\t{:.2}%", s.success_rate)?;
    writeln!(w)?;
    writeln!(w, "  Total data:\t{} bytes", s.bytes_received)?;
    writeln!(w, "  Size/request:\t{} bytes", s.size_per_request)?;

    writeln!(w, "\nResponse time histogram:")?;
    write_histogram(w, r)?;

    writeln!(w, "\nLatency distribution:")?;
    for (label, ms) in &r.latency_ms.percentiles {
        let pct = label.trim_start_matches('p');
        writeln!(w, "  {pct}% in {:.4} secs", ms / 1e3)?;
    }

    writeln!(w, "\nStatus code distribution:")?;
    for (code, count) in &r.status_codes {
        writeln!(w, "  [{code}]\t{count} responses")?;
    }

    if !r.errors.is_empty() {
        writeln!(w, "\nError distribution:")?;
        for (kind, count) in &r.errors {
            writeln!(w, "  [{count}]\t{}", kind.label())?;
        }
    }

    if !r.adjustments.is_empty() {
        writeln!(w, "\nLoad changes:")?;
        for a in &r.adjustments {
            let rate = a
                .rate
                .map(|q| format!(", {q:.1} req/s"))
                .unwrap_or_default();
            writeln!(w, "  at {:.1}s: {} workers{rate}", a.t, a.workers)?;
        }
    }
    if let Some(rows) = &r.comparison {
        write_comparison(w, rows)?;
    }
    if let Some(results) = &r.assertions {
        write_assertions(w, results)?;
    }
    write_notes(w, r)
}

/// "Compared with baseline" table.
pub fn write_comparison<W: Write>(w: &mut W, rows: &[ComparisonRow]) -> io::Result<()> {
    writeln!(w, "\nCompared with baseline:")?;
    writeln!(
        w,
        "  {:<8} {:>14} {:>14} {:>9}",
        "metric", "baseline", "current", "change"
    )?;
    for row in rows {
        let change = match row.change_pct {
            Some(p) => format!("{p:+.1}%"),
            None => "n/a".to_string(),
        };
        let mark = if row.improved { " " } else { " ▲" };
        writeln!(
            w,
            "  {:<8} {:>14} {:>14} {:>9}{mark}",
            row.metric.name(),
            row.metric.format(row.baseline),
            row.metric.format(row.current),
            change
        )?;
    }
    writeln!(w, "  (▲ = worse than baseline)")
}

/// One line per assertion, ✓ or ✗.
pub fn write_assertions<W: Write>(w: &mut W, results: &[AssertResult]) -> io::Result<()> {
    writeln!(w, "\nAssertions:")?;
    for r in results {
        let mark = if r.passed { "✓" } else { "✗" };
        writeln!(
            w,
            "  {mark} {}  (actual {}, limit {})",
            r.assertion,
            r.metric.format(r.actual),
            r.metric.format(r.threshold)
        )?;
    }
    Ok(())
}

fn write_histogram<W: Write>(w: &mut W, r: &Report) -> io::Result<()> {
    if r.histogram.is_empty() {
        return writeln!(w, "  (no samples)");
    }
    let max = r
        .histogram
        .iter()
        .map(|b| b.count)
        .max()
        .unwrap_or(1)
        .max(1);
    for b in &r.histogram {
        let len = (b.count as usize * BAR_WIDTH).div_ceil(max as usize);
        writeln!(
            w,
            "  {:.3} [{}]\t|{}",
            b.le_ms / 1e3,
            b.count,
            "■".repeat(len)
        )?;
    }
    Ok(())
}

fn write_notes<W: Write>(w: &mut W, r: &Report) -> io::Result<()> {
    if r.load.latency_corrected {
        writeln!(
            w,
            "\nLatencies are corrected for coordinated omission: measured from when each request was due."
        )?;
    }
    if r.summary.interrupted {
        writeln!(w, "\nRun was interrupted before it finished.")?;
    }
    Ok(())
}
