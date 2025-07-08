use anyhow::Result;
use futures::future::join_all;
use reqwest::Client;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, Semaphore};
use url::Url;

use super::types::{HttpMethod, Message, RequestMetric, TestConfig};
use floating_duration::TimeAsFloat;
use std::time::{Duration, Instant};

/// A request job to be processed by worker
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
    /// HTTP client used by all workers
    client: Client,
    /// Channel for sending jobs to workers
    job_sender: mpsc::Sender<RequestJob>,
    /// Channel for receiving metrics from workers
    metrics_receiver: mpsc::Receiver<Message>,
    /// Channel for sending metrics from workers
    metrics_sender: mpsc::Sender<Message>,
    /// Flag to control worker shutdown
    is_running: Arc<AtomicBool>,
    /// Semaphore to control concurrency
    concurrency_control: Arc<Semaphore>,
    /// Worker handles
    worker_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl WorkerPool {
    /// Create a new worker pool with the given configuration
    pub async fn new(config: &TestConfig) -> Result<Self> {
        let is_running = Arc::new(AtomicBool::new(true));
        let concurrency = config.concurrent;

        // Create concurrency control semaphore
        let concurrency_control = Arc::new(Semaphore::new(concurrency));

        // Create channels for work distribution
        let (job_sender, job_receiver) = mpsc::channel::<RequestJob>(concurrency * 2);
        let (metrics_sender, metrics_receiver) = mpsc::channel::<Message>(concurrency * 2);

        // Create HTTP client with configuration
        let client = Self::create_http_client(config)?;

        // Create and launch workers
        let mut worker_handles = Vec::with_capacity(concurrency);

        // Share the job receiver among all workers
        let job_receiver = Arc::new(tokio::sync::Mutex::new(job_receiver));

        for _ in 0..concurrency {
            let worker_client = client.clone();
            let worker_job_receiver = job_receiver.clone();
            let worker_metrics_sender = metrics_sender.clone();
            let worker_is_running = Arc::clone(&is_running);
            let worker_concurrency_control = Arc::clone(&concurrency_control);
            let worker_rate_limit = config.rate_limit;

            // Spawn the worker task
            let handle = tokio::spawn(async move {
                Self::worker_loop(
                    worker_client,
                    worker_job_receiver,
                    worker_metrics_sender,
                    worker_is_running,
                    worker_concurrency_control,
                    worker_rate_limit,
                )
                .await;
            });

            worker_handles.push(handle);
        }

        Ok(WorkerPool {
            client,
            job_sender,
            metrics_receiver,
            metrics_sender,
            is_running,
            concurrency_control,
            worker_handles,
        })
    }

    /// Create an HTTP client with the specified configuration
    fn create_http_client(config: &TestConfig) -> Result<Client> {
        let mut client_builder = Client::builder();

        // Configure proxy if specified
        if let Some(proxy) = &config.proxy {
            let proxy_url = format!("http://{proxy}");
            if let Ok(proxy) = reqwest::Proxy::http(&proxy_url) {
                client_builder = client_builder.proxy(proxy);
            } else {
                eprintln!("Warning: Invalid proxy URL.");
            }
        }

        // Configure additional HTTP options
        if config.disable_compression {
            client_builder = client_builder.no_gzip().no_brotli().no_deflate();
        }

        if config.disable_keepalive {
            client_builder = client_builder.tcp_nodelay(true).pool_max_idle_per_host(0);
        }

        if config.disable_redirects {
            client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
        }

        // Use more connection pooling by default
        client_builder = client_builder.pool_max_idle_per_host(100);

        // Build the client
        let client = client_builder.build().unwrap_or_else(|_| {
            eprintln!(
                "Warning: Failed to configure client with specified options. Using default client."
            );
            Client::new()
        });

        Ok(client)
    }

    /// Main worker processing loop
    async fn worker_loop(
        client: Client,
        job_receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<RequestJob>>>,
        metrics_sender: mpsc::Sender<Message>,
        is_running: Arc<AtomicBool>,
        concurrency_control: Arc<Semaphore>,
        rate_limit: f64,
    ) {
        while is_running.load(Ordering::SeqCst) {
            // Acquire a permit from the semaphore to ensure we respect concurrency limits
            let _permit = concurrency_control.acquire().await.unwrap();

            // Get the next job from the channel
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

            // Execute the request
            let result = Self::execute_request(
                &client,
                job.url.clone(),
                job.method,
                &job.headers,
                job.body.clone(),
                job.basic_auth.clone(),
                job.timeout,
                job.start_time,
            )
            .await;

            // Send the result metrics back
            let _ = metrics_sender.send(Message::RequestComplete(result)).await;
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
        // Calculate bytes sent (approximate)
        let bytes_sent = {
            let mut total = 0u64;

            // Add method and path bytes
            total += method.to_string().len() as u64;
            total += url.path().len() as u64;
            if let Some(query) = url.query() {
                total += query.len() as u64;
            }

            // Add header bytes
            for (name, value) in headers {
                total += name.len() as u64 + value.len() as u64 + 4; // ": " + "\r\n"
            }

            // Add body bytes
            if let Some(body) = &body {
                total += body.len() as u64;
            }

            // Add basic HTTP overhead (HTTP/1.1, Host header, etc.)
            total += 50; // Approximate overhead

            total
        };

        // Start timing
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

        // Set request timeout if specified
        if timeout > 0 {
            request_builder = request_builder.timeout(Duration::from_secs(timeout));
        }

        // Add custom headers
        for (name, value) in headers {
            request_builder = request_builder.header(name, value);
        }

        // Add basic auth if provided
        if let Some((username, password)) = &basic_auth {
            request_builder = request_builder.basic_auth(username, Some(password));
        }

        // Add request body if provided
        if let Some(body_content) = &body {
            request_builder = request_builder.body(body_content.clone());
        }

        // Send the request
        let result = request_builder.send().await;
        let duration = request_start.elapsed();

        // Process the response
        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let status_class = status / 100;
                let is_error = status_class != 2; // Consider non-2xx as errors

                // Calculate bytes received
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
                status_code: 0, // No status code for errors
                is_error: true,
                bytes_sent,
                bytes_received: 0,
            },
        }
    }

    /// Submit a job to the worker pool
    pub async fn submit_job(&self, job: RequestJob) -> Result<()> {
        Ok(self.job_sender.send(job).await?)
    }

    /// Get the metrics receiver
    pub fn metrics_receiver(&mut self) -> &mut mpsc::Receiver<Message> {
        &mut self.metrics_receiver
    }

    /// Stop the worker pool
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Wait for all workers to complete
    pub async fn wait(self) {
        // Wait for all worker tasks to complete
        if !self.worker_handles.is_empty() {
            let _ = join_all(self.worker_handles).await;
        }
    }
}
