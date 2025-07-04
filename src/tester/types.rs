use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};
use std::fmt::Debug;
use hdrhistogram::Histogram;
//use floating_duration::TimeAsFloat;

/// Configuration for the throughput test
#[derive(Clone)]
pub struct TestConfig {
    /// URL to test
    pub url: String,
    
    /// Number of requests to send (0 for unlimited)
    pub requests: usize,
    
    /// Number of concurrent connections
    pub concurrent: usize,
    
    /// Duration of the test in seconds (0 for unlimited)
    pub duration: u64,
    
    /// Rate limit in queries per second (QPS) per worker (0 for no limit)
    pub rate_limit: f64,
}

/// Metrics for a single request
#[derive(Debug, Clone)]
pub struct RequestMetric {
    pub timestamp: f64,
    pub latency_ms: f64,
    pub status_code: u16,
    pub is_error: bool,
}

/// Messages sent between testing and UI threads
pub enum Message {
    RequestComplete(RequestMetric),
    TestComplete,
}

/// Test state and statistics
#[derive(Debug)]
pub struct TestState {
    // Test configuration
    pub url: String,
    pub target_requests: usize,
    pub concurrent_requests: usize,
    pub duration: u64,
    pub start_time: Instant,
    
    // Result counters
    pub completed_requests: usize,
    pub error_count: usize,
    
    // Status code counts
    pub status_counts: HashMap<u16, usize>,
    
    // Recent metrics
    pub recent_latencies: VecDeque<f64>,
    pub recent_throughput: VecDeque<(f64, f64)>, // (timestamp, requests/sec)
    
    // Histograms
    pub latency_histogram: Histogram<u64>,
    
    // Chart data
    pub throughput_data: VecDeque<(f64, f64)>, // Rolling throughput over time
    pub latency_data: VecDeque<(f64, f64)>,    // Rolling latency over time
    
    // Running statistics
    pub min_latency: f64,
    pub max_latency: f64,
    pub p50_latency: f64,
    pub p90_latency: f64,
    pub p99_latency: f64,
    
    // Current throughput
    pub current_throughput: f64,
    
    // Test completion
    pub is_complete: bool,
    pub should_quit: bool,
    pub end_time: Option<Instant>,
}

impl TestState {
    /// Reset the state for a new test run
    pub fn reset(&mut self) {
        let now = Instant::now();
        self.start_time = now;
        
        // Reset counters
        self.completed_requests = 0;
        self.error_count = 0;
        self.status_counts.clear();
        
        // Reset data collections
        self.recent_latencies.clear();
        self.recent_throughput.clear();
        
        // Reset histogram
        self.latency_histogram = Histogram::<u64>::new(3).unwrap();
        
        // Reset chart data
        self.throughput_data.clear();
        self.latency_data.clear();
        
        // Reset statistics
        self.min_latency = f64::MAX;
        self.max_latency = 0.0;
        self.p50_latency = 0.0;
        self.p90_latency = 0.0;
        self.p99_latency = 0.0;
        self.current_throughput = 0.0;
        
        // Reset status
        self.is_complete = false;
        self.should_quit = false;
        self.end_time = None;
    }
    
    pub fn new(config: &TestConfig) -> Self {
        let now = Instant::now();
        TestState {
            url: config.url.clone(),
            target_requests: config.requests,
            concurrent_requests: config.concurrent,
            duration: config.duration,
            start_time: now,
            
            completed_requests: 0,
            error_count: 0,
            
            status_counts: HashMap::new(),
            
            recent_latencies: VecDeque::with_capacity(100),
            recent_throughput: VecDeque::with_capacity(30),
            
            latency_histogram: Histogram::<u64>::new(3).unwrap(),
            
            throughput_data: VecDeque::with_capacity(60),
            latency_data: VecDeque::with_capacity(60),
            
            min_latency: f64::MAX,
            max_latency: 0.0,
            p50_latency: 0.0,
            p90_latency: 0.0,
            p99_latency: 0.0,
            
            current_throughput: 0.0,
            
            is_complete: false,
            should_quit: false,
            end_time: None,
        }
    }
    
    pub fn update(&mut self, metric: RequestMetric) {
        // Update counters
        self.completed_requests += 1;
        
        if metric.is_error {
            self.error_count += 1;
        } else {
            *self.status_counts.entry(metric.status_code).or_insert(0) += 1;
        }
        
        // Update latency stats
        let latency = metric.latency_ms;
        self.recent_latencies.push_back(latency);
        if self.recent_latencies.len() > 100 {
            self.recent_latencies.pop_front();
        }
        
        // Convert from f64 to u64 (milliseconds * 10 for sub-millisecond precision)
        self.latency_histogram.record((latency * 10.0) as u64).unwrap();
        
        // Update min/max
        if latency < self.min_latency {
            self.min_latency = latency;
        }
        if latency > self.max_latency {
            self.max_latency = latency;
        }
        
        // Update percentiles
        if self.completed_requests % 10 == 0 {
            self.p50_latency = self.latency_histogram.value_at_quantile(0.5) as f64 / 10.0;
            self.p90_latency = self.latency_histogram.value_at_quantile(0.9) as f64 / 10.0;
            self.p99_latency = self.latency_histogram.value_at_quantile(0.99) as f64 / 10.0;
        }
        
        // Update throughput calculations once per second
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let last_throughput_time = self.throughput_data.back().map(|&(t, _)| t).unwrap_or(0.0);
        
        if elapsed - last_throughput_time >= 1.0 || self.throughput_data.is_empty() {
            // Calculate current throughput (requests per second)
            if !self.recent_throughput.is_empty() {
                let window_size = self.recent_throughput.len().min(10) as f64;
                let sum: f64 = self.recent_throughput.iter().map(|&(_, tps)| tps).sum();
                self.current_throughput = sum / window_size;
            }
            
            // Add data points for charts
            self.throughput_data.push_back((elapsed, self.current_throughput));
            if self.throughput_data.len() > 60 {
                self.throughput_data.pop_front();
            }
            
            let avg_latency: f64 = if !self.recent_latencies.is_empty() {
                self.recent_latencies.iter().sum::<f64>() / self.recent_latencies.len() as f64
            } else {
                0.0
            };
            
            self.latency_data.push_back((elapsed, avg_latency));
            if self.latency_data.len() > 60 {
                self.latency_data.pop_front();
            }
        }
        
        // Add throughput data point
        let second_bucket = elapsed.floor();
        let last_entry = self.recent_throughput.back().cloned();
        
        match last_entry {
            Some((bucket, count)) if bucket == second_bucket => {
                // Update existing bucket
                self.recent_throughput.pop_back();
                self.recent_throughput.push_back((bucket, count + 1.0));
            }
            _ => {
                // Create new bucket
                self.recent_throughput.push_back((second_bucket, 1.0));
                if self.recent_throughput.len() > 30 {
                    self.recent_throughput.pop_front();
                }
            }
        }
        
        // Check if test is complete
        if (self.target_requests > 0 && self.completed_requests >= self.target_requests) || 
           (self.duration > 0 && elapsed >= self.duration as f64) {
            // Only mark as complete and store end time if not already complete
            if !self.is_complete {
                self.is_complete = true;
                self.end_time = Some(Instant::now());
            }
        }
    }
}

/// Shared state wrapper for thread communication
pub struct SharedState {
    pub state: Arc<Mutex<TestState>>,
}