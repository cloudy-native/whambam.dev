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

use anyhow::{Context, Result};
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
use super::types::{HttpMethod, Message, RequestMetric, SharedState, TestConfig};

/// Unified runner implementation that combines worker pool and lock-free metrics
pub struct UnifiedRunner {
    config: TestConfig,
    metrics: SharedMetrics,
    shared_state: Option<SharedState>,
    is_running: Arc<AtomicBool>,
    tx: mpsc::Sender<Message>,
    #[allow(dead_code)]
    rx: mpsc::Receiver<Message>,
}

impl UnifiedRunner {
    /// Create a new unified runner with the given configuration
    #[allow(dead_code)]
    pub fn new(config: TestConfig) -> Self {
        let (tx, rx) = mpsc::channel::<Message>(config.concurrent * 2);
        let is_running = Arc::new(AtomicBool::new(true));
        let metrics = SharedMetrics::new(config.url.clone(), config.method.to_string());

        UnifiedRunner {
            config,
            metrics,
            shared_state: None,
            is_running,
            tx,
            rx,
        }
    }

    /// Create a new unified runner with the given configuration and shared state
    pub fn with_state(config: TestConfig, shared_state: SharedState) -> Self {
        let (tx, rx) = mpsc::channel::<Message>(config.concurrent * 2);
        let is_running = Arc::new(AtomicBool::new(true));
        let metrics = SharedMetrics::new(config.url.clone(), config.method.to_string());

        UnifiedRunner {
            config,
            metrics,
            shared_state: Some(shared_state),
            is_running,
            tx,
            rx,
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
        // Validate URL
        let url = Url::parse(&self.config.url).context("Invalid URL")?;

        // Clone values for task
        let load_tx = self.tx.clone();
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        let metrics = self.metrics.clone();

        // Create a channel for job completion with much larger capacity
        let (job_tx, mut job_rx) = mpsc::channel::<RequestMetric>(config.concurrent * 50);

        // Spawn load test task
        let _load_test_handle = tokio::spawn(async move {
            // Create HTTP client with pooling configuration
            let client = create_http_client(&config);
            let start_time = Instant::now();

            // Calculate test limits
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

            // Create a worker pool with shared ownership
            let worker_pool = Arc::new(WorkerPool::new(
                client,
                config.concurrent,
                job_tx,
                Arc::clone(&is_running),
                config.rate_limit,
            ));

            // A much simpler approach - submit a large number of jobs at once
            let mut _submitted_jobs = 0;
            let job_capacity = 1_000_000; // 1M job limit

            // Calculate how many jobs to actually submit
            // If limited by requests, use that, otherwise use our large capacity
            let jobs_to_submit = if max_requests > 0 {
                max_requests.min(job_capacity)
            } else {
                job_capacity
            };

            // Create a separate task for job submission to avoid blocking
            let job_submitter = tokio::spawn({
                let is_running_clone = Arc::clone(&is_running);
                let url_clone = url.clone();
                let headers_clone = config.headers.clone();
                let body_clone = config.body.clone();
                let auth_clone = config.basic_auth.clone();
                let method_clone = config.method;
                let timeout_clone = config.timeout;
                let pool_clone = Arc::clone(&worker_pool);

                async move {
                    let mut submitted = 0;

                    // Submit jobs in batches to avoid memory issues
                    let batch_size = 1000;
                    let num_batches = jobs_to_submit.div_ceil(batch_size);

                    for _ in 0..num_batches {
                        if !is_running_clone.load(Ordering::SeqCst) {
                            break; // Stop if test is cancelled
                        }

                        // Calculate this batch size
                        let current_batch = batch_size.min(jobs_to_submit - submitted);

                        // Submit a batch of jobs
                        for _ in 0..current_batch {
                            let job = RequestJob {
                                url: url_clone.clone(),
                                headers: headers_clone.clone(),
                                body: body_clone.clone(),
                                basic_auth: auth_clone.clone(),
                                method: method_clone,
                                timeout: timeout_clone,
                                start_time,
                            };

                            // Use async submission to properly backpressure
                            pool_clone.submit_job(job).await;
                            submitted += 1;
                        }

                        // Let other tasks run
                        tokio::task::yield_now().await;
                    }

                    submitted
                }
            });

            // Start a duration-based timer if needed
            let duration_timer = if let Some(max_dur) = max_duration {
                // This task will stop the worker pool when the max duration is reached
                let pool_for_timer = Arc::clone(&worker_pool);
                let timer_handle = tokio::spawn(async move {
                    tokio::time::sleep(max_dur).await;
                    pool_for_timer.stop();
                });
                Some(timer_handle)
            } else {
                None
            };

            // Wait for the job submitter to complete
            if let Ok(count) = job_submitter.await {
                _submitted_jobs = count;
            }

            // If we have a duration timer, wait for it
            if let Some(timer) = duration_timer {
                // We don't care about the result, just making sure it's done
                let _ = timer.await;
            }

            // Job submitters are already awaited in the code above

            // Wait a bit to allow metrics to be processed
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Mark the metrics as complete
            metrics.mark_complete();

            // Send completion message
            let _ = load_tx.send(Message::TestComplete).await;

            // We can't use wait() with Arc since it requires ownership
            // Just sleep a bit longer for workers to complete
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        // Spawn metrics processing task
        let metrics_clone = self.metrics.clone();
        let metrics_tx = self.tx.clone();
        let shared_state = self.shared_state.clone();

        let _metrics_handle = tokio::spawn(async move {
            // Efficiently process batched metrics from job channel
            while let Some(metric) = job_rx.recv().await {
                // Record the metric in the lock-free collector
                metrics_clone.record(&metric);

                // If we have a shared state, update it as well for UI compatibility
                if let Some(state) = &shared_state {
                    let mut guard = state.state.lock().unwrap();
                    guard.update(metric.clone());
                }

                // Send the message for any listeners
                let _ = metrics_tx.send(Message::RequestComplete(metric)).await;
            }

            // Do a final metrics processing
            metrics_clone.process_metrics();
        });

        // Start metrics processor task
        let metrics_ref = self.metrics.metrics.clone();
        let _processor_handle = tokio::spawn(async move {
            while !metrics_ref.is_complete() {
                // Process queued metrics periodically
                metrics_ref.process_queued_metrics();
                metrics_ref.update_statistics();

                // Sleep a bit to reduce CPU usage
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Final processing
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
    job_sender: mpsc::Sender<RequestJob>,
    #[allow(dead_code)]
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
        // Create a channel for distributing jobs with much larger buffer
        let (job_sender, job_receiver) = mpsc::channel::<RequestJob>(concurrency * 100);

        // Share the job receiver among workers
        let job_receiver = Arc::new(tokio::sync::Mutex::new(job_receiver));

        // Create worker tasks
        let mut worker_handles = Vec::with_capacity(concurrency);

        for _ in 0..concurrency {
            let worker_client = client.clone();
            let worker_job_receiver = job_receiver.clone();
            let worker_metric_sender = metric_sender.clone();
            let worker_is_running = Arc::clone(&is_running);
            let worker_rate_limit = rate_limit;

            // Create a semaphore for this worker to control its own concurrency
            let worker_sem = Arc::new(tokio::sync::Semaphore::new(1));

            // Spawn the worker task
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

        WorkerPool {
            client,
            job_sender,
            worker_handles,
            is_running,
        }
    }

    /// Submit a job to the worker pool
    pub async fn submit_job(&self, job: RequestJob) {
        // Send the job to the worker pool
        if self.is_running.load(Ordering::SeqCst) {
            let _ = self.job_sender.send(job).await;
        }
    }

    /// Try to submit a job to the worker pool without awaiting
    /// Returns true if the job was submitted, false otherwise
    #[allow(dead_code)]
    pub fn try_submit_job(&self, job: RequestJob) -> bool {
        // Check if we're still running
        if !self.is_running.load(Ordering::SeqCst) {
            return false;
        }

        // Try to send the job to the worker pool
        self.job_sender.try_send(job).is_ok()
    }

    /// Stop the worker pool
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Wait for all workers to complete
    #[allow(dead_code)]
    pub async fn wait(self) {
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
        while is_running.load(Ordering::SeqCst) {
            // Get the next job with timeout to check for stop condition
            let job_result = {
                let mut receiver = job_receiver.lock().await;
                tokio::select! {
                    job = receiver.recv() => job,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Check if we should stop
                        if !is_running.load(Ordering::SeqCst) {
                            None
                        } else {
                            continue;
                        }
                    }
                }
            };

            let job = match job_result {
                Some(job) => job,
                None => break, // No more jobs or stopping
            };

            // Apply rate limiting if configured
            if rate_limit > 0.0 {
                let delay_ms = (1000.0 / rate_limit) as u64;
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            // Acquire a permit from the semaphore
            let _permit = sem.acquire().await.unwrap();

            // Execute the request
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

            // Send the result metric
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
        // Calculate approximate bytes sent
        let bytes_sent = {
            let mut total = 0u64;

            // Method and path
            total += method.to_string().len() as u64;
            total += url.path().len() as u64;
            if let Some(query) = url.query() {
                total += query.len() as u64;
            }

            // Headers
            for (name, value) in headers {
                total += name.len() as u64 + value.len() as u64 + 4;
            }

            // Body
            if let Some(body) = &body {
                total += body.len() as u64;
            }

            // Basic overhead
            total += 50;

            total
        };

        // Start request timing
        let request_start = Instant::now();

        // Create the request builder based on method
        let mut request_builder = match method {
            HttpMethod::GET => client.get(url),
            HttpMethod::POST => client.post(url),
            HttpMethod::PUT => client.put(url),
            HttpMethod::DELETE => client.delete(url),
            HttpMethod::HEAD => client.head(url),
            HttpMethod::OPTIONS => client.request(reqwest::Method::OPTIONS, url),
        };

        // Set timeout
        if timeout > 0 {
            request_builder = request_builder.timeout(Duration::from_secs(timeout));
        }

        // Add headers
        for (name, value) in headers {
            request_builder = request_builder.header(name, value);
        }

        // Add basic auth
        if let Some((username, password)) = &basic_auth {
            request_builder = request_builder.basic_auth(username, Some(password));
        }

        // Add body
        if let Some(body_content) = &body {
            request_builder = request_builder.body(body_content.clone());
        }

        // Send request and process response
        let result = request_builder.send().await;
        let duration = request_start.elapsed();

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let status_class = status / 100;
                let is_error = status_class != 2;

                let bytes_received = match resp.bytes().await {
                    Ok(bytes) => bytes.len() as u64,
                    Err(_) => 0,
                };

                RequestMetric {
                    timestamp: start_time.elapsed().as_fractional_secs(),
                    latency_ms: duration.as_fractional_millis(),
                    status_code: status,
                    is_error,
                    bytes_sent,
                    bytes_received,
                }
            }
            Err(_) => RequestMetric {
                timestamp: start_time.elapsed().as_fractional_secs(),
                latency_ms: duration.as_fractional_millis(),
                status_code: 0,
                is_error: true,
                bytes_sent,
                bytes_received: 0,
            },
        }
    }
}

/// Create an HTTP client with optimal configuration for load testing
fn create_http_client(config: &TestConfig) -> Client {
    let mut client_builder = Client::builder();

    // Configure proxy if specified
    if let Some(proxy) = &config.proxy {
        let proxy_url = format!("http://{proxy}");
        if let Ok(proxy) = reqwest::Proxy::http(&proxy_url) {
            client_builder = client_builder.proxy(proxy);
        }
    }

    // Configure HTTP options
    if config.disable_compression {
        client_builder = client_builder.no_gzip().no_brotli().no_deflate();
    }

    if config.disable_keepalive {
        client_builder = client_builder.tcp_nodelay(true).pool_max_idle_per_host(0);
    }

    if config.disable_redirects {
        client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
    }

    // Optimize connection pooling
    client_builder = client_builder
        .pool_max_idle_per_host(config.concurrent * 2)
        .pool_idle_timeout(Duration::from_secs(300))
        .tcp_keepalive(Duration::from_secs(60));

    // Build client
    client_builder.build().unwrap_or_else(|_| {
        // Fallback to default client if build fails
        Client::new()
    })
}
