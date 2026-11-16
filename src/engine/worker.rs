// SPDX-License-Identifier: MIT
//! A worker: claim a slot, wait until it is due, send, report.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{mpsc, watch};
use tokio::time::Interval;
use tokio_util::sync::CancellationToken;

use super::control::Settings;
use super::schedule::Schedule;
use crate::http::Sender;
use crate::stats::RequestMetric;

pub struct Worker {
    /// Workers at or above the live worker count wait until it grows.
    pub index: usize,
    pub sender: Sender,
    pub schedule: Arc<Schedule>,
    pub settings: watch::Receiver<Settings>,
    /// Fires after the schedule is re-timed for a new global rate.
    pub retimed: watch::Receiver<()>,
    /// Deadline or user stop: no new requests.
    pub stop: CancellationToken,
    /// Cancelled once the request budget is spent, so parked workers exit.
    pub exhausted: CancellationToken,
    pub metrics: mpsc::Sender<RequestMetric>,
    pub bytes_sent: u64,
}

impl Worker {
    pub async fn run(mut self) {
        let mut rate = self.settings.borrow().rate;
        let mut interval = self.schedule.worker_interval(rate);
        while self.wait_until_active().await {
            let current = self.settings.borrow().rate;
            if current != rate {
                rate = current;
                interval = self.schedule.worker_interval(rate);
            }
            let Some(k) = self.schedule.claim() else {
                self.exhausted.cancel();
                break;
            };
            if !self.send_when_due(k, interval.as_mut()).await {
                break;
            }
        }
    }

    /// Park while this worker is above the live worker count. Returns false
    /// when the run is over.
    async fn wait_until_active(&mut self) -> bool {
        loop {
            if self.stop.is_cancelled() || self.exhausted.is_cancelled() {
                return false;
            }
            if self.index < self.settings.borrow_and_update().workers {
                return true;
            }
            tokio::select! {
                changed = self.settings.changed() => if changed.is_err() { return false },
                _ = self.stop.cancelled() => return false,
                _ = self.exhausted.cancelled() => return false,
            }
        }
    }

    /// Sleep until slot `k` is due under a global rate, re-checking when the
    /// schedule is re-timed. `Some(None)` means no global rate; `None` means stop.
    async fn wait_for_due(&mut self, k: usize) -> Option<Option<Instant>> {
        loop {
            let Some(due) = self.schedule.due(k) else {
                return Some(None);
            };
            if self.schedule.past_deadline(due) {
                return None;
            }
            tokio::select! {
                _ = tokio::time::sleep_until(due.into()) => return Some(Some(due)),
                changed = self.retimed.changed() => changed.ok()?,
                _ = self.stop.cancelled() => return None,
            }
        }
    }

    /// Wait for slot `k` to be due, send it and report. Returns false to stop.
    async fn send_when_due(&mut self, k: usize, interval: Option<&mut Interval>) -> bool {
        if let Some(iv) = interval {
            tokio::select! {
                _ = iv.tick() => {}
                _ = self.stop.cancelled() => return false,
            }
        }
        let Some(due) = self.wait_for_due(k).await else {
            return false;
        };
        if self.stop.is_cancelled() {
            return false;
        }
        let sent = Instant::now();
        let (outcome, latency) = self.sender.send().await;
        // Charge the time spent waiting for a free worker (coordinated omission).
        let begun = due.unwrap_or(sent).min(sent);
        let metric = RequestMetric {
            start_offset: begun.saturating_duration_since(self.schedule.start()),
            latency: latency + sent.duration_since(begun),
            outcome,
            bytes_sent: self.bytes_sent,
        };
        self.metrics.send(metric).await.is_ok()
    }
}
