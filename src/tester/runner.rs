use anyhow::{Context, Result};
// Removed unused imports
use reqwest::Client;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    // thread removed
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use url::Url;
use floating_duration::TimeAsFloat;

use super::types::{Message, RequestMetric, SharedState, TestConfig, TestState};

/// The throughput test runner
pub struct TestRunner {
    config: TestConfig,
    shared_state: SharedState,
    is_running: Arc<AtomicBool>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}

impl TestRunner {
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

    /// Get a reference to the shared state
    pub fn shared_state(&self) -> SharedState {
        SharedState {
            state: Arc::clone(&self.shared_state.state),
        }
    }

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
            let client = Client::new();
            let url = url.clone();
            let _requests_count = Arc::new(AtomicUsize::new(0));
            
            let start_time = Instant::now();
            let max_requests = if config.requests > 0 { config.requests } else { usize::MAX };
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
                    
                    // Make the request
                    let request_start = Instant::now();
                    let result = client_clone.get(url_clone).send().await;
                    let duration = request_start.elapsed();
                    
                    // Create metric
                    let metric = match result {
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            RequestMetric {
                                timestamp: start_time.elapsed().as_fractional_secs(),
                                latency_ms: duration.as_fractional_millis(),
                                status_code: status,
                                is_error: false,
                            }
                        }
                        Err(_) => RequestMetric {
                            timestamp: start_time.elapsed().as_fractional_secs(),
                            latency_ms: duration.as_fractional_millis(),
                            status_code: 0,
                            is_error: true,
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
        test_state.end_time.unwrap().duration_since(test_state.start_time).as_secs_f64()
    } else {
        test_state.start_time.elapsed().as_secs_f64()
    };
    let overall_tps = if elapsed > 0.0 {
        test_state.completed_requests as f64 / elapsed
    } else {
        0.0
    };
    
    println!("\n===== Blamo Web Throughput Test Results =====");
    println!("URL: {}", test_state.url);
    println!("Total Requests: {}", test_state.completed_requests);
    println!("Total Time: {:.2}s", elapsed);
    println!("Average Throughput: {:.2} req/s", overall_tps);
    println!("Error Count: {} ({:.2}%)", 
             test_state.error_count, 
             100.0 * test_state.error_count as f64 / test_state.completed_requests.max(1) as f64);
    
    println!("\nLatency Statistics:");
    println!("  Min: {:.2} ms", if test_state.min_latency == f64::MAX { 0.0 } else { test_state.min_latency });
    println!("  Max: {:.2} ms", test_state.max_latency);
    println!("  P50: {:.2} ms", test_state.p50_latency);
    println!("  P90: {:.2} ms", test_state.p90_latency);
    println!("  P99: {:.2} ms", test_state.p99_latency);
    
    println!("\nStatus Code Distribution:");
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort();
    
    for status in status_codes {
        let count = *test_state.status_counts.get(&status).unwrap_or(&0);
        let percentage = 100.0 * count as f64 / test_state.completed_requests.max(1) as f64;
        println!("  HTTP {}: {} ({:.2}%)", status, count, percentage);
    }
}