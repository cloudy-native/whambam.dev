// SPDX-License-Identifier: MIT
//! Hands out request slots to workers and decides when each is due.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use tokio::time::{Interval, MissedTickBehavior};

use crate::config::{Pacing, TestConfig};

/// A stretch of the run at one global rate: slot `base_k` is due at `base_t`.
#[derive(Debug, Clone, Copy)]
struct Segment {
    base_k: usize,
    base_t: Instant,
    qps: f64,
}

impl Segment {
    fn due(&self, k: usize) -> Instant {
        let ahead = k.saturating_sub(self.base_k) as f64;
        self.base_t + Duration::from_secs_f64(ahead / self.qps)
    }
}

/// Shared by all workers: a request counter plus the pacing rules.
#[derive(Debug)]
pub struct Schedule {
    start: Instant,
    next: AtomicUsize,
    budget: Option<usize>,
    deadline: Option<Instant>,
    pacing: Pacing,
    segment: Mutex<Option<Segment>>,
    retimed: watch::Sender<()>,
}

impl Schedule {
    pub fn new(config: &TestConfig, start: Instant) -> Self {
        let segment = match config.pacing {
            Pacing::Global(qps) => Some(Segment {
                base_k: 0,
                base_t: start,
                qps,
            }),
            _ => None,
        };
        Schedule {
            start,
            next: AtomicUsize::new(0),
            budget: config.requests,
            deadline: config.duration.map(|d| start + d),
            pacing: config.pacing,
            segment: Mutex::new(segment),
            retimed: watch::Sender::new(()),
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

    /// With a global rate, slot `k` is due on a fixed timetable no matter how
    /// busy the workers are. That is what makes the latency correction work:
    /// a request that waits for a free worker is charged for the wait.
    pub fn due(&self, k: usize) -> Option<Instant> {
        self.segment.lock().ok()?.map(|s| s.due(k))
    }

    /// Change the global rate from now: every slot not yet due under the old
    /// rate is re-timed at the new rate, starting at `now`. Slots already past
    /// due (a backlog) become due immediately rather than carrying the debt.
    pub fn set_global_rate(&self, qps: f64, now: Instant) {
        if let Ok(mut guard) = self.segment.lock()
            && let Some(old) = *guard
        {
            let elapsed = now.saturating_duration_since(old.base_t).as_secs_f64();
            let base_k = if now < old.base_t {
                old.base_k
            } else {
                old.base_k + (elapsed * old.qps).floor() as usize + 1
            };
            *guard = Some(Segment {
                base_k,
                base_t: now,
                qps,
            });
            drop(guard);
            self.retimed.send_replace(());
        }
    }

    /// Notified after every re-timing, so sleeping workers re-read `due`.
    pub fn subscribe_retimed(&self) -> watch::Receiver<()> {
        self.retimed.subscribe()
    }

    /// True if a request due at `t` would start after the test deadline.
    pub fn past_deadline(&self, t: Instant) -> bool {
        self.deadline.is_some_and(|d| t >= d)
    }

    /// hey-style per-worker ticker for `-q` at the given rate.
    pub fn worker_interval(&self, rate: Option<f64>) -> Option<Interval> {
        match (self.pacing, rate) {
            (Pacing::PerWorker(_), Some(r)) if r > 0.0 => {
                let mut iv = tokio::time::interval(Duration::from_secs_f64(1.0 / r));
                iv.set_missed_tick_behavior(MissedTickBehavior::Skip);
                Some(iv)
            }
            _ => None,
        }
    }

    pub fn is_global(&self) -> bool {
        matches!(self.pacing, Pacing::Global(_))
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
        assert!(s.is_global());
        assert_eq!(s.due(0), Some(s.start()));
        assert_eq!(s.due(50).unwrap() - s.start(), Duration::from_millis(500));
        assert!(s.worker_interval(Some(100.0)).is_none());
    }

    #[test]
    fn changing_the_rate_retimes_slots_not_yet_due() {
        let s = schedule(&["http://x", "--qps", "10"]);
        let ms = |k| s.due(k).unwrap() - s.start();
        // At 250 ms, slots 0-2 are already due (0, 100, 200 ms); slot 3 is next.
        s.set_global_rate(100.0, s.start() + Duration::from_millis(250));
        assert_eq!(ms(3), Duration::from_millis(250));
        assert_eq!(ms(13), Duration::from_millis(350));
        // Slots from before the change that are still unsent fire right away.
        assert_eq!(ms(1), Duration::from_millis(250));
        // Changing again keeps counting from the current segment.
        s.set_global_rate(10.0, s.start() + Duration::from_millis(300));
        assert_eq!(ms(9), Duration::from_millis(300));
        assert_eq!(ms(10), Duration::from_millis(400));
    }

    #[test]
    fn rate_changes_are_ignored_without_global_pacing() {
        let s = schedule(&["http://x"]);
        s.set_global_rate(10.0, Instant::now());
        assert_eq!(s.due(3), None);
    }

    #[tokio::test]
    async fn per_worker_rate_uses_an_interval() {
        let s = schedule(&["http://x", "-q", "20"]);
        assert!(s.due(5).is_none());
        let iv = s.worker_interval(Some(20.0)).unwrap();
        assert_eq!(iv.period(), Duration::from_millis(50));
        assert!(s.worker_interval(None).is_none());
    }
}
