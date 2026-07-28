// whambam - A high-performance HTTP load testing tool
//
// Copyright (c) 2025 Stephen Harrison
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

use anyhow::{anyhow, Context, Result};
use floating_duration::TimeAsFloat;
use reqwest::Client;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use url::Url;

use super::metrics::SharedMetrics;
use super::types::{HttpMethod, RequestMetric, SharedState, TestConfig};

/// Unified runner implementation that combines worker pool and lock-free metrics
pub struct UnifiedRunner {
    config: TestConfig,
    metrics: SharedMetrics,
    shared_state: Option<SharedState>,
    is_running: Arc<AtomicBool>,
}

impl UnifiedRunner {
    /// Create a new unified runner with the given configuration
    #[allow(dead_code)]
    pub fn new(config: TestConfig) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let metrics = SharedMetrics::new(config.url.clone(), config.method.to_string());

        UnifiedRunner {
            config,
            metrics,
            shared_state: None,
            is_running,
        }
    }

    /// Create a new unified runner with the given configuration and shared state
    pub fn with_state(config: TestConfig, shared_state: SharedState) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let metrics = SharedMetrics::new(config.url.clone(), config.method.to_string());

        UnifiedRunner {
            config,
            metrics,
            shared_state: Some(shared_state),
            is_running,
        }
    }

    /// Stop the test
    #[allow(dead_code)]
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Get a clone of the shared metrics
    #[allow(dead_code)]
    pub fn metrics(&self) -> SharedMetrics {
        self.metrics.clone()
    }

    /// Set the shared metrics to use for this runner
    #[allow(dead_code)]
    pub fn set_metrics(&mut self, metrics: SharedMetrics) {
        self.metrics = metrics;
    }

    /// Start the test in a separate task
    pub async fn start(&mut self) -> Result<()> {
        if self.config.concurrent == 0 {
            return Err(anyhow!("concurrent connections must be at least 1 (got 0)"));
        }

        // Validate URL
        let url = Url::parse(&self.config.url).context("Invalid URL")?;

        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        let shared_state = self.shared_state.clone();

        // Metric channel: workers produce, a single consumer updates metrics/UI
        let metric_capacity = config.concurrent.saturating_mul(50).max(1);
        let (metric_tx, mut metric_rx) = mpsc::channel::<RequestMetric>(metric_capacity);

        // Metrics consumer (also drives UI shared state)
        let metrics_clone = metrics.clone();
        let shared_for_metrics = shared_state.clone();
        let metrics_handle = tokio::spawn(async move {
            while let Some(metric) = metric_rx.recv().await {
                metrics_clone.record(&metric);
                if let Some(state) = &shared_for_metrics {
                    let mut guard = state.state.lock().unwrap();
                    guard.update(metric);
                }
            }
            metrics_clone.process_metrics();
        });

        // Load generation + worker lifecycle
        let load_is_running = Arc::clone(&is_running);
        let load_metrics = metrics.clone();
        let load_shared = shared_state.clone();
        let _load_test_handle = tokio::spawn(async move {
            let client = create_http_client(&config);
            let start_time = Instant::now();

            let max_requests = if config.requests > 0 {
                config.requests
            } else {
                usize::MAX
            };

            let max_duration = if config.duration > 0 {
                Some(Duration::from_secs(config.duration))
            } else {
                None
            };

            let worker_pool = WorkerPool::new(
                client,
                config.concurrent,
                metric_tx,
                Arc::clone(&load_is_running),
                config.rate_limit,
            );

            // Stop accepting new work when duration elapses
            let duration_timer = max_duration.map(|max_dur| {
                let flag = Arc::clone(&load_is_running);
                tokio::spawn(async move {
                    tokio::time::sleep(max_dur).await;
                    flag.store(false, Ordering::SeqCst);
                })
            });

            // Submit jobs while running and under the request cap (no fixed 1M ceiling).
            // Channel backpressure keeps memory bounded.
            let mut submitted = 0usize;
            while load_is_running.load(Ordering::SeqCst) && submitted < max_requests {
                let job = RequestJob {
                    url: url.clone(),
                    headers: config.headers.clone(),
                    body: config.body.clone(),
                    basic_auth: config.basic_auth.clone(),
                    method: config.method,
                    timeout: config.timeout,
                    start_time,
                };

                if !worker_pool.submit_job(job).await {
                    break;
                }
                submitted += 1;

                // Yield occasionally so workers/metrics get scheduled
                if submitted.is_multiple_of(256) {
                    tokio::task::yield_now().await;
                }
            }

            // Stop further submission (duration path may already have done this)
            load_is_running.store(false, Ordering::SeqCst);
            if let Some(timer) = duration_timer {
                timer.abort();
            }

            // Close the job queue and wait for workers to drain queued + in-flight work.
            // Once workers exit they drop metric senders; the consumer then finishes.
            worker_pool.close_and_wait().await;
            let _ = metrics_handle.await;

            // Silence unused-variable when submission is empty (e.g. stopped immediately)
            let _ = submitted;

            // Mark complete on lock-free metrics and UI state (do not rely only on metric updates)
            load_metrics.mark_complete();
            if let Some(state) = &load_shared {
                if let Ok(mut guard) = state.state.lock() {
                    guard.mark_complete();
                }
            }
        });

        // Periodic stats processor for lock-free metrics
        let metrics_ref = self.metrics.metrics.clone();
        let _processor_handle = tokio::spawn(async move {
            while !metrics_ref.is_complete() {
                metrics_ref.process_queued_metrics();
                metrics_ref.update_statistics();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            metrics_ref.process_queued_metrics();
            metrics_ref.update_statistics();
        });

        Ok(())
    }
}

/// A request job to be processed by a worker
pub struct RequestJob {
    /// URL to send the request to
    pub url: Url,
    /// HTTP headers to include
    pub headers: Vec<(String, String)>,
    /// Request body data
    pub body: Option<String>,
    /// Basic authentication credentials
    pub basic_auth: Option<(String, String)>,
    /// HTTP method to use
    pub method: HttpMethod,
    /// Request timeout in seconds
    pub timeout: u64,
    /// The start time of the test (for timestamp calculation)
    pub start_time: Instant,
}

/// A worker pool for efficiently processing HTTP requests
pub struct WorkerPool {
    #[allow(dead_code)]
    client: Client,
    job_sender: Option<mpsc::Sender<RequestJob>>,
    worker_handles: Vec<tokio::task::JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
}

impl WorkerPool {
    /// Create a new worker pool with the given configuration
    pub fn new(
        client: Client,
        concurrency: usize,
        metric_sender: mpsc::Sender<RequestMetric>,
        is_running: Arc<AtomicBool>,
        rate_limit: f64,
    ) -> Self {
        let queue_capacity = concurrency.saturating_mul(100).max(1);
        let (job_sender, job_receiver) = mpsc::channel::<RequestJob>(queue_capacity);

        // Share the job receiver among workers
        let job_receiver = Arc::new(tokio::sync::Mutex::new(job_receiver));

        let mut worker_handles = Vec::with_capacity(concurrency);

        for _ in 0..concurrency {
            let worker_client = client.clone();
            let worker_job_receiver = job_receiver.clone();
            let worker_metric_sender = metric_sender.clone();
            let worker_is_running = Arc::clone(&is_running);
            let worker_rate_limit = rate_limit;
            let worker_sem = Arc::new(tokio::sync::Semaphore::new(1));

            let handle = tokio::spawn(async move {
                Self::worker_loop(
                    worker_client,
                    worker_job_receiver,
                    worker_metric_sender,
                    worker_is_running,
                    worker_sem,
                    worker_rate_limit,
                )
                .await;
            });

            worker_handles.push(handle);
        }

        // metric_sender clones live in workers; drop the original so the metric
        // channel closes once all workers exit.
        drop(metric_sender);

        WorkerPool {
            client,
            job_sender: Some(job_sender),
            worker_handles,
            is_running,
        }
    }

    /// Submit a job to the worker pool. Returns false if stopped or channel closed.
    pub async fn submit_job(&self, job: RequestJob) -> bool {
        if !self.is_running.load(Ordering::SeqCst) {
            return false;
        }
        match &self.job_sender {
            Some(sender) => sender.send(job).await.is_ok(),
            None => false,
        }
    }

    /// Stop accepting new work
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Close the job queue and wait for all workers to finish draining
    pub async fn close_and_wait(mut self) {
        self.stop();
        // Dropping the sender closes the channel so workers exit after drain
        self.job_sender.take();
        if !self.worker_handles.is_empty() {
            let _ = futures::future::join_all(self.worker_handles).await;
        }
    }

    /// Main worker processing loop
    async fn worker_loop(
        client: Client,
        job_receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<RequestJob>>>,
        metric_sender: mpsc::Sender<RequestMetric>,
        is_running: Arc<AtomicBool>,
        sem: Arc<tokio::sync::Semaphore>,
        rate_limit: f64,
    ) {
        loop {
            // While running, poll with timeout so we notice stop signals.
            // After stop, block on recv until the queue is drained / closed.
            let job_result = {
                let mut receiver = job_receiver.lock().await;
                if is_running.load(Ordering::SeqCst) {
                    tokio::select! {
                        job = receiver.recv() => job,
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            continue;
                        }
                    }
                } else {
                    // Drain remaining jobs until the sender is dropped
                    receiver.recv().await
                }
            };

            let job = match job_result {
                Some(job) => job,
                None => break, // channel closed and empty
            };

            if rate_limit > 0.0 {
                let delay_ms = (1000.0 / rate_limit) as u64;
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            let _permit = sem.acquire().await.unwrap();

            let result = Self::execute_request(
                &client,
                job.url,
                job.method,
                &job.headers,
                job.body,
                job.basic_auth,
                job.timeout,
                job.start_time,
            )
            .await;

            let _ = metric_sender.send(result).await;
        }
    }

    /// Execute an HTTP request and return metrics
    #[allow(clippy::too_many_arguments)]
    async fn execute_request(
        client: &Client,
        url: Url,
        method: HttpMethod,
        headers: &[(String, String)],
        body: Option<String>,
        basic_auth: Option<(String, String)>,
        timeout: u64,
        start_time: Instant,
    ) -> RequestMetric {
        let bytes_sent = {
            let mut total = 0u64;

            total += method.to_string().len() as u64;
            total += url.path().len() as u64;
            if let Some(query) = url.query() {
                total += query.len() as u64;
            }

            for (name, value) in headers {
                total += name.len() as u64 + value.len() as u64 + 4;
            }

            if let Some(body) = &body {
                total += body.len() as u64;
            }

            total += 50;
            total
        };

        let request_start = Instant::now();

        let mut request_builder = match method {
            HttpMethod::GET => client.get(url),
            HttpMethod::POST => client.post(url),
            HttpMethod::PUT => client.put(url),
            HttpMethod::DELETE => client.delete(url),
            HttpMethod::PATCH => client.patch(url),
            HttpMethod::HEAD => client.head(url),
            HttpMethod::OPTIONS => client.request(reqwest::Method::OPTIONS, url),
            HttpMethod::TRACE => client.request(reqwest::Method::TRACE, url),
            HttpMethod::CONNECT => client.request(reqwest::Method::CONNECT, url),
        };

        if timeout > 0 {
            request_builder = request_builder.timeout(Duration::from_secs(timeout));
        }

        for (name, value) in headers {
            request_builder = request_builder.header(name, value);
        }

        if let Some((username, password)) = &basic_auth {
            request_builder = request_builder.basic_auth(username, Some(password));
        }

        if let Some(body_content) = &body {
            request_builder = request_builder.body(body_content.clone());
        }

        // End-to-end latency: headers + full body (accuracy over TTFB-only timing)
        let result = request_builder.send().await;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let status_class = status / 100;
                let is_error = status_class != 2;

                let bytes_received = match resp.bytes().await {
                    Ok(bytes) => bytes.len() as u64,
                    Err(_) => 0,
                };
                let duration = request_start.elapsed();

                RequestMetric {
                    timestamp: start_time.elapsed().as_fractional_secs(),
                    latency_ms: duration.as_fractional_millis(),
                    status_code: status,
                    is_error,
                    bytes_sent,
                    bytes_received,
                }
            }
            Err(_) => {
                let duration = request_start.elapsed();
                RequestMetric {
                    timestamp: start_time.elapsed().as_fractional_secs(),
                    latency_ms: duration.as_fractional_millis(),
                    status_code: 0,
                    is_error: true,
                    bytes_sent,
                    bytes_received: 0,
                }
            }
        }
    }
}

/// Create an HTTP client with optimal configuration for load testing
fn create_http_client(config: &TestConfig) -> Client {
    let mut client_builder = Client::builder();

    if let Some(proxy) = &config.proxy {
        let proxy_url = format!("http://{proxy}");
        if let Ok(proxy) = reqwest::Proxy::http(&proxy_url) {
            client_builder = client_builder.proxy(proxy);
        }
    }

    if config.disable_compression {
        client_builder = client_builder.no_gzip().no_brotli().no_deflate();
    }

    if config.disable_redirects {
        client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
    }

    // Apply pool settings last so disable-keepalive is not overwritten
    if config.disable_keepalive {
        client_builder = client_builder.tcp_nodelay(true).pool_max_idle_per_host(0);
    } else {
        client_builder = client_builder
            .pool_max_idle_per_host(config.concurrent * 2)
            .pool_idle_timeout(Duration::from_secs(300))
            .tcp_keepalive(Duration::from_secs(60));
    }

    client_builder.build().unwrap_or_else(|_| Client::new())
}
