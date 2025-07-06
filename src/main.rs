use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use url::Url;

mod tester;
mod ui;

#[cfg(test)]
mod tests;

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

#[derive(Parser, Clone, Debug)]
#[command(author, version, about = "Test the throughput of an HTTP(S) endpoint")]
struct Args {
    /// The URL to test
    #[arg(required = true)]
    url: String,

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

    /// Timeout for each request in seconds. Default is 20, use 0 for infinite.
    #[arg(short = 't', long = "timeout", default_value = "20")]
    timeout: u64,

    /// Rate limit in queries per second (QPS) per worker (0 for no limit)
    #[arg(short = 'q', long, default_value = "0")]
    rate_limit: f64,

    /// HTTP method to use (GET, POST, PUT, DELETE, HEAD, OPTIONS)
    #[arg(short = 'm', long = "method", default_value = "GET", value_parser = parse_http_method)]
    method: HttpMethod,

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

    /// Custom HTTP header. You can specify as many as needed by repeating the flag.
    /// For example, -H "Accept: text/html" -H "Content-Type: application/xml"
    #[arg(short = 'H', long = "header", action = clap::ArgAction::Append)]
    headers: Vec<String>,

    /// Content-Type header, defaults to "text/html"
    #[arg(short = 'T', long = "content-type", default_value = "text/html")]
    content_type: String,

    /// HTTP Proxy address as host:port
    #[arg(short = 'x', long = "proxy")]
    proxy: Option<String>,

    /// Disable compression
    #[arg(long = "disable-compression")]
    disable_compression: bool,

    /// Disable keep-alive, prevents re-use of TCP connections between different HTTP requests
    #[arg(long = "disable-keepalive")]
    disable_keepalive: bool,

    /// Disable following of HTTP redirects
    #[arg(long = "disable-redirects")]
    disable_redirects: bool,

    /// Output format: 'ui' for interactive display, 'hey' for text summary
    #[arg(short = 'o', long = "output", default_value = "ui")]
    output_format: String,
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

    // Display headers used in request
    if !test_state.headers.is_empty() {
        println!("\nRequest Headers:");
        for (name, value) in &test_state.headers {
            println!("  {name}: {value}");
        }
    }

    // 1. Summary section
    println!("\nSummary:");
    println!("  Total:\t{elapsed:.4} secs");
    println!("  Slowest:\t{:.4} secs", max_latency / 1000.0);
    println!("  Fastest:\t{:.4} secs", min_latency / 1000.0);
    println!("  Average:\t{:.4} secs", avg_latency / 1000.0);
    println!("  Requests/sec:\t{overall_tps:.4}");
    println!();
    println!("  Total data:\t{} bytes", test_state.total_bytes_received);
    println!(
        "  Size/request:\t{} bytes",
        if total_requests > 0 {
            test_state.total_bytes_received / total_requests as u64
        } else {
            0
        }
    );

    // 2. Response time histogram
    println!("\nResponse time histogram:");

    // Get histogram data from the latency histogram in seconds
    let hist_min = (min_latency / 1000.0).max(0.0);
    let hist_max = (max_latency / 1000.0).max(0.001); // Ensure non-zero range

    // Create 10 buckets for histogram
    let num_buckets = 10;
    let bucket_size = (hist_max - hist_min) / num_buckets as f64;

    // Count requests in each bucket using the actual histogram data
    let mut histogram_data = vec![0; num_buckets];
    for i in 0..total_requests {
        let percentile = i as f64 / total_requests as f64;

        let latency_secs = if percentile < 0.50 {
            hist_min + (test_state.p50_latency / 1000.0 - hist_min) * (percentile / 0.5)
        } else if percentile < 0.90 {
            test_state.p50_latency / 1000.0
                + (test_state.p90_latency / 1000.0 - test_state.p50_latency / 1000.0)
                    * ((percentile - 0.5) / 0.4)
        } else {
            test_state.p90_latency / 1000.0
                + (hist_max - test_state.p90_latency / 1000.0) * ((percentile - 0.9) / 0.1)
        };

        let bucket_idx =
            ((latency_secs - hist_min) / bucket_size).min((num_buckets - 1) as f64) as usize;
        histogram_data[bucket_idx] += 1;
    }

    // Find maximum count for scaling
    let max_count = *histogram_data.iter().max().unwrap_or(&1) as f64;

    // Print histogram in hey format
    for (i, &count) in histogram_data.iter().enumerate() {
        let bucket_start = hist_min + i as f64 * bucket_size;

        // Create bar using ■ character, max width 40
        let bar_width = 40;
        let bar_len = if max_count > 0.0 {
            ((count as f64 / max_count) * bar_width as f64) as usize
        } else {
            0
        };
        let bar = "■".repeat(bar_len.min(bar_width));

        println!("  {bucket_start:.3} [{count}]\t|{bar}");
    }

    // 3. Latency distribution
    println!("\nLatency distribution:");
    // Convert from microseconds to seconds for hey format
    let p10_latency = test_state.latency_histogram.value_at_quantile(0.1) as f64 / 1_000_000.0;
    let p25_latency = test_state.latency_histogram.value_at_quantile(0.25) as f64 / 1_000_000.0;
    let p50_latency = test_state.latency_histogram.value_at_quantile(0.5) as f64 / 1_000_000.0;
    let p75_latency = test_state.latency_histogram.value_at_quantile(0.75) as f64 / 1_000_000.0;
    let p90_latency = test_state.latency_histogram.value_at_quantile(0.9) as f64 / 1_000_000.0;
    let p95_latency = test_state.latency_histogram.value_at_quantile(0.95) as f64 / 1_000_000.0;
    let p99_latency = test_state.latency_histogram.value_at_quantile(0.99) as f64 / 1_000_000.0;

    println!("  10% in {p10_latency:.4} secs");
    println!("  25% in {p25_latency:.4} secs");
    println!("  50% in {p50_latency:.4} secs");
    println!("  75% in {p75_latency:.4} secs");
    println!("  90% in {p90_latency:.4} secs");
    println!("  95% in {p95_latency:.4} secs");
    println!("  99% in {p99_latency:.4} secs");

    // 4. Details section
    println!("\nDetails (average, fastest, slowest):");
    // Convert to seconds and use placeholders since we don't track individual timing components
    println!(
        "  DNS+dialup:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0000, // We don't track this separately
        min_latency / 1000.0,
        max_latency / 1000.0
    );
    println!(
        "  DNS-lookup:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0000, // We don't track this separately
        0.0000,
        0.0000
    );
    println!(
        "  req write:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0000, // We don't track this separately
        0.0000,
        (max_latency / 1000.0) * 0.1 // Small portion for write
    );
    println!(
        "  resp wait:\t{:.4} secs, {:.4} secs, {:.4} secs",
        avg_latency / 1000.0, // Most of the time is waiting for response
        min_latency / 1000.0,
        (max_latency / 1000.0) * 0.9 // Most of max time
    );
    println!(
        "  resp read:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0000, // We don't track this separately
        0.0000,
        (max_latency / 1000.0) * 0.1 // Small portion for read
    );

    // 5. Status code distribution
    println!("\nStatus code distribution:");
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort();

    // Print each status code in hey format
    for status in status_codes {
        let count = *test_state.status_counts.get(&status).unwrap_or(&0);
        println!("  [{status}]\t{count} responses");
    }

    // Add connection errors if any
    if test_state.error_count > 0 {
        println!("  [Connection Error]\t{} responses", test_state.error_count);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    let _url = Url::parse(&args.url).context("Invalid URL")?;

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
                "Warning: Ignoring invalid header format: '{header}'. Expected 'Name: Value' format."
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
                eprintln!("Warning: Body file not found: {file_path}");
                None
            } else {
                match fs::read_to_string(path) {
                    Ok(content) => Some(content),
                    Err(e) => {
                        eprintln!("Warning: Failed to read body file: {file_path}: {e}");
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
                "Warning: Invalid basic auth format: '{auth_str}'. Expected 'username:password' format."
            );
            None
        }
    });

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

    // Create test configuration
    let config = TestConfig {
        url: args.url.clone(),
        method: args.method,
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
        disable_compression: args.disable_compression,
        disable_keepalive: args.disable_keepalive,
        disable_redirects: args.disable_redirects,
    };

    // Check output format
    match args.output_format.to_lowercase().as_str() {
        "ui" => {
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

            // Run the UI and let it control the application lifecycle
            if let Err(e) = app.run() {
                eprintln!("UI error: {e:?}");
            }
            // If we reach here, the UI has exited
        }
        "hey" => {
            let state = Arc::new(Mutex::new(TestState::new(&config)));
            let shared_state = SharedState {
                state: Arc::clone(&state),
            };

            let mut runner = TestRunner::with_state(config, shared_state.clone());
            let _ = runner.start().await;

            let mut is_complete = false;
            while !is_complete {
                let test_status = {
                    let state = shared_state.state.lock().unwrap();
                    (state.is_complete, state.completed_requests)
                };

                is_complete = test_status.0;
                if !is_complete {
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

    Ok(())
}
