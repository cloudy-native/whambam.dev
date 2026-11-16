// SPDX-License-Identifier: MIT
//! Hands out request slots to workers and decides when each is due.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::time::{Interval, MissedTickBehavior};

use crate::config::{Pacing, TestConfig};

/// Shared by all workers: a request counter plus the pacing rules.
#[derive(Debug)]
pub struct Schedule {
    start: Instant,
    next: AtomicUsize,
    budget: Option<usize>,
    deadline: Option<Instant>,
    pacing: Pacing,
}

impl Schedule {
    pub fn new(config: &TestConfig, start: Instant) -> Self {
        Schedule {
            start,
            next: AtomicUsize::new(0),
            budget: config.requests,
            deadline: config.duration.map(|d| start + d),
            pacing: config.pacing,
        }
    }

    pub fn start(&self) -> Instant {
        self.start
    }

    /// Claim the next request slot, or `None` when the budget is spent.
    pub fn claim(&self) -> Option<usize> {
        let k = self.next.fetch_add(1, Ordering::Relaxed);
        match self.budget {
            Some(b) if k >= b => None,
            _ => Some(k),
        }
    }

    /// With a global rate, slot `k` is due at `start + k / qps` no matter how
    /// busy the workers are. That is what makes the latency correction work:
    /// a request that waits for a free worker is charged for the wait.
    pub fn due(&self, k: usize) -> Option<Instant> {
        match self.pacing {
            Pacing::Global(qps) => Some(self.start + Duration::from_secs_f64(k as f64 / qps)),
            _ => None,
        }
    }

    /// True if a request due at `t` would start after the test deadline.
    pub fn past_deadline(&self, t: Instant) -> bool {
        self.deadline.is_some_and(|d| t >= d)
    }

    /// hey-style per-worker ticker, if `-q` is set.
    pub fn worker_interval(&self) -> Option<Interval> {
        match self.pacing {
            Pacing::PerWorker(rate) => {
                let mut iv = tokio::time::interval(Duration::from_secs_f64(1.0 / rate));
                iv.set_missed_tick_behavior(MissedTickBehavior::Skip);
                Some(iv)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    fn schedule(args: &[&str]) -> Schedule {
        let cli =
            Cli::try_parse_from(std::iter::once("whambam").chain(args.iter().copied())).unwrap();
        Schedule::new(&TestConfig::from_cli(&cli).unwrap(), Instant::now())
    }

    #[test]
    fn budget_limits_claims() {
        let s = schedule(&["http://x", "-n", "3"]);
        assert_eq!(
            (s.claim(), s.claim(), s.claim()),
            (Some(0), Some(1), Some(2))
        );
        assert_eq!(s.claim(), None);
        assert_eq!(s.claim(), None);
    }

    #[test]
    fn duration_runs_are_unbounded_with_deadline() {
        let s = schedule(&["http://x", "-z", "1s"]);
        for _ in 0..1000 {
            assert!(s.claim().is_some());
        }
        assert!(!s.past_deadline(s.start()));
        assert!(s.past_deadline(s.start() + Duration::from_secs(1)));
    }

    #[test]
    fn global_rate_spaces_due_times() {
        let s = schedule(&["http://x", "--qps", "100"]);
        assert_eq!(s.due(0), Some(s.start()));
        assert_eq!(s.due(50).unwrap() - s.start(), Duration::from_millis(500));
        assert!(s.worker_interval().is_none());
    }

    #[tokio::test]
    async fn per_worker_rate_uses_an_interval() {
        let s = schedule(&["http://x", "-q", "20"]);
        assert!(s.due(5).is_none());
        let iv = s.worker_interval().unwrap();
        assert_eq!(iv.period(), Duration::from_millis(50));
    }
}
