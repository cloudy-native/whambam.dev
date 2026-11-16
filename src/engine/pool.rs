// SPDX-License-Identifier: MIT
//! Supervises workers and applies live setting changes.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::control::{Control, Settings};
use super::schedule::Schedule;
use super::worker::Worker;
use crate::http::Sender;
use crate::stats::{RequestMetric, SharedState};

pub struct Pool {
    pub senders: Vec<Sender>,
    pub http2_parallel: usize,
    pub schedule: Arc<Schedule>,
    pub control: Control,
    pub state: SharedState,
    pub stop: CancellationToken,
    pub metrics: mpsc::Sender<RequestMetric>,
    pub bytes_sent: u64,
}

impl Pool {
    /// Run until every worker has finished, growing the pool when the live
    /// worker count goes up. Workers above a lowered count park themselves.
    pub async fn run(self) {
        let exhausted = CancellationToken::new();
        let mut set = JoinSet::new();
        let mut spawned = 0;
        let mut rx = self.control.subscribe();
        let mut last = *rx.borrow_and_update();
        self.spawn_up_to(&mut set, &mut spawned, last.workers, &exhausted);
        let mut watching = true;
        loop {
            tokio::select! {
                joined = set.join_next() => if joined.is_none() { break },
                changed = rx.changed(), if watching => match changed {
                    Ok(()) => {
                        let next = *rx.borrow_and_update();
                        self.apply(last, next);
                        self.spawn_up_to(&mut set, &mut spawned, next.workers, &exhausted);
                        last = next;
                    }
                    Err(_) => watching = false,
                },
            }
        }
    }

    fn apply(&self, old: Settings, new: Settings) {
        if new.rate != old.rate
            && self.schedule.is_global()
            && let Some(rate) = new.rate
        {
            self.schedule.set_global_rate(rate, Instant::now());
        }
        self.state.lock().record_adjustment(new.workers, new.rate);
    }

    fn spawn_up_to(
        &self,
        set: &mut JoinSet<()>,
        spawned: &mut usize,
        target: usize,
        exhausted: &CancellationToken,
    ) {
        while *spawned < target {
            let index = *spawned;
            let worker = Worker {
                index,
                // HTTP/2: `parallel` workers share each connection; extra
                // workers added live reuse the existing connections.
                sender: self.senders[(index / self.http2_parallel) % self.senders.len()].clone(),
                schedule: Arc::clone(&self.schedule),
                settings: self.control.subscribe(),
                retimed: self.schedule.subscribe_retimed(),
                stop: self.stop.clone(),
                exhausted: exhausted.clone(),
                metrics: self.metrics.clone(),
                bytes_sent: self.bytes_sent,
            };
            set.spawn(worker.run());
            *spawned += 1;
        }
    }
}
