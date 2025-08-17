// MIT License
//
// Copyright (c) 2025 Whambam Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crossbeam_queue::SegQueue;
use hdrhistogram::Histogram;
use parking_lot::RwLock as PLRwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;

use super::types::RequestMetric;

/// A thread-safe metrics collector that uses lock-free data structures
/// to minimize contention when collecting metrics from multiple threads
pub struct LockFreeMetrics {
    // Configuration
    #[allow(dead_code)]
    url: String,
    #[allow(dead_code)]
    method: String,
    #[allow(dead_code)]
    start_time: Instant,

    // Atomic counters for frequently updated simple metrics
    completed_requests: AtomicUsize,
    error_count: AtomicUsize,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,

    // Concurrent queue for incoming metrics
    metrics_queue: Arc<SegQueue<RequestMetric>>,

    // These are updated less frequently and can use a lightweight RwLock
    // We use parking_lot's RwLock for better performance
    status_counts: Arc<PLRwLock<HashMap<u16, usize>>>,

    // Histogram for latency calculations
    // HDRHistogram is already thread-safe for recording values
    latency_histogram: Arc<RwLock<Histogram<u64>>>,

    // Derived statistics that are calculated periodically
    min_latency: AtomicU64,
    max_latency: AtomicU64,
    p50_latency: AtomicU64,
    p90_latency: AtomicU64,
    p95_latency: AtomicU64,
    p99_latency: AtomicU64,

    // Test completion flag
    is_complete: AtomicBool,
    end_time: RwLock<Option<Instant>>,

    // Last update time for periodic calculations
    last_stats_update: RwLock<Instant>,
}

#[allow(dead_code)]
impl LockFreeMetrics {
    /// Create a new lock-free metrics collector
    pub fn new(url: String, method: String) -> Self {
        let histogram = Histogram::<u64>::new(5).unwrap();

        LockFreeMetrics {
            url,
            method,
            start_time: Instant::now(),

            completed_requests: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),

            metrics_queue: Arc::new(SegQueue::new()),

            status_counts: Arc::new(PLRwLock::new(HashMap::new())),

            latency_histogram: Arc::new(RwLock::new(histogram)),

            min_latency: AtomicU64::new(u64::MAX),
            max_latency: AtomicU64::new(0),
            p50_latency: AtomicU64::new(0),
            p90_latency: AtomicU64::new(0),
            p95_latency: AtomicU64::new(0),
            p99_latency: AtomicU64::new(0),

            is_complete: AtomicBool::new(false),
            end_time: RwLock::new(None),

            last_stats_update: RwLock::new(Instant::now()),
        }
    }

    /// Record a metric without blocking
    /// This is the main entry point for recording metrics
    pub fn record(&self, metric: &RequestMetric) {
        // Increment atomic counters immediately
        self.completed_requests.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent
            .fetch_add(metric.bytes_sent, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(metric.bytes_received, Ordering::Relaxed);

        // Update error count if needed
        if metric.is_error {
            self.error_count.fetch_add(1, Ordering::Relaxed);
        }

        // Update min/max latency using compare_exchange
        let latency_as_u64 = (metric.latency_ms * 1000.0) as u64;

        // Update min latency with compare_exchange loop
        let mut current_min = self.min_latency.load(Ordering::Relaxed);
        while latency_as_u64 < current_min {
            match self.min_latency.compare_exchange(
                current_min,
                latency_as_u64,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        // Update max latency with compare_exchange loop
        let mut current_max = self.max_latency.load(Ordering::Relaxed);
        while latency_as_u64 > current_max {
            match self.max_latency.compare_exchange(
                current_max,
                latency_as_u64,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }

        // Queue the metric for batch processing
        self.metrics_queue.push(metric.clone());

        // Periodically process the queued metrics
        // We don't want to do this on every record call, so use a simple heuristic
        let completed = self.completed_requests.load(Ordering::Relaxed);
        if completed % 100 == 0 {
            self.process_queued_metrics();
            self.update_statistics();
        }
    }

    /// Process all queued metrics in batch
    pub fn process_queued_metrics(&self) {
        // Process status counts in batches
        let mut status_updates: HashMap<u16, usize> = HashMap::new();

        // Drain the queue, processing each metric
        while let Some(metric) = self.metrics_queue.pop() {
            // Update status counts locally
            if metric.status_code > 0 {
                *status_updates.entry(metric.status_code).or_insert(0) += 1;
            }

            // Add to histogram
            let latency_as_u64 = (metric.latency_ms * 1000.0) as u64;
            if let Ok(mut hist) = self.latency_histogram.write() {
                let _ = hist.record(latency_as_u64);
            }
        }

        // Now update the shared status counts with a single write lock
        if !status_updates.is_empty() {
            let mut counts = self.status_counts.write();
            for (code, count) in status_updates {
                *counts.entry(code).or_insert(0) += count;
            }
        }
    }

    /// Update derived statistics
    pub fn update_statistics(&self) {
        // Check if it's time to update statistics
        let elapsed = match self.last_stats_update.write() {
            Ok(guard) => guard.elapsed().as_millis(),
            Err(_) => return, // If we can't get the lock, just skip this update
        };

        // Only update every 500ms to avoid contention
        if elapsed < 500 {
            return;
        }

        // Update the timestamp
        if let Ok(mut last_update) = self.last_stats_update.write() {
            *last_update = Instant::now();
        }

        // Update percentiles
        if let Ok(hist) = self.latency_histogram.read() {
            // Convert from microseconds to milliseconds
            let p50 = hist.value_at_quantile(0.5);
            let p90 = hist.value_at_quantile(0.9);
            let p95 = hist.value_at_quantile(0.95);
            let p99 = hist.value_at_quantile(0.99);

            self.p50_latency.store(p50, Ordering::Relaxed);
            self.p90_latency.store(p90, Ordering::Relaxed);
            self.p95_latency.store(p95, Ordering::Relaxed);
            self.p99_latency.store(p99, Ordering::Relaxed);
        }
    }

    /// Mark the test as complete
    pub fn mark_complete(&self) {
        self.is_complete.store(true, Ordering::SeqCst);
        if let Ok(mut end_time) = self.end_time.write() {
            *end_time = Some(Instant::now());
        }
    }

    /// Get the total number of completed requests
    pub fn completed_requests(&self) -> usize {
        self.completed_requests.load(Ordering::Relaxed)
    }

    /// Get the total number of errors
    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Get the total bytes sent
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    /// Get the total bytes received
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Get the minimum latency in milliseconds
    pub fn min_latency(&self) -> f64 {
        let min = self.min_latency.load(Ordering::Relaxed);
        if min == u64::MAX {
            0.0
        } else {
            min as f64 / 1000.0
        }
    }

    /// Get the maximum latency in milliseconds
    pub fn max_latency(&self) -> f64 {
        self.max_latency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the P50 latency in milliseconds
    pub fn p50_latency(&self) -> f64 {
        self.p50_latency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the P90 latency in milliseconds
    pub fn p90_latency(&self) -> f64 {
        self.p90_latency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the P95 latency in milliseconds
    pub fn p95_latency(&self) -> f64 {
        self.p95_latency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the P99 latency in milliseconds
    pub fn p99_latency(&self) -> f64 {
        self.p99_latency.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get a copy of the status counts
    pub fn status_counts(&self) -> HashMap<u16, usize> {
        self.status_counts.read().clone()
    }

    /// Get the start time
    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    /// Get the end time if available
    pub fn end_time(&self) -> Option<Instant> {
        match self.end_time.read() {
            Ok(guard) => *guard,
            Err(_) => None,
        }
    }

    /// Check if the test is complete
    pub fn is_complete(&self) -> bool {
        self.is_complete.load(Ordering::SeqCst)
    }

    /// Get the URL being tested
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the method being used
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Get the elapsed time in seconds
    pub fn elapsed_seconds(&self) -> f64 {
        match self.end_time.read() {
            Ok(guard) => {
                if let Some(end) = *guard {
                    end.duration_since(self.start_time).as_secs_f64()
                } else {
                    self.start_time.elapsed().as_secs_f64()
                }
            }
            Err(_) => self.start_time.elapsed().as_secs_f64(),
        }
    }

    /// Get the current throughput in requests per second
    pub fn throughput(&self) -> f64 {
        let elapsed = self.elapsed_seconds();
        if elapsed > 0.0 {
            self.completed_requests() as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// A thread-safe shared metrics collector
#[derive(Clone)]
pub struct SharedMetrics {
    pub metrics: Arc<LockFreeMetrics>,
}

impl SharedMetrics {
    /// Create a new shared metrics collector
    pub fn new(url: String, method: String) -> Self {
        SharedMetrics {
            metrics: Arc::new(LockFreeMetrics::new(url, method)),
        }
    }

    /// Record a metric
    pub fn record(&self, metric: &RequestMetric) {
        self.metrics.record(metric);
    }

    /// Process all queued metrics
    pub fn process_metrics(&self) {
        self.metrics.process_queued_metrics();
        self.metrics.update_statistics();
    }

    /// Process queued metrics and update statistics
    #[allow(dead_code)]
    pub fn process_queued_metrics(&self) {
        self.metrics.process_queued_metrics();
    }

    /// Update derived statistics
    #[allow(dead_code)]
    pub fn update_statistics(&self) {
        self.metrics.update_statistics();
    }

    /// Mark the test as complete
    pub fn mark_complete(&self) {
        self.metrics.mark_complete();
    }
}
