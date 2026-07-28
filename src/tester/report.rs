// whambam - accurate text report (hey-inspired layout, measurement-first)
//
// Copyright (c) 2025 Stephen Harrison

use std::io::{self, Write};

use super::types::TestState;

/// Write a text summary for a completed test.
///
/// Layout is inspired by hey for familiarity, but **accuracy takes priority**:
/// - Average is the true mean of recorded latencies (HDR histogram mean).
/// - Response time histogram uses real sample counts from the histogram.
/// - Latency percentiles come from the HDR histogram.
/// - Fabricated per-phase breakdowns (DNS, dial, etc.) are omitted — we do not measure them.
pub fn print_hey_format_report<W: Write>(w: &mut W, test_state: &TestState) -> io::Result<()> {
    let elapsed = if test_state.is_complete {
        if let Some(end) = test_state.end_time {
            end.duration_since(test_state.start_time).as_secs_f64()
        } else {
            test_state.start_time.elapsed().as_secs_f64()
        }
    } else {
        test_state.start_time.elapsed().as_secs_f64()
    };

    let total_requests = test_state.completed_requests;
    let overall_tps = if elapsed > 0.0 {
        total_requests as f64 / elapsed
    } else {
        0.0
    };

    let min_latency_ms = if test_state.min_latency == f64::MAX {
        0.0
    } else {
        test_state.min_latency
    };
    let max_latency_ms = test_state.max_latency;

    // Histogram stores microseconds (latency_ms * 1000)
    let mean_latency_secs = if test_state.latency_histogram.is_empty() {
        0.0
    } else {
        test_state.latency_histogram.mean() / 1_000_000.0
    };

    if !test_state.headers.is_empty() {
        writeln!(w, "\nRequest Headers:")?;
        for (name, value) in &test_state.headers {
            writeln!(w, "  {name}: {value}")?;
        }
    }

    writeln!(w, "\nSummary:")?;
    writeln!(w, "  Total:\t{elapsed:.4} secs")?;
    writeln!(w, "  Slowest:\t{:.4} secs", max_latency_ms / 1000.0)?;
    writeln!(w, "  Fastest:\t{:.4} secs", min_latency_ms / 1000.0)?;
    writeln!(w, "  Average:\t{mean_latency_secs:.4} secs")?;
    writeln!(w, "  Requests/sec:\t{overall_tps:.4}")?;
    writeln!(w)?;
    writeln!(
        w,
        "  Total data:\t{} bytes",
        test_state.total_bytes_received
    )?;
    let size_per = if total_requests > 0 {
        test_state.total_bytes_received / total_requests as u64
    } else {
        0
    };
    writeln!(w, "  Size/request:\t{size_per} bytes")?;

    writeln!(w, "\nResponse time histogram:")?;
    write_real_latency_histogram(w, test_state)?;

    writeln!(w, "\nLatency distribution:")?;
    let q = |p: f64| -> f64 {
        if test_state.latency_histogram.is_empty() {
            0.0
        } else {
            test_state.latency_histogram.value_at_quantile(p) as f64 / 1_000_000.0
        }
    };
    writeln!(w, "  10% in {:.4} secs", q(0.10))?;
    writeln!(w, "  25% in {:.4} secs", q(0.25))?;
    writeln!(w, "  50% in {:.4} secs", q(0.50))?;
    writeln!(w, "  75% in {:.4} secs", q(0.75))?;
    writeln!(w, "  90% in {:.4} secs", q(0.90))?;
    writeln!(w, "  95% in {:.4} secs", q(0.95))?;
    writeln!(w, "  99% in {:.4} secs", q(0.99))?;

    // No fake DNS/dialup/req-write breakdown — those phases are not instrumented.
    // Latency samples are end-to-end (request start through full body read).

    writeln!(w, "\nStatus code distribution:")?;
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort_unstable();
    for status in status_codes {
        let count = *test_state.status_counts.get(&status).unwrap_or(&0);
        writeln!(w, "  [{status}]\t{count} responses")?;
    }

    Ok(())
}

/// Linear histogram from real HDR sample counts (microsecond units → seconds labels).
fn write_real_latency_histogram<W: Write>(w: &mut W, test_state: &TestState) -> io::Result<()> {
    let hist = &test_state.latency_histogram;
    if hist.is_empty() {
        writeln!(w, "  (no samples)")?;
        return Ok(());
    }

    // Stored unit: microseconds
    let min_us = hist.min().max(1);
    let max_us = hist.max().max(min_us + 1);
    let num_buckets = 11u64; // similar density to hey
    let range = max_us.saturating_sub(min_us).max(1);
    let step = (range / num_buckets).max(1);

    let mut buckets: Vec<(f64, u64)> = Vec::new();
    for v in hist.iter_linear(step) {
        let count = v.count_since_last_iteration();
        // value_iterated_to is the upper edge of the linear step (microseconds)
        let edge_secs = v.value_iterated_to() as f64 / 1_000_000.0;
        buckets.push((edge_secs, count));
        // Cap bar count roughly like hey (~11 lines)
        if buckets.len() >= 12 {
            break;
        }
    }

    // If iter_linear produced only empty trailing buckets, fall back to recorded bins
    if buckets.iter().all(|&(_, c)| c == 0) {
        buckets.clear();
        for v in hist.iter_recorded() {
            let edge_secs = v.value_iterated_to() as f64 / 1_000_000.0;
            let count = v.count_since_last_iteration();
            buckets.push((edge_secs, count));
        }
    }

    let max_count = buckets.iter().map(|&(_, c)| c).max().unwrap_or(1).max(1) as f64;
    let bar_width = 40usize;
    for (edge_secs, count) in buckets {
        let bar_len = ((count as f64 / max_count) * bar_width as f64) as usize;
        let bar = "■".repeat(bar_len.min(bar_width));
        writeln!(w, "  {edge_secs:.3} [{count}]\t|{bar}")?;
    }

    Ok(())
}
