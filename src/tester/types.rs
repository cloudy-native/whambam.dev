use hdrhistogram::Histogram;
use std::fmt::Debug;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Instant,
};
//use floating_duration::TimeAsFloat;

/// HTTP methods supported for testing
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::HEAD => write!(f, "HEAD"),
            HttpMethod::OPTIONS => write!(f, "OPTIONS"),
        }
    }
}

/// Configuration for the throughput test
#[derive(Clone)]
pub struct TestConfig {
    /// URL to test
    pub url: String,

    /// HTTP method to use
    pub method: HttpMethod,

    /// Number of requests to send (0 for unlimited)
    pub requests: usize,

    /// Number of concurrent connections
    pub concurrent: usize,

    /// Duration of the test in seconds (0 for unlimited)
    pub duration: u64,

    /// Rate limit in queries per second (QPS) per worker (0 for no limit)
    pub rate_limit: f64,

    /// Custom HTTP headers to include with each request
    pub headers: Vec<(String, String)>,

    /// Timeout for each request in seconds (0 for no timeout)
    pub timeout: u64,

    /// Request body as a string
    pub body: Option<String>,

    /// Content-Type header value
    #[allow(dead_code)]
    pub content_type: String,

    /// Basic authentication in (username, password) format
    pub basic_auth: Option<(String, String)>,

    /// HTTP proxy address in host:port format
    pub proxy: Option<String>,

    /// Whether to disable compression
    pub disable_compression: bool,

    /// Whether to disable keep-alive (prevent TCP connection reuse)
    pub disable_keepalive: bool,

    /// Whether to disable following redirects
    pub disable_redirects: bool,

    /// Whether to use interactive UI
    pub interactive: bool,

    /// Output format ("ui" or "hey")
    pub output_format: String,
}

/// Metrics for a single request
#[derive(Debug, Clone)]
pub struct RequestMetric {
    #[allow(dead_code)]
    pub timestamp: f64,
    pub latency_ms: f64,
    pub status_code: u16,
    pub is_error: bool,
    pub bytes_sent: u64,
    pub bytes_received: u64,
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
    pub method: HttpMethod,
    pub target_requests: usize,
    pub concurrent_requests: usize,
    pub duration: u64,
    pub start_time: Instant,
    pub headers: Vec<(String, String)>,

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
    pub p95_latency: f64,
    pub p99_latency: f64,

    // Current throughput
    pub current_throughput: f64,

    // Test completion
    pub is_complete: bool,
    pub should_quit: bool,
    pub end_time: Option<Instant>,

    // Byte tracking
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
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

        // Reset histogram with higher precision (5 significant figures)
        self.latency_histogram = Histogram::<u64>::new(5).unwrap();

        // Reset chart data
        self.throughput_data.clear();
        self.latency_data.clear();

        // Reset statistics
        self.min_latency = f64::MAX;
        self.max_latency = 0.0;
        self.p50_latency = 0.0;
        self.p90_latency = 0.0;
        self.p95_latency = 0.0;
        self.p99_latency = 0.0;
        self.current_throughput = 0.0;

        // Reset status
        self.is_complete = false;
        self.should_quit = false;
        self.end_time = None;

        // Reset byte tracking
        self.total_bytes_sent = 0;
        self.total_bytes_received = 0;
    }

    pub fn new(config: &TestConfig) -> Self {
        let now = Instant::now();
        TestState {
            url: config.url.clone(),
            method: config.method,
            target_requests: config.requests,
            concurrent_requests: config.concurrent,
            duration: config.duration,
            start_time: now,
            headers: config.headers.clone(),

            completed_requests: 0,
            error_count: 0,

            status_counts: HashMap::new(),

            recent_latencies: VecDeque::with_capacity(100),
            recent_throughput: VecDeque::with_capacity(30),

            // Higher precision for latency histogram (5 significant figures instead of 3)
            latency_histogram: Histogram::<u64>::new(5).unwrap(),

            throughput_data: VecDeque::with_capacity(60),
            latency_data: VecDeque::with_capacity(60),

            min_latency: f64::MAX,
            max_latency: 0.0,
            p50_latency: 0.0,
            p90_latency: 0.0,
            p95_latency: 0.0,
            p99_latency: 0.0,

            current_throughput: 0.0,

            is_complete: false,
            should_quit: false,
            end_time: None,

            total_bytes_sent: 0,
            total_bytes_received: 0,
        }
    }

    pub fn update(&mut self, metric: RequestMetric) {
        // Update counters
        self.completed_requests += 1;

        // Update byte counters
        self.total_bytes_sent += metric.bytes_sent;
        self.total_bytes_received += metric.bytes_received;

        // Always update status counts with the status code
        if metric.status_code > 0 {
            // Only update if there is a valid status code
            *self.status_counts.entry(metric.status_code).or_insert(0) += 1;
        }

        // Update error count if it's an error (now includes non-2xx responses)
        if metric.is_error {
            self.error_count += 1;
        }

        // Update latency stats
        let latency = metric.latency_ms;
        self.recent_latencies.push_back(latency);
        if self.recent_latencies.len() > 100 {
            self.recent_latencies.pop_front();
        }

        // Convert from f64 to u64 with higher resolution (microseconds = milliseconds * 1000)
        // This gives us nanosecond-level precision for recording in the histogram
        self.latency_histogram
            .record((latency * 1000.0) as u64)
            .unwrap();

        // Update min/max
        if latency < self.min_latency {
            self.min_latency = latency;
        }
        if latency > self.max_latency {
            self.max_latency = latency;
        }

        // Update percentiles
        if self.completed_requests % 10 == 0 {
            // Divide by 1000 to convert back to milliseconds from the microsecond storage
            self.p50_latency = self.latency_histogram.value_at_quantile(0.5) as f64 / 1000.0;
            self.p90_latency = self.latency_histogram.value_at_quantile(0.9) as f64 / 1000.0;
            self.p95_latency = self.latency_histogram.value_at_quantile(0.95) as f64 / 1000.0;
            self.p99_latency = self.latency_histogram.value_at_quantile(0.99) as f64 / 1000.0;
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
            self.throughput_data
                .push_back((elapsed, self.current_throughput));
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
        if (self.target_requests > 0 && self.completed_requests >= self.target_requests)
            || (self.duration > 0 && elapsed >= self.duration as f64)
        {
            // Only mark as complete and store end time if not already complete
            if !self.is_complete {
                self.is_complete = true;
                self.end_time = Some(Instant::now());
            }
        }
    }
}

/// Shared state wrapper for thread communication
#[derive(Clone)]
pub struct SharedState {
    pub state: Arc<Mutex<TestState>>,
}
