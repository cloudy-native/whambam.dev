use anyhow::{Context, Result};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use url::Url;

use super::types::{Message, SharedState, TestConfig, TestState};
use super::worker_pool::{RequestJob, WorkerPool};

/// The optimized throughput test runner using a worker pool
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
        let (tx, rx) = mpsc::channel::<Message>(config.concurrent * 2);
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
        let (tx, rx) = mpsc::channel::<Message>(config.concurrent * 2);
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
        let state_clone = Arc::clone(&self.shared_state.state);

        // Use the existing receiver
        let mut rx = std::mem::replace(&mut self.rx, mpsc::channel::<Message>(config.concurrent * 2).1);

        // Spawn load test task
        let _load_test_handle = tokio::spawn(async move {
            // Create the worker pool
            let worker_pool = match WorkerPool::new(&config).await {
                Ok(pool) => pool,
                Err(e) => {
                    eprintln!("Failed to create worker pool: {}", e);
                    return;
                }
            };
            
            // Calculate test parameters
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
            
            // Submit all jobs to the worker pool
            let mut submitted_jobs = 0;
            for _ in 0..max_requests {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }

                // Check duration limit
                if let Some(max_dur) = max_duration {
                    if start_time.elapsed() >= max_dur {
                        break;
                    }
                }
                
                // Create job
                let job = RequestJob {
                    url: url.clone(),
                    headers: config.headers.clone(),
                    body: config.body.clone(),
                    basic_auth: config.basic_auth.clone(),
                    method: config.method,
                    timeout: config.timeout,
                    start_time,
                };
                
                // Submit job
                if let Err(e) = worker_pool.submit_job(job).await {
                    eprintln!("Failed to submit job: {}", e);
                    break;
                }
                
                submitted_jobs += 1;
            }
            
            // Shut down the worker pool
            worker_pool.stop();
            
            // Signal that we're done
            let _ = tx.send(Message::TestComplete).await;
            
            // Wait for worker pool to complete
            worker_pool.wait().await;
        });

        // Spawn metrics processing task
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