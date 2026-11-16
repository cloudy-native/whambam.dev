// SPDX-License-Identifier: MIT
//! Fixed-interval time series of throughput and mean latency for charts.

use std::collections::VecDeque;

/// Sample period in seconds.
pub const SAMPLE_INTERVAL: f64 = 0.1;
/// Samples kept for charting (one minute at 100 ms).
const HISTORY: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Seconds since the test started (end of the sample window).
    pub t: f64,
    pub rps: f64,
    /// Mean latency (ms) of requests that finished in this window.
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Series {
    samples: VecDeque<Sample>,
    window_start: f64,
    count: u64,
    latency_sum_ms: f64,
}

impl Series {
    pub fn clear(&mut self) {
        *self = Series::default();
    }

    /// Record a finished request at `t` seconds.
    pub fn record(&mut self, t: f64, latency_ms: f64) {
        self.advance(t);
        self.count += 1;
        self.latency_sum_ms += latency_ms;
    }

    /// Close every full window that ended before `t` (call regularly so the
    /// chart keeps moving even when no requests complete).
    pub fn advance(&mut self, t: f64) {
        while t - self.window_start >= SAMPLE_INTERVAL {
            let end = self.window_start + SAMPLE_INTERVAL;
            self.push(end, SAMPLE_INTERVAL);
            self.window_start = end;
        }
    }

    /// Flush the partial window at the end of a run.
    pub fn finish(&mut self, t: f64) {
        self.advance(t);
        let dt = t - self.window_start;
        if self.count > 0 && dt > 0.0 {
            self.push(t, dt);
            self.window_start = t;
        }
    }

    fn push(&mut self, t: f64, dt: f64) {
        let latency_ms = if self.count > 0 {
            self.latency_sum_ms / self.count as f64
        } else {
            self.samples.back().map_or(0.0, |s| s.latency_ms)
        };
        self.samples.push_back(Sample {
            t,
            rps: self.count as f64 / dt,
            latency_ms,
        });
        if self.samples.len() > HISTORY {
            self.samples.pop_front();
        }
        self.count = 0;
        self.latency_sum_ms = 0.0;
    }

    pub fn samples(&self) -> &VecDeque<Sample> {
        &self.samples
    }

    pub fn last(&self) -> Option<Sample> {
        self.samples.back().copied()
    }

    /// (t, value) points for a chart.
    pub fn points(&self, value: impl Fn(&Sample) -> f64) -> Vec<(f64, f64)> {
        self.samples.iter().map(|s| (s.t, value(s))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_close_at_fixed_intervals() {
        let mut s = Series::default();
        for i in 0..10 {
            s.record(0.01 * i as f64, 2.0);
        }
        assert!(s.samples().is_empty());
        s.advance(0.1);
        let first = s.last().unwrap();
        assert!((first.rps - 100.0).abs() < 1e-9);
        assert_eq!(first.latency_ms, 2.0);
    }

    #[test]
    fn idle_windows_report_zero_rps_and_keep_latency() {
        let mut s = Series::default();
        s.record(0.05, 4.0);
        s.advance(0.35);
        assert_eq!(s.samples().len(), 3);
        let last = s.last().unwrap();
        assert_eq!(last.rps, 0.0);
        assert_eq!(last.latency_ms, 4.0);
    }

    #[test]
    fn finish_flushes_partial_window_and_history_is_bounded() {
        let mut s = Series::default();
        s.record(0.02, 1.0);
        s.finish(0.05);
        assert_eq!(s.samples().len(), 1);
        assert!((s.last().unwrap().rps - 20.0).abs() < 1e-9);

        let mut long = Series::default();
        long.advance(HISTORY as f64 * SAMPLE_INTERVAL * 2.0);
        assert_eq!(long.samples().len(), HISTORY);
        assert_eq!(long.points(|p| p.rps).len(), HISTORY);
    }
}
