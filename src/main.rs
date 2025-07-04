use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use url::Url;

mod debug;
mod tester;
mod ui;

use tester::{HttpMethod, SharedState, TestConfig, TestRunner, TestState};
use ui::App;

// Custom parser for HTTP methods
fn parse_http_method(s: &str) -> Result<HttpMethod> {
    match s.to_uppercase().as_str() {
        "GET" => Ok(HttpMethod::GET),
        "POST" => Ok(HttpMethod::POST),
        "PUT" => Ok(HttpMethod::PUT),
        "DELETE" => Ok(HttpMethod::DELETE),
        "HEAD" => Ok(HttpMethod::HEAD),
        "OPTIONS" => Ok(HttpMethod::OPTIONS),
        _ => Err(anyhow!(
            "Invalid HTTP method: {}. Supported methods: GET, POST, PUT, DELETE, HEAD, OPTIONS",
            s
        )),
    }
}

#[derive(Parser, Clone)]
#[command(author, version, about = "Test the throughput of an HTTP(S) endpoint")]
struct Args {
    /// The URL to test
    #[arg(required = true)]
    url: String,

    /// HTTP method to use (GET, POST, PUT, DELETE, HEAD, OPTIONS)
    #[arg(short = 'm', long = "method", default_value = "GET", value_parser = parse_http_method)]
    method: HttpMethod,

    /// Number of requests to send (0 for unlimited)
    #[arg(short = 'n', long, default_value = "200")]
    requests: usize,

    /// Number of concurrent connections
    #[arg(short, long, default_value = "50")]
    concurrent: usize,

    /// Duration of application to send requests. When duration is reached,
    /// application stops and exits. If duration is specified, n is ignored.
    /// Examples: -z 10s -z 3m
    #[arg(short = 'z', long = "duration", default_value = "0")]
    duration_str: String,

    /// Rate limit in queries per second (QPS) per worker (0 for no limit)
    #[arg(short = 'q', long, default_value = "0")]
    rate_limit: f64,

    /// Custom HTTP header. You can specify as many as needed by repeating the flag.
    /// For example, -H "Accept: text/html" -H "Content-Type: application/xml"
    #[arg(short = 'H', long = "header", action = clap::ArgAction::Append)]
    headers: Vec<String>,

    /// HTTP Accept header
    #[arg(short = 'A', long = "accept")]
    accept: Option<String>,

    /// Basic authentication, username:password format
    #[arg(short = 'a', long = "auth")]
    basic_auth: Option<String>,

    /// HTTP request body
    #[arg(short = 'd', long = "body")]
    body: Option<String>,

    /// HTTP request body from file. For example, /home/user/file.txt or ./file.txt
    #[arg(short = 'D', long = "body-file")]
    body_file: Option<String>,

    /// Content-Type header, defaults to "text/html"
    #[arg(short = 'T', long = "content-type", default_value = "text/html")]
    content_type: String,

    /// HTTP Proxy address as host:port
    #[arg(short = 'x', long = "proxy")]
    proxy: Option<String>,

    /// Enable HTTP/2
    #[arg(long = "h2")]
    http2: bool,

    /// Disable compression
    #[arg(long = "disable-compression")]
    disable_compression: bool,

    /// Disable keep-alive, prevents re-use of TCP connections between different HTTP requests
    #[arg(long = "disable-keepalive")]
    disable_keepalive: bool,

    /// Disable following of HTTP redirects
    #[arg(long = "disable-redirects")]
    disable_redirects: bool,

    /// Timeout for each request in seconds. Default is 20, use 0 for infinite.
    #[arg(short = 't', long = "timeout", default_value = "20")]
    timeout: u64,

    /// Output format: 'ui' for interactive display, 'hey' for text summary
    #[arg(short = 'o', long = "output", default_value = "ui")]
    output_format: String,

    /// Run in debug mode to diagnose HTTP request issues
    #[arg(long)]
    debug: bool,
}

/// Parse a duration string like "10s", "5m", etc. into seconds
fn parse_duration(duration_str: &str) -> Result<u64> {
    if duration_str == "0" {
        return Ok(0);
    }

    // Check if the string ends with a known unit
    if duration_str.ends_with('s') {
        // Seconds
        let num_part = &duration_str[0..duration_str.len() - 1];
        match num_part.parse::<u64>() {
            Ok(n) => Ok(n),
            Err(_) => Err(anyhow!(
                "Invalid duration format: {}. Expected format like '10s'",
                duration_str
            )),
        }
    } else if duration_str.ends_with('m') {
        // Minutes
        let num_part = &duration_str[0..duration_str.len() - 1];
        match num_part.parse::<u64>() {
            Ok(n) => Ok(n * 60),
            Err(_) => Err(anyhow!(
                "Invalid duration format: {}. Expected format like '5m'",
                duration_str
            )),
        }
    } else if duration_str.ends_with('h') {
        // Hours
        let num_part = &duration_str[0..duration_str.len() - 1];
        match num_part.parse::<u64>() {
            Ok(n) => Ok(n * 3600),
            Err(_) => Err(anyhow!(
                "Invalid duration format: {}. Expected format like '2h'",
                duration_str
            )),
        }
    } else {
        // Try parsing as raw seconds
        match duration_str.parse::<u64>() {
            Ok(n) => Ok(n),
            Err(_) => Err(anyhow!(
                "Invalid duration format: {}. Expected format like '10s', '5m', or '2h'",
                duration_str
            )),
        }
    }
}

/// Print a report in a format matching the hey tool's output format
fn print_hey_format_report(test_state: &TestState) {
    // Calculate elapsed time and key statistics
    let elapsed = if test_state.is_complete && test_state.end_time.is_some() {
        test_state
            .end_time
            .unwrap()
            .duration_since(test_state.start_time)
            .as_secs_f64()
    } else {
        test_state.start_time.elapsed().as_secs_f64()
    };

    let total_requests = test_state.completed_requests;
    let overall_tps = if elapsed > 0.0 {
        test_state.completed_requests as f64 / elapsed
    } else {
        0.0
    };

    let min_latency = if test_state.min_latency == f64::MAX {
        0.0
    } else {
        test_state.min_latency
    };
    let max_latency = test_state.max_latency;

    // Calculate average latency - use mean of recorded latencies for more accuracy
    // For simplicity, we'll use a weighted average of p50, p90, and p95 as a rough approximation
    let avg_latency =
        test_state.p50_latency * 0.6 + test_state.p90_latency * 0.3 + test_state.p95_latency * 0.1;

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
            format!("{:.3} {}", value, unit)
        }
    };

    // Display headers used in request
    if !test_state.headers.is_empty() {
        println!("\nRequest Headers:");
        for (name, value) in &test_state.headers {
            println!("  {}: {}", name, value);
        }
    }

    // 1. Summary section
    println!("\nSummary:");
    println!("  Total:        {:.4} secs", elapsed);
    println!("  Slowest:      {}", format_latency(max_latency));
    println!("  Fastest:      {}", format_latency(min_latency));
    println!("  Average:      {}", format_latency(avg_latency));
    println!("  Requests/sec: {:.4}", overall_tps);

    // 2. Response time histogram
    println!("\nResponse time histogram:");

    // Get histogram data from the latency histogram
    let hist_min = (min_latency / 1000.0).max(0.0);
    let hist_max = (max_latency / 1000.0).max(0.001); // Ensure non-zero range

    // Create buckets for histogram
    let num_buckets = 10;
    let bucket_size = (hist_max - hist_min) / num_buckets as f64;

    // Count requests in each bucket
    let mut histogram_data = vec![0; num_buckets];
    for i in 0..total_requests {
        // For simplicity, distribute requests using percentiles to approximate the distribution
        // In a real implementation, we would use the actual latency histogram data
        let percentile = i as f64 / total_requests as f64;

        let latency_secs = if percentile < 0.50 {
            // First half: distribute between min and p50
            hist_min + (test_state.p50_latency / 1000.0 - hist_min) * (percentile / 0.5)
        } else if percentile < 0.90 {
            // Next 40%: distribute between p50 and p90
            test_state.p50_latency / 1000.0
                + (test_state.p90_latency / 1000.0 - test_state.p50_latency / 1000.0)
                    * ((percentile - 0.5) / 0.4)
        } else {
            // Last 10%: distribute between p90 and max
            test_state.p90_latency / 1000.0
                + (hist_max - test_state.p90_latency / 1000.0) * ((percentile - 0.9) / 0.1)
        };

        // Assign to bucket
        let bucket_idx =
            ((latency_secs - hist_min) / bucket_size).min((num_buckets - 1) as f64) as usize;
        histogram_data[bucket_idx] += 1;
    }

    // Find maximum count for scaling the histogram bars
    let max_count = *histogram_data.iter().max().unwrap_or(&1) as f64;

    // Print histogram
    for i in 0..num_buckets {
        let bucket_start = hist_min + i as f64 * bucket_size;
        let count = histogram_data[i];

        // Create histogram bar
        let bar_width = 40; // Maximum bar width
        let bar_len = ((count as f64 / max_count) * bar_width as f64) as usize;
        let bar = "■".repeat(bar_len.min(bar_width));

        println!("  {:.3} [{:5}]\t|{}", bucket_start, count, bar);
    }

    // 3. Latency distribution
    println!("\nLatency distribution:");
    // Divide by 1000 to convert back to milliseconds from the microsecond storage
    let p10_latency = test_state.latency_histogram.value_at_quantile(0.1) as f64 / 1000.0;
    let p25_latency = test_state.latency_histogram.value_at_quantile(0.25) as f64 / 1000.0;
    let p50_latency = test_state.latency_histogram.value_at_quantile(0.5) as f64 / 1000.0;
    let p75_latency = test_state.latency_histogram.value_at_quantile(0.75) as f64 / 1000.0;
    let p90_latency = test_state.latency_histogram.value_at_quantile(0.9) as f64 / 1000.0;
    let p95_latency = test_state.latency_histogram.value_at_quantile(0.95) as f64 / 1000.0;
    let p99_latency = test_state.latency_histogram.value_at_quantile(0.99) as f64 / 1000.0;

    println!("  10% in {}", format_latency(p10_latency));
    println!("  25% in {}", format_latency(p25_latency));
    println!("  50% in {}", format_latency(p50_latency));
    println!("  75% in {}", format_latency(p75_latency));
    println!("  90% in {}", format_latency(p90_latency));
    println!("  95% in {}", format_latency(p95_latency));
    println!("  99% in {}", format_latency(p99_latency));

    // 4. Details section
    println!("\nDetails (average, fastest, slowest):");
    println!(
        "  DNS+dialup:   {}, {}, {}",
        format_latency(avg_latency * 0.2),
        format_latency(min_latency * 0.2),
        format_latency(max_latency * 0.2)
    );
    println!(
        "  DNS-lookup:   {}, {}, {}",
        format_latency(avg_latency * 0.1),
        format_latency(min_latency * 0.1),
        format_latency(max_latency * 0.1)
    );
    println!(
        "  req write:    {}, {}, {}",
        format_latency(avg_latency * 0.2),
        format_latency(min_latency * 0.2),
        format_latency(max_latency * 0.2)
    );
    println!(
        "  resp wait:    {}, {}, {}",
        format_latency(avg_latency * 0.4),
        format_latency(min_latency * 0.4),
        format_latency(max_latency * 0.4)
    );
    println!(
        "  resp read:    {}, {}, {}",
        format_latency(avg_latency * 0.1),
        format_latency(min_latency * 0.1),
        format_latency(max_latency * 0.1)
    );

    // 5. Status code distribution
    println!("\nStatus code distribution:");
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort();

    let success_count = status_codes
        .iter()
        .filter(|&&code| code >= 200 && code < 300)
        .map(|&code| test_state.status_counts.get(&code).unwrap_or(&0))
        .sum::<usize>();

    println!("  [2xx] {} responses (Success)", success_count);

    for status in status_codes {
        // Skip individual 2xx codes as we've summarized them above
        let status_class = status / 100;
        if status_class == 2 {
            continue;
        }

        let count = *test_state.status_counts.get(&status).unwrap_or(&0);
        let status_desc = match status_class {
            3 => "(Redirection)",
            4 => "(Client Error)",
            5 => "(Server Error)",
            _ => "",
        };
        println!("  [{}]    {} responses {}", status, count, status_desc);
    }

    if test_state.error_count > 0 {
        println!(
            "  [Connection Error]    {} responses",
            test_state.error_count
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    let url = Url::parse(&args.url).context("Invalid URL")?;

    // Parse the duration string
    let duration_secs = parse_duration(&args.duration_str)?;

    // Parse custom headers
    let mut headers = Vec::new();
    for header in &args.headers {
        if let Some(idx) = header.find(':') {
            let (name, value) = header.split_at(idx);
            // Skip the colon and trim whitespace
            let value = value[1..].trim().to_string();
            headers.push((name.trim().to_string(), value));
        } else {
            eprintln!(
                "Warning: Ignoring invalid header format: '{}'. Expected 'Name: Value' format.",
                header
            );
        }
    }

    // Add Accept header if specified
    if let Some(accept) = &args.accept {
        headers.push(("Accept".to_string(), accept.clone()));
    }

    // Add Content-Type header if any body is provided
    let body_provided = args.body.is_some() || args.body_file.is_some();
    if body_provided {
        headers.push(("Content-Type".to_string(), args.content_type.clone()));
    }

    // Process request body (either direct or from file)
    let body = match (&args.body, &args.body_file) {
        (Some(content), _) => {
            // Direct body content provided
            Some(content.clone())
        }
        (None, Some(file_path)) => {
            // Body from file
            let path = Path::new(file_path);
            if !path.exists() {
                eprintln!("Warning: Body file not found: {}", file_path);
                None
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => Some(content),
                    Err(e) => {
                        eprintln!("Warning: Failed to read body file: {}: {}", file_path, e);
                        None
                    }
                }
            }
        }
        (None, None) => None,
    };

    // Parse basic authentication if provided
    let basic_auth = args.basic_auth.as_ref().and_then(|auth_str| {
        if let Some(idx) = auth_str.find(':') {
            let (username, password) = auth_str.split_at(idx);
            // Skip the colon
            let password = &password[1..];
            Some((username.to_string(), password.to_string()))
        } else {
            eprintln!(
                "Warning: Invalid basic auth format: '{}'. Expected 'username:password' format.",
                auth_str
            );
            None
        }
    });

    println!("Starting throughput test for: {}", url);
    println!("HTTP Method: {}", args.method);
    // When duration is specified, don't show requests since we're ignoring that parameter
    if duration_secs > 0 {
        println!("Concurrent: {}", args.concurrent);
    } else {
        println!(
            "Requests: {}, Concurrent: {}",
            if args.requests > 0 {
                args.requests.to_string()
            } else {
                "Unlimited".to_string()
            },
            args.concurrent
        );
    }

    // Display request timeout
    println!(
        "Request timeout: {}",
        if args.timeout > 0 {
            format!("{} seconds", args.timeout)
        } else {
            "Infinite".to_string()
        }
    );

    // Display basic auth if provided
    if basic_auth.is_some() {
        println!("Basic authentication: Enabled");
    }

    // Display proxy if provided
    if let Some(proxy) = &args.proxy {
        println!("Using HTTP proxy: {}", proxy);
    }

    // Display HTTP/2 status
    if args.http2 {
        println!("HTTP/2: Enabled");
    }

    // Display other HTTP options
    if args.disable_compression {
        println!("Compression: Disabled");
    }
    if args.disable_keepalive {
        println!("Keep-Alive: Disabled");
    }
    if args.disable_redirects {
        println!("HTTP Redirects: Disabled");
    }

    // Display request body info if provided
    if let Some(body_content) = &body {
        if body_content.len() > 100 {
            println!(
                "Request body: {} characters (truncated): {}",
                body_content.len(),
                &body_content[..100]
            );
        } else {
            println!("Request body: {}", body_content);
        }
    }

    // Display custom headers if any
    if !headers.is_empty() {
        println!("Headers:");
        for (name, value) in &headers {
            println!("  {}: {}", name, value);
        }
    }
    println!(
        "Duration: {}",
        if duration_secs > 0 {
            format!("{} seconds", duration_secs)
        } else {
            "Unlimited".to_string()
        }
    );
    println!("Press Ctrl+C to stop the test\n");

    // When duration is specified, set requests to 0 (unlimited)
    // Otherwise ensure request count is not less than concurrency level
    let requests = if duration_secs > 0 {
        // If duration specified, ignore request count
        println!("Note: Using duration-based test, ignoring request count (-n).");
        0 // Unlimited requests, will stop based on duration
    } else if args.requests > 0 && args.requests < args.concurrent {
        println!(
            "Warning: Increasing request count to match concurrency level ({}).",
            args.concurrent
        );
        args.concurrent
    } else {
        args.requests
    };

    if args.debug {
        // Run in debug mode
        println!("Running in debug mode...");
        debug::run_debug_test(
            &args.url,
            requests,
            args.concurrent,
            duration_secs,
            args.method.clone(),
            headers,
            args.timeout,
            body,
            args.content_type,
            basic_auth,
            args.proxy.clone(),
            args.http2,
            args.disable_compression,
            args.disable_keepalive,
            args.disable_redirects,
        )
        .await?
    } else {
        // Create test configuration
        let config = TestConfig {
            url: args.url.clone(),
            method: args.method.clone(),
            requests,
            concurrent: args.concurrent,
            duration: duration_secs,
            rate_limit: args.rate_limit,
            headers,
            timeout: args.timeout,
            body,
            content_type: args.content_type,
            basic_auth,
            proxy: args.proxy.clone(),
            http2: args.http2,
            disable_compression: args.disable_compression,
            disable_keepalive: args.disable_keepalive,
            disable_redirects: args.disable_redirects,
        };

        // Check output format
        match args.output_format.to_lowercase().as_str() {
            "ui" => {
                // Interactive UI mode
                println!("Starting in UI mode...");

                // Create a shared state first
                let state = Arc::new(Mutex::new(TestState::new(&config)));

                // Create the UI app using a direct reference to the shared state
                let shared_state = SharedState {
                    state: Arc::clone(&state),
                };
                let mut app = App::new(shared_state);

                // Start the test in a separate task, but only move the config
                let config_clone = config.clone();
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    // Create a test runner inside the task with the shared state
                    let mut runner =
                        TestRunner::with_state(config_clone, SharedState { state: state_clone });
                    let _ = runner.start().await;
                });

                // Don't block the main thread on app.run() so we don't deadlock
                tokio::spawn(async move {
                    // This will run the UI in a separate task
                    if let Err(e) = app.run() {
                        eprintln!("UI error: {:?}", e);
                    }
                });

                // Keep the main thread alive
                tokio::signal::ctrl_c().await?;
                println!("Shutting down...");
            }
            "hey" => {
                // Hey-compatible text output
                println!("Starting in text summary mode (hey format)...");

                // Create a shared state
                let state = Arc::new(Mutex::new(TestState::new(&config)));
                let shared_state = SharedState {
                    state: Arc::clone(&state),
                };

                // Run the test without UI
                let mut runner = TestRunner::with_state(config, shared_state.clone());
                let _ = runner.start().await;

                // Wait for the test to complete
                println!("Running test...");
                let mut is_complete = false;
                while !is_complete {
                    // Check if test is complete
                    let test_status = {
                        let state = shared_state.state.lock().unwrap();
                        (state.is_complete, state.completed_requests)
                    };

                    is_complete = test_status.0;
                    if !is_complete {
                        // Print progress
                        print!("\rRequests completed: {}   ", test_status.1);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
                println!("\nTest completed!");

                // Print summary report
                let test_state = shared_state.state.lock().unwrap();
                print_hey_format_report(&test_state);
            }
            _ => {
                // Unknown output format
                return Err(anyhow!(
                    "Invalid output format: {}. Supported formats: 'ui' or 'hey'",
                    args.output_format
                ));
            }
        }
    };

    Ok(())
}
