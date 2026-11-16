// SPDX-License-Identifier: MIT
//! Runs a load test: spawns workers, collects metrics into shared state.

mod schedule;
mod worker;

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::TestConfig;
use crate::http::{Sender, build_clients};
use crate::report::CsvSink;
use crate::stats::{RequestMetric, SharedState};
use schedule::Schedule;
use worker::Worker;

/// Max metrics recorded per lock of the shared state.
const BATCH: usize = 512;

/// Run one test to completion (or until `stop` is cancelled).
///
/// `stop` is the user's quit signal; the test deadline uses a child token so
/// the final state records whether the run was interrupted.
pub async fn run(
    config: Arc<TestConfig>,
    state: SharedState,
    stop: CancellationToken,
    sink: Option<CsvSink>,
) -> Result<Option<CsvSink>> {
    let clients = build_clients(&config)?;
    state.reset();
    let schedule = Arc::new(Schedule::new(&config, state.lock().start));
    let run_token = stop.child_token();
    if let Some(duration) = config.duration {
        let token = run_token.clone();
        let deadline = schedule.start() + duration;
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline.into()) => token.cancel(),
                _ = token.cancelled() => {}
            }
        });
    }

    let (tx, rx) = mpsc::channel(config.workers().saturating_mul(16).clamp(1024, 65_536));
    let consumer = tokio::spawn(consume(rx, state.clone(), sink));

    let bytes_sent = config.request.approx_size();
    let senders: Vec<Sender> = clients
        .into_iter()
        .map(|c| Sender::new(c, &config))
        .collect();
    let workers: Vec<_> = (0..config.workers())
        .map(|i| {
            let worker = Worker {
                sender: senders[(i / config.http2_parallel) % senders.len()].clone(),
                schedule: Arc::clone(&schedule),
                stop: run_token.clone(),
                metrics: tx.clone(),
                bytes_sent,
            };
            tokio::spawn(worker.run())
        })
        .collect();
    drop(tx);
    futures::future::join_all(workers).await;
    run_token.cancel();
    let sink = consumer.await?;
    state.lock().mark_complete(stop.is_cancelled());
    Ok(sink)
}

async fn consume(
    mut rx: mpsc::Receiver<RequestMetric>,
    state: SharedState,
    mut sink: Option<CsvSink>,
) -> Option<CsvSink> {
    let mut batch = Vec::with_capacity(BATCH);
    while rx.recv_many(&mut batch, BATCH).await > 0 {
        {
            let mut guard = state.lock();
            for m in &batch {
                guard.record(m);
            }
        }
        if let Some(s) = sink.as_mut() {
            batch.iter().for_each(|m| s.write(m));
        }
        batch.clear();
    }
    sink
}

#[cfg(test)]
mod tests;
