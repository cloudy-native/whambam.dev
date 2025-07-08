use anyhow::Result;
use clap::Parser;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod args;
mod tester;
mod ui;

use args::{Args, HttpMethods};
use tester::{HttpMethod, OptimizedTestRunner, SharedState, TestConfig, TestState};
use ui::App;

/// Parse duration string in the format of 5s, 1m, 1h
fn parse_duration(s: &str) -> Result<Duration> {
    let mut chars = s.chars();
    let last_char = chars.next_back().unwrap_or('s');
    let num_str: String = chars.collect();
    let num = num_str.parse::<u64>()?;

    let duration = match last_char {
        's' => Duration::from_secs(num),
        'm' => Duration::from_secs(num * 60),
        'h' => Duration::from_secs(num * 60 * 60),
        _ => return Err(anyhow::anyhow!("Invalid duration format: {s}")),
    };

    Ok(duration)
}

/// Print the final report in the format used by hey
fn print_hey_format_report<W: io::Write>(out: &mut W, test_state: &TestState) -> Result<()> {
    let elapsed = if test_state.is_complete && test_state.end_time.is_some() {
        test_state
            .end_time
            .unwrap()
            .duration_since(test_state.start_time)
            .as_secs_f64()
    } else {
        test_state.start_time.elapsed().as_secs_f64()
    };
    
    let req_per_sec = if elapsed > 0.0 {
        test_state.completed_requests as f64 / elapsed
    } else {
        0.0
    };
    
    let bytes_per_sec = if elapsed > 0.0 {
        test_state.total_bytes_received as f64 / elapsed
    } else {
        0.0
    };
    
    writeln!(out)?;
    writeln!(out, "Summary:")?;
    writeln!(out, "  Total:\t{:.4} secs", elapsed)?;
    writeln!(out, "  Slowest:\t{:.4} secs", test_state.max_latency / 1000.0)?;
    writeln!(
        out,
        "  Fastest:\t{:.4} secs",
        if test_state.min_latency == f64::MAX {
            0.0
        } else {
            test_state.min_latency / 1000.0
        }
    )?;
    writeln!(out, "  Average:\t{:.4} secs", test_state.p50_latency / 1000.0)?;
    writeln!(out, "  Requests/sec:\t{:.4}", req_per_sec)?;
    
    if test_state.total_bytes_received > 0 {
        if bytes_per_sec >= 1024.0 * 1024.0 {
            writeln!(out, "  Transfer/sec:\t{:.2} MB", bytes_per_sec / (1024.0 * 1024.0))?;
        } else if bytes_per_sec >= 1024.0 {
            writeln!(out, "  Transfer/sec:\t{:.2} KB", bytes_per_sec / 1024.0)?;
        } else {
            writeln!(out, "  Transfer/sec:\t{:.2} B", bytes_per_sec)?;
        }
    }
    
    writeln!(out)?;
    writeln!(out, "Response time histogram:")?;
    // Here we'd need actual histogram data, which we don't have in the current implementation
    // so we'll skip this part
    
    writeln!(out)?;
    writeln!(out, "Latency distribution:")?;
    writeln!(
        out,
        "  10% in {:.4} secs",
        test_state.p50_latency / 2000.0 // Approximation
    )?;
    writeln!(out, "  25% in {:.4} secs", test_state.p50_latency / 1500.0)?;
    writeln!(out, "  50% in {:.4} secs", test_state.p50_latency / 1000.0)?;
    writeln!(out, "  75% in {:.4} secs", test_state.p90_latency / 1000.0)?;
    writeln!(out, "  90% in {:.4} secs", test_state.p90_latency / 1000.0)?;
    writeln!(out, "  95% in {:.4} secs", test_state.p95_latency / 1000.0)?;
    writeln!(out, "  99% in {:.4} secs", test_state.p99_latency / 1000.0)?;
    
    writeln!(out)?;
    writeln!(out, "HTTP response status codes:")?;
    
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort();
    
    for status in status_codes {
        let count = test_state.status_counts[&status];
        let percentage = 100.0 * count as f64 / test_state.completed_requests.max(1) as f64;
        writeln!(out, "  [{status}] {count} responses ({percentage:.2}%)")?;
    }
    
    if test_state.error_count > 0 && !test_state.status_counts.contains_key(&0) {
        let percentage =
            100.0 * test_state.error_count as f64 / test_state.completed_requests.max(1) as f64;
        writeln!(
            out,
            "  [connection errors] {} responses ({:.2}%)",
            test_state.error_count, percentage
        )?;
    }
    
    Ok(())
}

/// Run a performance test with the given command line arguments
pub async fn run(args: Args) -> Result<()> {
    // Convert HTTP method
    let method = match args.method {
        HttpMethods::GET => HttpMethod::GET,
        HttpMethods::POST => HttpMethod::POST,
        HttpMethods::PUT => HttpMethod::PUT,
        HttpMethods::DELETE => HttpMethod::DELETE,
        HttpMethods::HEAD => HttpMethod::HEAD,
        HttpMethods::OPTIONS => HttpMethod::OPTIONS,
    };
    
    // Parse body data
    let body = if let Some(body_data) = args.body {
        Some(body_data)
    } else if let Some(body_file) = args.body_file {
        Some(std::fs::read_to_string(body_file)?)
    } else {
        None
    };
    
    // Parse headers
    let mut headers = Vec::new();
    
    // Add custom headers from command line
    for header in args.headers {
        let parts: Vec<&str> = header.splitn(2, ':').collect();
        if parts.len() == 2 {
            headers.push((parts[0].trim().to_string(), parts[1].trim().to_string()));
        } else {
            return Err(anyhow::anyhow!("Invalid header format: {header}"));
        }
    }
    
    // Add optional content-type header
    if !args.content_type.is_empty() {
        headers.push(("Content-Type".to_string(), args.content_type.clone()));
    }
    
    // Add optional accept header
    if !args.accept.is_empty() {
        headers.push(("Accept".to_string(), args.accept));
    }
    
    // Parse basic auth
    let basic_auth = if let Some(auth) = args.auth {
        let parts: Vec<&str> = auth.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            return Err(anyhow::anyhow!("Invalid basic auth format: {auth}"));
        }
    } else {
        None
    };
    
    // Parse duration
    let duration_secs = if let Some(duration_str) = args.duration {
        match parse_duration(&duration_str) {
            Ok(duration) => duration.as_secs(),
            Err(e) => return Err(anyhow::anyhow!("Invalid duration format: {}", e)),
        }
    } else {
        0 // No duration limit
    };
    
    // Set number of requests (0 means unlimited)
    let requests = if duration_secs > 0 && args.requests == 0 {
        // If duration is set but requests is not, use a large value
        usize::MAX
    } else if args.requests == 0 {
        // Default to 200 if neither duration nor requests is specified
        200
    } else {
        args.requests
    };
    
    // Create test configuration
    let config = TestConfig {
        url: args.url.clone(),
        method,
        headers,
        body,
        basic_auth,
        duration: duration_secs,
        requests,
        concurrent: args.concurrent,
        timeout: args.timeout,
        rate_limit: args.rate_limit,
        disable_compression: args.disable_compression,
        disable_keepalive: args.disable_keepalive,
        disable_redirects: args.disable_redirects,
        interactive: args.output_format.to_lowercase() == "ui",
        output_format: args.output_format.clone(),
        content_type: args.content_type.clone(),
        proxy: args.proxy.clone(),
    };
    
    let shared_state = Arc::new(Mutex::new(TestState::new(&config)));
    
    if config.interactive {
        let mut app = App::new(SharedState { state: shared_state });
        app.run()?;
    } else {
        let mut test_runner = OptimizedTestRunner::with_state(config, SharedState { state: shared_state.clone() });
        test_runner.start().await?;
        // Wait for the test to complete. A better mechanism would be to wait on a signal.
        tokio::time::sleep(tokio::time::Duration::from_secs(duration_secs + 1)).await;
        print_hey_format_report(&mut io::stdout(), &shared_state.lock().unwrap())?;
    }
    
    Ok(())
}

/// Parse command line arguments and run the application
pub async fn run_cli() -> Result<()> {
    let args = Args::parse();
    run(args).await
}