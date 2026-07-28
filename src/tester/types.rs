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
    PATCH,
    HEAD,
    OPTIONS,
    TRACE,
    CONNECT,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::PATCH => write!(f, "PATCH"),
            HttpMethod::HEAD => write!(f, "HEAD"),
            HttpMethod::OPTIONS => write!(f, "OPTIONS"),
            HttpMethod::TRACE => write!(f, "TRACE"),
            HttpMethod::CONNECT => write!(f, "CONNECT"),
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
    #[allow(dead_code)]
    pub interactive: bool,

    /// Deprecated output format field
    #[deprecated]
    #[allow(dead_code)]
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
#[allow(dead_code)]
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

    // Chart sampling: completed count at last sample (for interval RPS)
    last_sample_completed: usize,

    // Test completion
    pub is_complete: bool,
    pub should_quit: bool,
    pub end_time: Option<Instant>,

    // Byte tracking
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
}

/// How often to append chart samples (seconds). Short enough for sub-second runs.
const CHART_SAMPLE_INTERVAL_SECS: f64 = 0.1;

/// Max chart samples retained (~30s at 0.1s interval).
const CHART_HISTORY_LEN: usize = 300;

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
        self.last_sample_completed = 0;

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

            throughput_data: VecDeque::with_capacity(CHART_HISTORY_LEN),
            latency_data: VecDeque::with_capacity(CHART_HISTORY_LEN),

            min_latency: f64::MAX,
            max_latency: 0.0,
            p50_latency: 0.0,
            p90_latency: 0.0,
            p95_latency: 0.0,
            p99_latency: 0.0,

            current_throughput: 0.0,
            last_sample_completed: 0,

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

        let elapsed = self.start_time.elapsed().as_secs_f64();

        // Count requests into 100ms buckets first so samples can use them
        let bucket = (elapsed * 10.0).floor() / 10.0;
        let last_entry = self.recent_throughput.back().cloned();
        match last_entry {
            Some((b, count)) if (b - bucket).abs() < f64::EPSILON => {
                self.recent_throughput.pop_back();
                self.recent_throughput.push_back((bucket, count + 1.0));
            }
            _ => {
                self.recent_throughput.push_back((bucket, 1.0));
                if self.recent_throughput.len() > 30 {
                    self.recent_throughput.pop_front();
                }
            }
        }

        // Sample charts often enough for short (sub-second) runs
        let last_sample_t = self
            .throughput_data
            .back()
            .map(|&(t, _)| t)
            .unwrap_or(-CHART_SAMPLE_INTERVAL_SECS);

        if elapsed - last_sample_t >= CHART_SAMPLE_INTERVAL_SECS || self.throughput_data.is_empty()
        {
            self.push_chart_sample(elapsed);
        }

        // Check if test is complete based on observed metrics
        if (self.target_requests > 0 && self.completed_requests >= self.target_requests)
            || (self.duration > 0 && elapsed >= self.duration as f64)
        {
            self.mark_complete();
        }
    }

    /// Append one throughput/latency chart sample at `elapsed` seconds.
    fn push_chart_sample(&mut self, elapsed: f64) {
        let dt = if self.throughput_data.is_empty() {
            elapsed.max(1e-6)
        } else {
            let last_t = self.throughput_data.back().map(|&(t, _)| t).unwrap_or(0.0);
            (elapsed - last_t).max(1e-6)
        };
        let dc = self
            .completed_requests
            .saturating_sub(self.last_sample_completed);
        self.current_throughput = dc as f64 / dt;
        self.last_sample_completed = self.completed_requests;

        self.throughput_data
            .push_back((elapsed, self.current_throughput));
        if self.throughput_data.len() > CHART_HISTORY_LEN {
            self.throughput_data.pop_front();
        }

        let avg_latency: f64 = if !self.recent_latencies.is_empty() {
            self.recent_latencies.iter().sum::<f64>() / self.recent_latencies.len() as f64
        } else {
            0.0
        };

        self.latency_data.push_back((elapsed, avg_latency));
        if self.latency_data.len() > CHART_HISTORY_LEN {
            self.latency_data.pop_front();
        }
    }

    /// Mark the test as complete (idempotent). Used when the runner finishes
    /// draining work, not only when a metric arrives after the duration boundary.
    pub fn mark_complete(&mut self) {
        if self.is_complete {
            return;
        }
        self.is_complete = true;
        let end = Instant::now();
        self.end_time = Some(end);

        let elapsed = end.duration_since(self.start_time).as_secs_f64();

        // Final percentile refresh so short runs are not left at zero
        if self.completed_requests > 0 {
            self.p50_latency = self.latency_histogram.value_at_quantile(0.5) as f64 / 1000.0;
            self.p90_latency = self.latency_histogram.value_at_quantile(0.9) as f64 / 1000.0;
            self.p95_latency = self.latency_histogram.value_at_quantile(0.95) as f64 / 1000.0;
            self.p99_latency = self.latency_histogram.value_at_quantile(0.99) as f64 / 1000.0;
        }

        // Final sample so short runs get a visible chart point and non-zero "Current"
        if self.completed_requests > 0 && elapsed > 0.0 {
            let overall = self.completed_requests as f64 / elapsed;
            self.current_throughput = overall;

            // Prefer a clean final point at overall RPS over a stale interval sample
            if let Some(last) = self.throughput_data.back_mut() {
                if (elapsed - last.0) < CHART_SAMPLE_INTERVAL_SECS {
                    *last = (elapsed, overall);
                } else {
                    self.throughput_data.push_back((elapsed, overall));
                    if self.throughput_data.len() > CHART_HISTORY_LEN {
                        self.throughput_data.pop_front();
                    }
                }
            } else {
                self.throughput_data.push_back((elapsed, overall));
            }

            let avg_latency: f64 = if !self.recent_latencies.is_empty() {
                self.recent_latencies.iter().sum::<f64>() / self.recent_latencies.len() as f64
            } else {
                0.0
            };
            if let Some(last) = self.latency_data.back_mut() {
                if (elapsed - last.0) < CHART_SAMPLE_INTERVAL_SECS {
                    *last = (elapsed, avg_latency);
                } else {
                    self.latency_data.push_back((elapsed, avg_latency));
                    if self.latency_data.len() > CHART_HISTORY_LEN {
                        self.latency_data.pop_front();
                    }
                }
            } else {
                self.latency_data.push_back((elapsed, avg_latency));
            }

            self.last_sample_completed = self.completed_requests;
        }
    }
}

/// Shared state wrapper for thread communication
#[derive(Clone)]
pub struct SharedState {
    pub state: Arc<Mutex<TestState>>,
}
