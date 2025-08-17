# Overview

Whambam is a simple HTTP load testing tool built in Rust, designed to benchmark web server performance with a focus on high throughput and low overhead. This document provides a detailed technical explanation of the application's architecture, performance optimizations, and implementation details.

# Architecture

## System Overview

Whambam follows a worker pool architecture with lock-free metrics collection, providing a high-performance solution for load testing HTTP endpoints. The system is composed of several key components:

- **Command Parser**: Processes user inputs to configure the test parameters
- **Unified Test Runner**: Coordinates the overall test execution
- **Worker Pool**: Manages concurrent request execution with individual worker threads
- **Metrics Collection**: Lock-free metrics gathering and analysis
- **Interactive UI**: Real-time visualization of test progress and results

## Worker Pool Design

The core of Whambam's performance lies in its worker pool implementation. Instead of traditional thread pools that might experience contention under high load, Whambam uses an asynchronous task-based approach with Tokio:

```rust
pub struct WorkerPool {
    client: Client,
    job_sender: mpsc::Sender<RequestJob>,
    worker_handles: Vec<tokio::task::JoinHandle<()>>,
    is_running: Arc<AtomicBool>,
}

impl WorkerPool {
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
            // Create worker task
            // ...
            worker_handles.push(handle);
        }
        
        WorkerPool {
            client,
            job_sender,
            worker_handles,
            is_running,
        }
    }
}
```

Each worker independently processes requests, allowing for maximum utilization of system resources. The architecture minimizes contention by:

- Using a shared job receiver with a mutex
- Processing metrics asynchronously via channels
- Employing lock-free data structures for statistics

# Performance Optimizations

## Lock-Free Metrics Collection

One of the most significant performance bottlenecks in high-throughput load testing is metrics collection. Whambam implements a lock-free metrics system using atomic operations and concurrent queues:

```rust
pub struct LockFreeMetrics {
    // Atomic counters for frequently updated simple metrics
    completed_requests: AtomicUsize,
    error_count: AtomicUsize,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    
    // Concurrent queue for incoming metrics
    metrics_queue: Arc<SegQueue<RequestMetric>>,
    
    // These are updated less frequently and can use a lightweight RwLock
    status_counts: Arc<PLRwLock<HashMap<u16, usize>>>,
    
    // Histogram for latency calculations
    latency_histogram: Arc<RwLock<Histogram<u64>>>,
    
    // Derived statistics calculated periodically
    min_latency: AtomicU64,
    max_latency: AtomicU64,
    p50_latency: AtomicU64,
    p90_latency: AtomicU64,
    p95_latency: AtomicU64,
    p99_latency: AtomicU64,
}
```

## HTTP Client Optimization

The HTTP client is configured for optimal performance in high-throughput scenarios:

```rust
fn create_http_client(config: &TestConfig) -> Client {
    let mut client_builder = Client::builder();
    
    // Configure options based on user preferences
    // ...
    
    // Optimize connection pooling
    client_builder = client_builder
        .pool_max_idle_per_host(config.concurrent * 2)
        .pool_idle_timeout(Duration::from_secs(300))
        .tcp_keepalive(Duration::from_secs(60));
    
    client_builder.build().unwrap_or_else(|_| {
        // Fallback to default client
        Client::new()
    })
}
```

## Efficient Histogram Implementation

Latency statistics are calculated using HDRHistogram, which provides accurate percentile measurements with minimal overhead:

```rust
// Recording a metric
let latency_as_u64 = (metric.latency_ms * 1000.0) as u64;
if let Ok(mut hist) = self.latency_histogram.write() {
    let _ = hist.record(latency_as_u64);
}

// Updating percentiles
if let Ok(hist) = self.latency_histogram.read() {
    let p50 = hist.value_at_quantile(0.5) as u64;
    let p90 = hist.value_at_quantile(0.9) as u64;
    let p95 = hist.value_at_quantile(0.95) as u64;
    let p99 = hist.value_at_quantile(0.99) as u64;
    
    self.p50_latency.store(p50, Ordering::Relaxed);
    self.p90_latency.store(p90, Ordering::Relaxed);
    self.p95_latency.store(p95, Ordering::Relaxed);
    self.p99_latency.store(p99, Ordering::Relaxed);
}
```

# UI Implementation

Whambam features an interactive terminal UI built with Ratatui and Crossterm. The UI is designed to:

- Display real-time metrics with minimal impact on test performance
- Provide different views of test results (summary, detailed statistics)
- Allow user interaction during test execution

```rust
pub fn run(&mut self) -> Result<()> {
    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Start the event loop
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // ...
    }
}
```

# Does it work?

Let's set up a test.

- Use a trivial, local web server
- Run whambam against it
- Run other similar tools
- Compare results

## Running a local web server

We use `http-server` ([https://github.com/http-party/http-server](https://github.com/http-party/http-server)) to run a local web server. You had me at turtles strapped to rockets.

```bash
$ brew install http-server
$ http-server -s .
```

This will start a local web server on port 8080 that serves files from your current directory.

By default, whambam displays a simple UI in the terminal.

![whambam UI screenshot](/images/ui-benchmark-comparison.png)

# Technology Stack

Whambam leverages several key Rust crates to achieve its performance goals:

- tokio: Asynchronous runtime for concurrent operations
- reqwest: HTTP client with performance optimization options
- ratatui + crossterm: Terminal UI framework for interactive display
- hdrhistogram: Efficient latency percentile tracking
- parking_lot: High-performance synchronization primitives
- crossbeam-queue: Lock-free concurrent queues

## Asynchronous Processing

The application is built on Tokio's async runtime, allowing it to:

- Execute thousands of concurrent HTTP requests with minimal overhead
- Process responses without blocking threads
- Coordinate metrics collection and UI updates efficiently

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Application initialization and execution
    // ...
}
```

## Memory Efficiency

Rust's ownership model and zero-cost abstractions allow Whambam to achieve high performance with minimal memory overhead. The application:

- Reuses buffers where possible
- Avoids unnecessary copies of request/response data
- Uses efficient data structures for metrics collection

# Core Components

## Unified Runner

The `UnifiedRunner` coordinates the overall test execution:

```rust
pub struct UnifiedRunner {
    config: TestConfig,
    metrics: SharedMetrics,
    shared_state: Option<SharedState>,
    is_running: Arc<AtomicBool>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}
```

## Worker Pool Implementation

The worker loop demonstrates how requests are processed efficiently:

```rust
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
```

## Lock-Free Metrics Recording

The metrics recording path is optimized for minimal contention:

```rust
pub fn record(&self, metric: &RequestMetric) {
    // Increment atomic counters immediately
    self.completed_requests.fetch_add(1, Ordering::Relaxed);
    self.bytes_sent.fetch_add(metric.bytes_sent, Ordering::Relaxed);
    self.bytes_received.fetch_add(metric.bytes_received, Ordering::Relaxed);

    // Update error count if needed
    if metric.is_error {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    // Update min/max latency using compare_exchange
    let latency_as_u64 = (metric.latency_ms * 1000.0) as u64;

    // Update min latency with compare_exchange loop
    let mut current_min = self.min_latency.load(Ordering::Relaxed);
    while latency_as_u64 < current_min {
        match self.min_latency.compare_exchange(
            current_min,
            latency_as_u64,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(x) => current_min = x,
        }
    }

    // Queue the metric for batch processing
    self.metrics_queue.push(metric.clone());

    // Periodically process the queued metrics
    let completed = self.completed_requests.load(Ordering::Relaxed);
    if completed % 100 == 0 {
        self.process_queued_metrics();
        self.update_statistics();
    }
}
```

# Does it work? (benchmarks)

Be aware that any server—whether local or not—could be the performance limiter. In fact, for testing comparisons, we want it to be.

## hey

Install and run:

```bash
brew install hey
hey -z 10s -c 125 http://localhost:8080
```

Example results (after warmup):

```text
$ hey -z 10s -c 125 http://localhost:8080

Summary:
  Total:	10.0842 secs
  Slowest:	1.0425 secs
  Fastest:	0.0085 secs
  Average:	0.1013 secs
  Requests/sec:	1228.7495


Response time histogram:
  0.008 [1]	|
  0.112 [11299]	|■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■
  0.215 [1012]	|■■■■
  0.319 [15]	|
  0.422 [12]	|
  0.525 [10]	|
  0.629 [11]	|
  0.732 [7]	|
  0.836 [9]	|
  0.939 [8]	|
  1.042 [7]	|


Latency distribution:
  10% in 0.0877 secs
  25% in 0.0893 secs
  50% in 0.1048 secs
  75% in 0.1081 secs
  90% in 0.1114 secs
  95% in 0.1142 secs
  99% in 0.1283 secs

Details (average, fastest, slowest):
  DNS+dialup:	0.0001 secs, 0.0085 secs, 1.0425 secs
  DNS-lookup:	0.0000 secs, 0.0000 secs, 0.0023 secs
  req write:	0.0000 secs, 0.0000 secs, 0.0003 secs
  resp wait:	0.1012 secs, 0.0085 secs, 1.0342 secs
  resp read:	0.0000 secs, 0.0000 secs, 0.0004 secs

Status code distribution:
  [200]    12391 responses
```

## wrk

Install and run:

```bash
brew install wrk
wrk -t 125 -d 10 -c 125 http://localhost:8080
```

Runs a load test for 10 seconds with 125 concurrent connections. We allocate 125 threads to avoid any contention between threads and connections.

Here are the results, again after letting `http-server` warm up:

```text
$ wrk -t 125 -d 10 -c 125 http://localhost:8080
Running 10s test @ http://localhost:8080
125 threads and 125 connections
Thread Stats   Avg      Stdev     Max   +/- Stdev
Latency   107.51ms   70.56ms   1.04s    96.22%
Req/Sec    10.03      2.51    40.00     94.04%
12484 requests in 10.10s, 847.62MB read
Requests/sec:   1236.51
Transfer/sec:     83.96MB
```

## bombardier

Install and run:

```bash
brew install bombardier
bombardier http://localhost:8080
```

Example results:

```text
$ bombardier http://localhost:8080
Bombarding http://localhost:8080 for 10s using 125 connection(s)
[===============================================================================================================================================] 10s
Done!
Statistics        Avg      Stdev        Max
  Reqs/sec      1173.66     352.69    1654.16
  Latency      106.08ms    17.72ms   540.28ms
  HTTP codes:
    1xx - 0, 2xx - 11847, 3xx - 0, 4xx - 0, 5xx - 0
    others - 0
  Throughput:    79.72MB/s
```

Again, this looks comfortably in range.

## Now whambam

Install and run:

```bash
brew tap cloudy-native/whambam
brew install whambam
whambam -z 10s -c 125 http://localhost:8080
```

By default, whambam displays a simple UI in the terminal (see screenshot above). All the numbers look good and in range.

Note: Output format `--output hey` is currently disabled in this version, but when available it prints a text summary similar to `hey`.
