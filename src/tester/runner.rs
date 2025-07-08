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
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use url::Url;

use super::types::{HttpMethod, Message, RequestMetric, SharedState, TestConfig, TestState};

/// The throughput test runner
pub struct TestRunner {
    config: TestConfig,
    shared_state: SharedState,
    is_running: Arc<AtomicBool>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}

impl TestRunner {
    #[allow(dead_code)]
    /// Create a new test runner with the given configuration
    pub fn new(config: TestConfig) -> Self {
        let state = Arc::new(std::sync::Mutex::new(TestState::new(&config)));
        let (tx, rx) = mpsc::channel::<Message>(100);
        let is_running = Arc::new(AtomicBool::new(true));

        TestRunner {
            config,
            shared_state: SharedState { state },
            is_running,
            tx,
            rx,
        }
    }

    /// Create a new test runner with an existing shared state
    pub fn with_state(config: TestConfig, shared_state: SharedState) -> Self {
        let (tx, rx) = mpsc::channel::<Message>(100);
        let is_running = Arc::new(AtomicBool::new(true));

        TestRunner {
            config,
            shared_state,
            is_running,
            tx,
            rx,
        }
    }

    #[allow(dead_code)]
    /// Get a reference to the shared state
    pub fn shared_state(&self) -> SharedState {
        SharedState {
            state: Arc::clone(&self.shared_state.state),
        }
    }

    #[allow(dead_code)]
    /// Stop the test
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    /// Start the test in a separate task
    pub async fn start(&mut self) -> Result<()> {
        // Validate URL
        let url = Url::parse(&self.config.url).context("Invalid URL")?;

        // Clone values for task
        let tx = self.tx.clone();
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone();
        let _state = Arc::clone(&self.shared_state.state);

        // Use the existing receiver
        let mut rx = std::mem::replace(&mut self.rx, mpsc::channel::<Message>(100).1);

        // Spawn load test task
        let _load_test_handle = tokio::spawn(async move {
            // Create client with specified configuration
            let client = {
                let mut client_builder = Client::builder();

                // Configure proxy if specified
                if let Some(proxy) = &config.proxy {
                    let proxy_url = format!("http://{proxy}");
                    match reqwest::Proxy::http(&proxy_url) {
                        Ok(proxy) => {
                            client_builder = client_builder.proxy(proxy);
                        }
                        Err(_) => {
                            eprintln!("Warning: Invalid proxy URL.");
                        }
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

                // Build the client
                client_builder.build().unwrap_or_else(|_| {
                    eprintln!("Warning: Failed to configure client with specified options. Using default client.");
                    Client::new()
                })
            };
            let url = url.clone();
            let _requests_count = Arc::new(AtomicUsize::new(0));

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

            // Use a different approach to manage concurrency
            let mut handles = Vec::new();

            for _i in 0..max_requests {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }

                // Check duration limit
                if let Some(max_dur) = max_duration {
                    if start_time.elapsed() >= max_dur {
                        break;
                    }
                }

                // Create clones for this task
                let client_clone = client.clone();
                let url_clone = url.clone();
                let tx_clone = tx.clone();
                let is_running_clone = Arc::clone(&is_running);

                // Create clones of data we need in the task to avoid ownership issues
                let headers_clone = config.headers.clone();
                let body_clone = config.body.clone();
                let basic_auth_clone = config.basic_auth.clone();

                // Spawn a task for this request
                let handle = tokio::spawn(async move {
                    // Apply rate limiting if configured
                    if config.rate_limit > 0.0 {
                        let delay_ms = (1000.0 / config.rate_limit) as u64;
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }

                    // Check if we should stop
                    if !is_running_clone.load(Ordering::SeqCst) {
                        return;
                    }

                    // Make the request with specified method
                    let request_start = Instant::now();

                    // Calculate bytes sent (approximate) - before moving url_clone
                    let bytes_sent = {
                        let mut total = 0u64;

                        // Add method and path bytes
                        total += config.method.to_string().len() as u64;
                        total += url_clone.path().len() as u64;
                        if let Some(query) = url_clone.query() {
                            total += query.len() as u64;
                        }

                        // Add header bytes
                        for (name, value) in &headers_clone {
                            total += name.len() as u64 + value.len() as u64 + 4;
                            // ": " + "\r\n"
                        }

                        // Add body bytes
                        if let Some(body) = &body_clone {
                            total += body.len() as u64;
                        }

                        // Add basic HTTP overhead (HTTP/1.1, Host header, etc.)
                        total += 50; // Approximate overhead

                        total
                    };

                    // Create the request builder based on method
                    let mut request_builder = match config.method {
                        HttpMethod::GET => client_clone.get(url_clone),
                        HttpMethod::POST => client_clone.post(url_clone),
                        HttpMethod::PUT => client_clone.put(url_clone),
                        HttpMethod::DELETE => client_clone.delete(url_clone),
                        HttpMethod::HEAD => client_clone.head(url_clone),
                        HttpMethod::OPTIONS => {
                            client_clone.request(reqwest::Method::OPTIONS, url_clone)
                        }
                    };

                    // Set request timeout if specified
                    if config.timeout > 0 {
                        request_builder =
                            request_builder.timeout(Duration::from_secs(config.timeout));
                    }

                    // Add custom headers from the clone we created before spawning
                    for (name, value) in &headers_clone {
                        request_builder = request_builder.header(name, value);
                    }

                    // Add basic auth if provided
                    if let Some((username, password)) = &basic_auth_clone {
                        request_builder = request_builder.basic_auth(username, Some(password));
                    }

                    // Add request body if provided
                    if let Some(body_content) = &body_clone {
                        request_builder = request_builder.body(body_content.clone());
                    }

                    // Send the request
                    let result = request_builder.send().await;
                    let duration = request_start.elapsed();

                    // Create metric
                    let metric = match result {
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
                            status_code: 0,
                            is_error: true,
                            bytes_sent,
                            bytes_received: 0,
                        },
                    };

                    // Send metric update
                    let _ = tx_clone.send(Message::RequestComplete(metric)).await;
                });

                handles.push(handle);

                // Maintain concurrency level by waiting for one task to complete
                // when we reach the concurrency limit
                if handles.len() >= config.concurrent {
                    if let Some(handle) = handles.pop() {
                        let _ = handle.await;
                    }
                }
            }

            // Wait for remaining requests to complete
            for handle in handles {
                let _ = handle.await;
            }

            // Signal that we're done
            let _ = tx.send(Message::TestComplete).await;
        });

        // Spawn metrics processing task
        let state_clone = Arc::clone(&self.shared_state.state);
        let _metrics_handle = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match message {
                    Message::RequestComplete(metric) => {
                        let mut app_state = state_clone.lock().unwrap();
                        app_state.update(metric);
                    }
                    Message::TestComplete => {
                        break;
                    }
                }
            }
        });

        // Don't wait for tasks to complete, just let them run independently
        // The UI thread will handle displaying results from shared state

        Ok(())
    }
}

/// Generate a final report from the test state
pub fn print_final_report(test_state: &TestState) {
    let elapsed = if test_state.is_complete && test_state.end_time.is_some() {
        test_state
            .end_time
            .unwrap()
            .duration_since(test_state.start_time)
            .as_secs_f64()
    } else {
        test_state.start_time.elapsed().as_secs_f64()
    };
    let overall_tps = if elapsed > 0.0 {
        test_state.completed_requests as f64 / elapsed
    } else {
        0.0
    };

    println!("\n===== WHAMBAM Results =====");
    println!("URL: {}", test_state.url);
    println!("HTTP Method: {}", test_state.method);

    // Display custom headers if any
    if !test_state.headers.is_empty() {
        println!("Custom headers:");
        for (name, value) in &test_state.headers {
            println!("  {name}: {value}");
        }
    }

    println!("Total Requests: {}", test_state.completed_requests);
    println!("Total Time: {elapsed:.2}s");
    println!("Average Throughput: {overall_tps:.2} req/s");
    println!(
        "Error Count: {} ({:.2}%)",
        test_state.error_count,
        100.0 * test_state.error_count as f64 / test_state.completed_requests.max(1) as f64
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
        format_bytes(test_state.total_bytes_sent)
    );
    println!(
        "Total Bytes Received: {}",
        format_bytes(test_state.total_bytes_received)
    );
    println!(
        "Total Bytes: {}",
        format_bytes(test_state.total_bytes_sent + test_state.total_bytes_received)
    );

    // Helper function to format latency with appropriate units and hide trailing zeros
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
        format_latency(if test_state.min_latency == f64::MAX {
            0.0
        } else {
            test_state.min_latency
        })
    );
    println!("  Max: {}", format_latency(test_state.max_latency));
    println!("  P50: {}", format_latency(test_state.p50_latency));
    println!("  P90: {}", format_latency(test_state.p90_latency));
    println!("  P95: {}", format_latency(test_state.p95_latency));
    println!("  P99: {}", format_latency(test_state.p99_latency));

    println!("\nStatus Code Distribution:");
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort();

    for status in status_codes {
        let count = *test_state.status_counts.get(&status).unwrap_or(&0);
        let percentage = 100.0 * count as f64 / test_state.completed_requests.max(1) as f64;
        println!("  HTTP {status}: {count} ({percentage:.2}%)");
    }
}
