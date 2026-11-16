// SPDX-License-Identifier: MIT
//! Live load settings the UI can change while a test runs.

use tokio::sync::watch;

use crate::config::{Pacing, TestConfig};

/// Upper bound for live worker changes.
pub const MAX_WORKERS: usize = 10_000;
/// Each key press scales workers or rate by this fraction (at least one step).
const STEP: f64 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub workers: usize,
    /// Requests per second: total for `--qps`, per worker for `-q`, `None` when unthrottled.
    pub rate: Option<f64>,
}

impl Settings {
    pub fn from_config(config: &TestConfig) -> Self {
        let rate = match config.pacing {
            Pacing::Unlimited => None,
            Pacing::PerWorker(r) | Pacing::Global(r) => Some(r),
        };
        Settings {
            workers: config.workers(),
            rate,
        }
    }
}

/// Shared handle: the UI writes, the engine and workers watch.
#[derive(Debug, Clone)]
pub struct Control {
    tx: watch::Sender<Settings>,
}

impl Control {
    pub fn new(settings: Settings) -> Self {
        Control {
            tx: watch::Sender::new(settings),
        }
    }

    pub fn get(&self) -> Settings {
        *self.tx.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<Settings> {
        self.tx.subscribe()
    }

    /// Reset to the configured values (used when a test restarts).
    pub fn reset(&self, settings: Settings) {
        self.tx.send_replace(settings);
    }

    /// Scale the worker count up (`+1`) or down (`-1`) by one step.
    pub fn step_workers(&self, direction: i32) {
        self.tx.send_if_modified(|s| {
            let next = step(s.workers as f64, direction).round() as usize;
            let next = next.clamp(1, MAX_WORKERS);
            let changed = next != s.workers;
            s.workers = next;
            changed
        });
    }

    /// Scale the request rate by one step. No effect on unthrottled runs.
    pub fn step_rate(&self, direction: i32) {
        self.tx.send_if_modified(|s| match s.rate {
            Some(r) => {
                let next = step(r, direction).max(0.1);
                s.rate = Some(next);
                (next - r).abs() > f64::EPSILON
            }
            None => false,
        });
    }
}

fn step(value: f64, direction: i32) -> f64 {
    let delta = (value * STEP).max(1.0);
    if direction >= 0 {
        value + delta
    } else {
        value - delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(workers: usize, rate: Option<f64>) -> Control {
        Control::new(Settings { workers, rate })
    }

    #[test]
    fn workers_step_by_ten_percent_with_a_minimum_of_one() {
        let c = control(50, None);
        c.step_workers(1);
        assert_eq!(c.get().workers, 55);
        let small = control(2, None);
        small.step_workers(-1);
        small.step_workers(-1);
        assert_eq!(small.get().workers, 1);
        let big = control(MAX_WORKERS, None);
        big.step_workers(1);
        assert_eq!(big.get().workers, MAX_WORKERS);
    }

    #[test]
    fn rate_steps_only_when_throttled() {
        let c = control(1, Some(100.0));
        let rx = c.subscribe();
        c.step_rate(-1);
        assert_eq!(c.get().rate, Some(90.0));
        assert!(rx.has_changed().unwrap());

        let open = control(1, None);
        let mut rx = open.subscribe();
        open.step_rate(1);
        assert_eq!(open.get().rate, None);
        assert!(!rx.has_changed().unwrap());
        rx.mark_unchanged();
    }

    #[test]
    fn reset_restores_settings() {
        let c = control(4, Some(10.0));
        c.step_workers(1);
        c.reset(Settings {
            workers: 4,
            rate: Some(10.0),
        });
        assert_eq!(
            c.get(),
            Settings {
                workers: 4,
                rate: Some(10.0)
            }
        );
    }
}
