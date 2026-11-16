// SPDX-License-Identifier: MIT
//! A worker: claim a slot, wait until it is due, send, report.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::schedule::Schedule;
use crate::http::Sender;
use crate::stats::RequestMetric;

pub struct Worker {
    pub sender: Sender,
    pub schedule: Arc<Schedule>,
    pub stop: CancellationToken,
    pub metrics: mpsc::Sender<RequestMetric>,
    pub bytes_sent: u64,
}

impl Worker {
    pub async fn run(self) {
        let mut interval = self.schedule.worker_interval();
        while let Some(k) = self.schedule.claim() {
            if let Some(iv) = interval.as_mut() {
                tokio::select! {
                    _ = iv.tick() => {}
                    _ = self.stop.cancelled() => break,
                }
            }
            let due = self.schedule.due(k);
            if let Some(due) = due {
                if self.schedule.past_deadline(due) {
                    break;
                }
                tokio::select! {
                    _ = tokio::time::sleep_until(due.into()) => {}
                    _ = self.stop.cancelled() => break,
                }
            }
            if self.stop.is_cancelled() {
                break;
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
            if self.metrics.send(metric).await.is_err() {
                break;
            }
        }
    }
}
