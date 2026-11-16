// SPDX-License-Identifier: MIT
//! Live statistics shared by the engine, the UI and the reports.

mod series;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use hdrhistogram::Histogram;

pub use series::{SAMPLE_INTERVAL, Sample, Series};

use crate::http::{ErrorKind, Outcome};

/// Highest latency the histogram tracks precisely (one hour, in µs).
const MAX_TRACKED_US: u64 = 3_600_000_000;

/// One finished request.
#[derive(Debug, Clone, Copy)]
pub struct RequestMetric {
    /// When the request was due (or sent), relative to the test start.
    pub start_offset: Duration,
    pub latency: Duration,
    pub outcome: Outcome,
    pub bytes_sent: u64,
}

impl RequestMetric {
    /// 1xx–3xx responses count as successes; 4xx/5xx and transport errors do not.
    pub fn is_success(&self) -> bool {
        matches!(self.outcome.status, Ok(code) if code < 400)
    }
}

#[derive(Debug, Clone)]
pub struct TestState {
    pub start: Instant,
    pub end: Option<Instant>,
    pub completed: usize,
    pub successes: usize,
    pub status_counts: BTreeMap<u16, usize>,
    pub error_counts: BTreeMap<ErrorKind, usize>,
    /// End-to-end latency in microseconds.
    pub histogram: Histogram<u64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub series: Series,
    pub interrupted: bool,
}

impl Default for TestState {
    fn default() -> Self {
        TestState {
            start: Instant::now(),
            end: None,
            completed: 0,
            successes: 0,
            status_counts: BTreeMap::new(),
            error_counts: BTreeMap::new(),
            histogram: Histogram::new_with_bounds(1, MAX_TRACKED_US, 3).expect("valid bounds"),
            bytes_sent: 0,
            bytes_received: 0,
            series: Series::default(),
            interrupted: false,
        }
    }
}

impl TestState {
    pub fn record(&mut self, m: &RequestMetric) {
        self.completed += 1;
        if m.is_success() {
            self.successes += 1;
        }
        match m.outcome.status {
            Ok(code) => *self.status_counts.entry(code).or_default() += 1,
            Err(kind) => *self.error_counts.entry(kind).or_default() += 1,
        }
        self.bytes_sent += m.bytes_sent;
        self.bytes_received += m.outcome.bytes_received;
        let micros = (m.latency.as_micros() as u64).clamp(1, MAX_TRACKED_US);
        self.histogram.saturating_record(micros);
        let now = self.start.elapsed().as_secs_f64();
        self.series.record(now, m.latency.as_secs_f64() * 1e3);
    }

    /// Advance the chart series to now; call from the UI tick.
    pub fn tick(&mut self) {
        if self.end.is_none() {
            let now = self.start.elapsed().as_secs_f64();
            self.series.advance(now);
        }
    }

    /// Freeze the clock. Idempotent.
    pub fn mark_complete(&mut self, interrupted: bool) {
        if self.end.is_none() {
            let end = Instant::now();
            self.end = Some(end);
            self.interrupted = interrupted;
            self.series
                .finish(end.duration_since(self.start).as_secs_f64());
        }
    }

    pub fn is_complete(&self) -> bool {
        self.end.is_some()
    }

    pub fn elapsed(&self) -> Duration {
        self.end
            .unwrap_or_else(Instant::now)
            .duration_since(self.start)
    }

    pub fn rps(&self) -> f64 {
        let secs = self.elapsed().as_secs_f64();
        if secs > 0.0 {
            self.completed as f64 / secs
        } else {
            0.0
        }
    }

    pub fn failures(&self) -> usize {
        self.completed - self.successes
    }

    pub fn transport_errors(&self) -> usize {
        self.error_counts.values().sum()
    }

    pub fn success_rate(&self) -> f64 {
        if self.completed == 0 {
            100.0
        } else {
            100.0 * self.successes as f64 / self.completed as f64
        }
    }

    /// Latency at quantile `q` (0..=1) in milliseconds.
    pub fn percentile_ms(&self, q: f64) -> f64 {
        self.hist_ms(|h| h.value_at_quantile(q) as f64)
    }

    pub fn min_ms(&self) -> f64 {
        self.hist_ms(|h| h.min() as f64)
    }

    pub fn max_ms(&self) -> f64 {
        self.hist_ms(|h| h.max() as f64)
    }

    pub fn mean_ms(&self) -> f64 {
        self.hist_ms(Histogram::mean)
    }

    pub fn stdev_ms(&self) -> f64 {
        self.hist_ms(Histogram::stdev)
    }

    fn hist_ms(&self, f: impl Fn(&Histogram<u64>) -> f64) -> f64 {
        if self.histogram.is_empty() {
            0.0
        } else {
            f(&self.histogram) / 1e3
        }
    }
}

/// Thread-safe handle to the live state.
#[derive(Clone, Default)]
pub struct SharedState(Arc<Mutex<TestState>>);

impl SharedState {
    /// Lock the state, recovering from a poisoned lock (stats stay usable).
    pub fn lock(&self) -> MutexGuard<'_, TestState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> TestState {
        self.lock().clone()
    }

    pub fn reset(&self) {
        *self.lock() = TestState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(status: Result<u16, ErrorKind>, ms: u64) -> RequestMetric {
        RequestMetric {
            start_offset: Duration::ZERO,
            latency: Duration::from_millis(ms),
            outcome: Outcome {
                status,
                bytes_received: 10,
            },
            bytes_sent: 5,
        }
    }

    #[test]
    fn records_statuses_errors_and_bytes() {
        let mut s = TestState::default();
        s.record(&metric(Ok(200), 10));
        s.record(&metric(Ok(302), 20));
        s.record(&metric(Ok(503), 30));
        s.record(&metric(Err(ErrorKind::Timeout), 40));
        assert_eq!(s.completed, 4);
        assert_eq!(s.successes, 2);
        assert_eq!(s.failures(), 2);
        assert_eq!(s.transport_errors(), 1);
        assert_eq!(s.status_counts[&503], 1);
        assert_eq!(s.error_counts[&ErrorKind::Timeout], 1);
        assert_eq!(s.bytes_received, 40);
        assert_eq!(s.bytes_sent, 20);
        assert_eq!(s.success_rate(), 50.0);
    }

    #[test]
    fn latency_statistics_come_from_the_histogram() {
        let mut s = TestState::default();
        assert_eq!(s.percentile_ms(0.5), 0.0);
        for ms in 1..=100 {
            s.record(&metric(Ok(200), ms));
        }
        assert!((s.mean_ms() - 50.5).abs() < 0.1);
        assert!((s.min_ms() - 1.0).abs() < 0.01);
        assert!((s.max_ms() - 100.0).abs() < 0.1);
        assert!((s.percentile_ms(0.99) - 99.0).abs() < 0.1);
        assert!(s.stdev_ms() > 28.0);
    }

    #[test]
    fn completion_freezes_elapsed_and_is_idempotent() {
        let shared = SharedState::default();
        shared.lock().record(&metric(Ok(200), 1));
        shared.lock().mark_complete(true);
        let first = shared.lock().elapsed();
        shared.lock().mark_complete(false);
        let snap = shared.snapshot();
        assert!(snap.is_complete() && snap.interrupted);
        assert_eq!(snap.elapsed(), first);
        assert!(snap.rps() > 0.0);
        shared.reset();
        assert_eq!(shared.lock().completed, 0);
    }
}
