import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { Image } from "@heroui/image";
import { Link } from "@heroui/link";
import { Snippet } from "@heroui/snippet";

export default function TechnologyPage() {
  return (
    <DefaultLayout>
      <main className="w-full p-8">
        <section className="mb-16 scroll-mt-24">
          <h1 className={title({ size: "lg" })}>Technical Mumbo Jumbo</h1>
        </section>

        <section id="overview" className="mb-16 scroll-mt-24">
          <div className="space-y-4">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Overview
            </h2>
            <p className="text-default-700">
              Whambam is a simple HTTP load testing tool built in Rust, designed
              to benchmark web server performance with a focus on high
              throughput and low overhead. This document provides a detailed
              technical explanation of the application's architecture,
              performance optimizations, and implementation details.
            </p>
          </div>
        </section>

        <section id="architecture" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Architecture
            </h2>

            <h3 className="text-xl font-bold pt-4">System Overview</h3>
            <p className="text-default-700">
              Whambam follows a worker pool architecture with lock-free metrics
              collection, providing a high-performance solution for load testing
              HTTP endpoints. The system is composed of several key components:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>
                <b>Command Parser</b>: Processes user inputs to configure the
                test parameters
              </li>
              <li>
                <b>Unified Test Runner</b>: Coordinates the overall test
                execution
              </li>
              <li>
                <b>Worker Pool</b>: Manages concurrent request execution with
                individual worker threads
              </li>
              <li>
                <b>Metrics Collection</b>: Lock-free metrics gathering and
                analysis
              </li>
              <li>
                <b>Interactive UI</b>: Real-time visualization of test progress
                and results
              </li>
            </ul>

            <h3 className="text-xl font-bold pt-4">Worker Pool Design</h3>
            <p className="text-default-700">
              The core of Whambam's performance lies in its worker pool
              implementation. Instead of traditional thread pools that might
              experience contention under high load, Whambam uses an
              asynchronous task-based approach with Tokio:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`pub struct WorkerPool {
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
}`}
                </code>
              </pre>
            </Snippet>
            <p className="text-default-700">
              Each worker independently processes requests, allowing for maximum
              utilization of system resources. The architecture minimizes
              contention by:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>Using a shared job receiver with a mutex</li>
              <li>Processing metrics asynchronously via channels</li>
              <li>Employing lock-free data structures for statistics</li>
            </ul>
          </div>
        </section>

        <section id="performance-optimizations" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Performance Optimizations
            </h2>

            <h3 className="text-xl font-bold pt-4">
              Lock-Free Metrics Collection
            </h3>
            <p className="text-default-700">
              One of the most significant performance bottlenecks in
              high-throughput load testing is metrics collection. Whambam
              implements a lock-free metrics system using atomic operations and
              concurrent queues:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`pub struct LockFreeMetrics {
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
    
    // Additional state...
}`}
                </code>
              </pre>
            </Snippet>

            <h3 className="text-xl font-bold pt-4">HTTP Client Optimization</h3>
            <p className="text-default-700">
              The HTTP client is configured for optimal performance in
              high-throughput scenarios:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`fn create_http_client(config: &TestConfig) -> Client {
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
}`}
                </code>
              </pre>
            </Snippet>

            <h3 className="text-xl font-bold pt-4">
              Efficient Histogram Implementation
            </h3>
            <p className="text-default-700">
              Latency statistics are calculated using HDRHistogram, which
              provides accurate percentile measurements with minimal overhead:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`// Recording a metric
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
}`}
                </code>
              </pre>
            </Snippet>
          </div>
        </section>

        <section id="technology-stack" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Technology Stack
            </h2>
            <p className="text-default-700">
              Whambam leverages several key Rust crates to achieve its
              performance goals:
            </p>

            <h3 className="text-xl font-bold pt-4">Core Dependencies</h3>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>
                <b>tokio</b>: Asynchronous runtime for concurrent operations
              </li>
              <li>
                <b>reqwest</b>: HTTP client with performance optimization
                options
              </li>
              <li>
                <b>ratatui + crossterm</b>: Terminal UI framework for
                interactive display
              </li>
              <li>
                <b>hdrhistogram</b>: Efficient latency percentile tracking
              </li>
              <li>
                <b>parking_lot</b>: High-performance synchronization primitives
              </li>
              <li>
                <b>crossbeam-queue</b>: Lock-free concurrent queues
              </li>
            </ul>

            <h3 className="text-xl font-bold pt-4">Asynchronous Processing</h3>
            <p className="text-default-700">
              The application is built on Tokio's async runtime, allowing it to:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>
                Execute thousands of concurrent HTTP requests with minimal
                overhead
              </li>
              <li>Process responses without blocking threads</li>
              <li>Coordinate metrics collection and UI updates efficiently</li>
            </ul>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`#[tokio::main]
async fn main() -> Result<()> {
    // Application initialization and execution
    // ...
}`}
                </code>
              </pre>
            </Snippet>

            <h3 className="text-xl font-bold pt-4">Memory Efficiency</h3>
            <p className="text-default-700">
              Rust's ownership model and zero-cost abstractions allow Whambam to
              achieve high performance with minimal memory overhead. The
              application:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>Reuses buffers where possible</li>
              <li>Avoids unnecessary copies of request/response data</li>
              <li>Uses efficient data structures for metrics collection</li>
            </ul>
          </div>
        </section>

        <section id="core-components" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Core Components
            </h2>

            <h3 className="text-xl font-bold pt-4">Unified Runner</h3>
            <p className="text-default-700">
              The <code>UnifiedRunner</code> coordinates the overall test
              execution:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`pub struct UnifiedRunner {
    config: TestConfig,
    metrics: SharedMetrics,
    shared_state: Option<SharedState>,
    is_running: Arc<AtomicBool>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
}`}
                </code>
              </pre>
            </Snippet>

            <h3 className="text-xl font-bold pt-4">
              Worker Pool Implementation
            </h3>
            <p className="text-default-700">
              The worker loop demonstrates how requests are processed
              efficiently:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`async fn worker_loop(
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
}`}
                </code>
              </pre>
            </Snippet>

            <h3 className="text-xl font-bold pt-4">
              Lock-Free Metrics Recording
            </h3>
            <p className="text-default-700">
              The metrics recording path is optimized for minimal contention:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`pub fn record(&self, metric: &RequestMetric) {
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
}`}
                </code>
              </pre>
            </Snippet>
          </div>
        </section>

        <section id="ui-implementation" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              UI Implementation
            </h2>
            <p className="text-default-700">
              Whambam features an interactive terminal UI built with Ratatui and
              Crossterm. The UI is designed to:
            </p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>
                Display real-time metrics with minimal impact on test
                performance
              </li>
              <li>
                Provide different views of test results (summary, detailed
                statistics)
              </li>
              <li>Allow user interaction during test execution</li>
            </ul>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`pub fn run(&mut self) -> Result<()> {
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
        // Minimize the time we hold the lock - get a snapshot of the state
        let should_quit;

        {
            // CRITICAL: Lock for as little time as possible to avoid blocking the test runner
            let app_state = self.shared_state.state.lock().unwrap();

            // Just render with the current state snapshot
            terminal.draw(|f| ui(f, &app_state, &self.ui_state))?;

            // Store quit value for checking later
            should_quit = app_state.should_quit;
        }

        // Handle events, check for exit conditions
        // ...
    }
}`}
                </code>
              </pre>
            </Snippet>
          </div>
        </section>

        <section id="does-it-work" className="mb-16 scroll-mt-24">
          <div className="space-y-6">
            <h2 className={title({ size: "md", class: "mt-8 mb-6" })}>
              Does it work?
            </h2>
            <p className="text-default-700">Let's set up a test</p>
            <ul className="list-disc pl-5 space-y-1 text-default-600">
              <li>Use a trivial, local web server</li>
              <li>Run whambam against it</li>
              <li>Run other similar tools</li>
              <li>Compare results</li>
            </ul>
            <p className="text-default-700">
              But let's be aware that any server—whether local or not—could well
              be the performance limiter. In fact, if you think about it, we
              definitely want it to be.
            </p>
            <h3 className="text-xl font-bold pt-4">
              Running a local web server
            </h3>
            <p className="text-default-700">
              We use <code>http-server</code> (
              <Link href="https://github.com/http-party/http-server">
                https://github.com/http-party/http-server
              </Link>
              ) to run a local web server. You had me at turtles strapped to
              rockets.
            </p>
            <Snippet variant="bordered">
              <span>brew install http-server</span>
              <span>http-server -s .</span>
            </Snippet>
            <p className="text-default-700">
              This will start a local web server on port 8080 that delivers
              files from wherever you started it.
            </p>
            <h3 className="text-xl font-bold pt-4">
              <code>hey</code>
            </h3>
            <p className="text-default-700">
              <code>hey</code> is the reason we went down this path, so we had
              better be comparable and compatible.
            </p>
            <Snippet variant="bordered">
              <span>brew install hey</span>
            </Snippet>
            <p className="text-default-700">
              Installs <code>hey</code>.
            </p>
            <Snippet variant="bordered">
              <span>hey -z 10s -c 125 http://localhost:8080</span>
            </Snippet>
            <p className="text-default-700">
              Runs a load test for 10 seconds with 125 concurrent connections.
            </p>
            <p className="text-default-700">
              Here are the results after running the test a few times to let
              http-server warm up:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`$ hey -z 10s -c 125 http://localhost:8080

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
  [200]	12391 responses`}
                </code>
              </pre>
            </Snippet>
            <p className="text-default-700">
              <code>hey</code> can drive 1,200+ requests per second, with
              latency distribution fairly narrow.
            </p>

            <h3 className="text-xl font-bold pt-4">
              <code>wrk</code>
            </h3>
            <p className="text-default-700">
              <code>wrk</code> is another great tool. Let's try it.
            </p>
            <Snippet variant="bordered">
              <span>brew install wrk</span>
            </Snippet>
            <p className="text-default-700">
              Installs <code>wrk</code>.
            </p>
            <Snippet variant="bordered">
              <span>wrk -t 125 -d 10 -c 125 http://localhost:8080</span>
            </Snippet>
            <p className="text-default-700">
              Runs a load test for 10 seconds with 125 concurrent connections.
              We allocate 125 threads to avoid any contention between threads
              and connections.
            </p>
            <p className="text-default-700">
              Here are the results, again after letting <code>http-server</code>{" "}
              warm up:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`$ wrk -t 125 -d 10 -c 125 http://localhost:8080
Running 10s test @ http://localhost:8080
125 threads and 125 connections
Thread Stats   Avg      Stdev     Max   +/- Stdev
Latency   107.51ms   70.56ms   1.04s    96.22%
Req/Sec    10.03      2.51    40.00     94.04%
12484 requests in 10.10s, 847.62MB read
Requests/sec:   1236.51
Transfer/sec:     83.96MB`}
                </code>
              </pre>
            </Snippet>
            <p className="text-default-700">
              <code>wrk</code> can also drive 1,200+ requests per second, with
              latency distribution in the same range as <code>hey</code>.
            </p>
            <h3 className="text-xl font-bold pt-4">
              <code>bombardier</code>
            </h3>
            <p className="text-default-700">
              <code>bombardier</code> is yet another great tool (we told you
              there were lots). Let's give that a try too.
            </p>
            <Snippet variant="bordered">
              <span>brew install bombardier</span>
            </Snippet>
            <p className="text-default-700">
              Installs it. And after letting <code>http-server</code> warm up,
              here are the results:
            </p>
            <Snippet hideSymbol hideCopyButton variant="bordered">
              <pre>
                <code>
                  {`$ bombardier http://localhost:8080
Bombarding http://localhost:8080 for 10s using 125 connection(s)
[===============================================================================================================================================] 10s
Done!
Statistics        Avg      Stdev        Max
  Reqs/sec      1173.66     352.69    1654.16
  Latency      106.08ms    17.72ms   540.28ms
  HTTP codes:
    1xx - 0, 2xx - 11847, 3xx - 0, 4xx - 0, 5xx - 0
    others - 0
  Throughput:    79.72MB/s`}
                </code>
              </pre>
            </Snippet>
            <p className="text-default-700">
              Again, this looks comfortably in range.
            </p>

            <h3 className="text-xl font-bold pt-4">
              <code>Now whambam</code>
            </h3>
            <p className="text-default-700">Install like this.</p>
            <Snippet variant="bordered">
              <span>brew tap cloudy-native/whambam</span>
              <span>brew install whambam</span>
            </Snippet>
            <p className="text-default-700">And run like this.</p>
            <Snippet variant="bordered">
              <span>whambam -z 10s -c 125 http://localhost:8080</span>
            </Snippet>
            <p className="text-default-700">
              By default, whambam displays a simple UI in the terminal.
            </p>
            <div className="mt-8 flex justify-center">
              <Image
                shadow="sm"
                radius="lg"
                width="100%"
                alt="whambam UI screenshot"
                src="/images/ui-benchmark-comparison.png"
              />
            </div>
            <p className="text-default-700">
              All the numbers look good and in range. You can also run whambam
              with "<code>--output hey</code>" for output that's similar format
              to <code>hey</code>. Let's take a look.
            </p>
          </div>
        </section>
      </main>
    </DefaultLayout>
  );
}
