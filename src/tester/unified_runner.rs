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
    rx: mpsc::Receiver<Message>,
}

impl UnifiedRunner {
    /// Create a new unified runner with the given configuration
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
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Get a clone of the shared metrics
    pub fn metrics(&self) -> SharedMetrics {
        self.metrics.clone()
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

        // Create a channel for job completion
        let (job_tx, mut job_rx) = mpsc::channel::<RequestMetric>(config.concurrent * 2);

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

            // Create a worker pool
            let worker_pool = WorkerPool::new(
                client,
                config.concurrent,
                job_tx,
                Arc::clone(&is_running),
                config.rate_limit,
            );

            // Submit jobs to the worker pool
            let mut _submitted_jobs = 0;

            for _i in 0..max_requests {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }

                if let Some(max_dur) = max_duration {
                    if start_time.elapsed() >= max_dur {
                        break;
                    }
                }

                let job = RequestJob {
                    url: url.clone(),
                    headers: config.headers.clone(),
                    body: config.body.clone(),
                    basic_auth: config.basic_auth.clone(),
                    method: config.method,
                    timeout: config.timeout,
                    start_time,
                };

                worker_pool.submit_job(job).await;
                _submitted_jobs += 1;
            }

            // Signal completion
            worker_pool.stop();

            // Wait a bit to allow metrics to be processed
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Mark the metrics as complete
            metrics.mark_complete();

            // Send completion message
            let _ = load_tx.send(Message::TestComplete).await;

            // Wait for worker pool to finish
            worker_pool.wait().await;
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
    client: Client,
    job_sender: mpsc::Sender<RequestJob>,
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
        // Create a channel for distributing jobs
        let (job_sender, job_receiver) = mpsc::channel::<RequestJob>(concurrency * 2);

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

    /// Stop the worker pool
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Wait for all workers to complete
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
            // Get the next job
            let job = {
                let mut receiver = job_receiver.lock().await;
                match receiver.recv().await {
                    Some(job) => job,
                    None => break,
                }
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

/// Generate a final report using the optimized metrics collector
pub fn print_final_report(metrics: &SharedMetrics) {
    let metrics_ref = metrics.metrics.clone();

    // Process any queued metrics
    metrics_ref.process_queued_metrics();
    metrics_ref.update_statistics();

    // Calculate overall elapsed time
    let elapsed = metrics_ref.elapsed_seconds();
    let overall_tps = if elapsed > 0.0 {
        metrics_ref.completed_requests() as f64 / elapsed
    } else {
        0.0
    };

    println!("\n===== WHAMBAM Results =====");
    println!("URL: {}", metrics_ref.url());
    println!("HTTP Method: {}", metrics_ref.method());

    // Get status counts
    let status_counts = metrics_ref.status_counts();

    println!("Total Requests: {}", metrics_ref.completed_requests());
    println!("Total Time: {:.2}s", elapsed);
    println!("Average Throughput: {:.2} req/s", overall_tps);
    println!(
        "Error Count: {} ({:.2}%)",
        metrics_ref.error_count(),
        100.0 * metrics_ref.error_count() as f64 / metrics_ref.completed_requests().max(1) as f64
    );

    // Format bytes function
    let format_bytes = |bytes: u64| -> String {
        if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    };

    println!(
        "Total Bytes Sent: {}",
        format_bytes(metrics_ref.bytes_sent())
    );
    println!(
        "Total Bytes Received: {}",
        format_bytes(metrics_ref.bytes_received())
    );
    println!(
        "Total Bytes: {}",
        format_bytes(metrics_ref.bytes_sent() + metrics_ref.bytes_received())
    );

    // Helper function to format latency with appropriate units
    let format_latency = |latency_ms: f64| -> String {
        let (value, unit) = if latency_ms < 1.0 {
            // Microseconds
            (latency_ms * 1000.0, "μs")
        } else if latency_ms < 1000.0 {
            // Milliseconds
            (latency_ms, "ms")
        } else {
            // Seconds
            (latency_ms / 1000.0, "s")
        };

        // Check if the fractional part is zero
        if value.fract() == 0.0 {
            format!("{} {}", value as i64, unit)
        } else {
            format!("{value:.3} {unit}")
        }
    };

    println!("\nLatency Statistics:");
    println!(
        "  Min: {}",
        format_latency(if metrics_ref.min_latency() == 0.0 {
            0.0
        } else {
            metrics_ref.min_latency()
        })
    );
    println!("  Max: {}", format_latency(metrics_ref.max_latency()));
    println!("  P50: {}", format_latency(metrics_ref.p50_latency()));
    println!("  P90: {}", format_latency(metrics_ref.p90_latency()));
    println!("  P95: {}", format_latency(metrics_ref.p95_latency()));
    println!("  P99: {}", format_latency(metrics_ref.p99_latency()));

    println!("\nStatus Code Distribution:");
    let mut status_codes: Vec<u16> = status_counts.keys().cloned().collect();
    status_codes.sort();

    for status in status_codes {
        let count = *status_counts.get(&status).unwrap_or(&0);
        let percentage = 100.0 * count as f64 / metrics_ref.completed_requests().max(1) as f64;
        println!("  HTTP {status}: {count} ({percentage:.2}%)");
    }
}

