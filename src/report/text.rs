// SPDX-License-Identifier: MIT
//! hey-style text summary. Accuracy wins over matching hey's numbers: there is
//! no fake per-phase (DNS/dial/write) breakdown because it is not measured.

use std::io::{self, Write};

use super::Report;

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

    write_notes(w, r)
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
