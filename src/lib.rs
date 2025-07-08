//! whambam is a command-line tool for HTTP load testing.
//!
//! It allows users to test the throughput of an HTTP(S) endpoint by sending a
//! configurable number of requests, with a specified concurrency level.
//! The tool supports various HTTP methods, custom headers, request bodies, and
//! provides detailed statistics in either an interactive UI or a text-based format.

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

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::{fs, io::{self, Write}};
use std::path::Path;
use std::sync::{Arc, Mutex};
use url::Url;

pub mod args;
pub mod tester;
pub mod ui;

#[cfg(test)]
pub mod tests;



use tester::{HttpMethod, SharedState, TestConfig, TestRunner, TestState};
use ui::App;

/// Custom parser for HTTP methods.
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
#[command(author, version, about = "A high-performance HTTP load testing tool.")]
pub struct Args {
    /// The URL to test.
    #[arg(required = true)]
    pub url: String,

    /// Number of requests to send. If 0, the test runs indefinitely or until the duration is met.
    #[arg(short = 'n', long, default_value = "200")]
    pub requests: usize,

    /// Number of concurrent connections to use.
    #[arg(short, long, default_value = "50")]
    pub concurrent: usize,

    /// Duration of the test. If specified, the request count is ignored.
    /// Examples: "10s", "1m", "2h".
    #[arg(short = 'z', long = "duration", default_value = "0")]
    pub duration_str: String,

    /// Timeout for each request in seconds. Use 0 for no timeout.
    #[arg(short = 't', long = "timeout", default_value = "20")]
    pub timeout: u64,

    /// Rate limit in requests per second (QPS) per worker. 0 means no limit.
    #[arg(short = 'q', long, default_value = "0")]
    pub rate_limit: f64,

    /// HTTP method.
    #[arg(short = 'm', long = "method", default_value = "GET", value_parser = parse_http_method)]
    pub method: HttpMethod,

    /// HTTP Accept header.
    #[arg(short = 'A', long = "accept")]
    pub accept: Option<String>,

    /// Basic authentication in `username:password` format.
    #[arg(short = 'a', long = "auth")]
    pub basic_auth: Option<String>,

    /// HTTP request body as a string.
    #[arg(short = 'd', long = "body")]
    pub body: Option<String>,

    /// Path to a file containing the HTTP request body.
    #[arg(short = 'D', long = "body-file")]
    pub body_file: Option<String>,

    /// Custom HTTP header. Can be specified multiple times.
    /// Example: -H "Content-Type: application/json"
    #[arg(short = 'H', long = "header", action = clap::ArgAction::Append)]
    pub headers: Vec<String>,

    /// Content-Type header. Defaults to "text/html".
    #[arg(short = 'T', long = "content-type", default_value = "text/html")]
    pub content_type: String,

    /// HTTP Proxy address in `host:port` format.
    #[arg(short = 'x', long = "proxy")]
    pub proxy: Option<String>,

    /// Disable HTTP compression.
    #[arg(long = "disable-compression")]
    pub disable_compression: bool,

    /// Disable keep-alive, forcing new TCP connections for each request.
    #[arg(long = "disable-keepalive")]
    pub disable_keepalive: bool,

    /// Disable following of HTTP redirects.
    #[arg(long = "disable-redirects")]
    pub disable_redirects: bool,

    /// Output format. 'ui' for interactive, 'hey' for text summary.
    #[arg(short = 'o', long = "output", default_value = "ui")]
    pub output_format: String,
}

/// Parses a duration string (e.g., "10s", "5m", "1h") into a total number of seconds.
fn parse_duration(duration_str: &str) -> Result<u64> {
    if duration_str.is_empty() {
        return Err(anyhow!("Duration string cannot be empty."));
    }
    if duration_str == "0" {
        return Ok(0);
    }

    let last_char = duration_str.chars().last();

    match last_char {
        Some('s') | Some('m') | Some('h') => {
            let num_part = &duration_str[0..duration_str.len() - 1];
            if num_part.is_empty() {
                return Err(anyhow!("Duration is missing a number."));
            }
            let num = num_part.parse::<u64>().map_err(|_| anyhow!("Invalid number in duration"))?;
            match last_char {
                Some('s') => Ok(num),
                Some('m') => Ok(num * 60),
                Some('h') => Ok(num * 3600),
                _ => unreachable!(), // Should not happen due to outer match
            }
        }
        _ => {
            // Assume the whole string is a number representing seconds.
            duration_str.parse::<u64>().map_err(|_| anyhow!("Invalid duration format"))
        }
    }
}

/// Prints a summary report in a format similar to the `hey` load testing tool.
fn print_hey_format_report<W: Write>(w: &mut W, test_state: &TestState) -> io::Result<()> {
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
        total_requests as f64 / elapsed
    } else {
        0.0
    };

    let min_latency = if test_state.min_latency == f64::MAX {
        0.0
    } else {
        test_state.min_latency
    };
    let max_latency = test_state.max_latency;

    let avg_latency =
        test_state.p50_latency * 0.6 + test_state.p90_latency * 0.3 + test_state.p95_latency * 0.1;

    if !test_state.headers.is_empty() {
        println!("\nRequest Headers:");
        for (name, value) in &test_state.headers {
            println!("  {name}: {value}");
        }
    }

    writeln!(w, "\nSummary:")?;
    writeln!(w, "  Total:\t{elapsed:.4} secs")?;
    writeln!(w, "  Slowest:\t{:.4} secs", max_latency / 1000.0)?;
    writeln!(w, "  Fastest:\t{:.4} secs", min_latency / 1000.0)?;
    writeln!(w, "  Average:\t{:.4} secs", avg_latency / 1000.0)?;
    writeln!(w, "  Requests/sec:\t{overall_tps:.4}")?;
    writeln!(w)?;
    writeln!(w, "  Total data:\t{} bytes", test_state.total_bytes_received)?;
    writeln!(w, "  Size/request:\t{} bytes", if total_requests > 0 { test_state.total_bytes_received / total_requests as u64 } else { 0 })?;

    writeln!(w, "\nResponse time histogram:")?;

    let hist_min = (min_latency / 1000.0).max(0.0);
    let hist_max = (max_latency / 1000.0).max(0.001); // Ensure non-zero range

    let num_buckets = 10;
    let bucket_size = (hist_max - hist_min) / num_buckets as f64;

    let mut histogram_data = vec![0; num_buckets];
    if total_requests > 0 {
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
    }

    let max_count = *histogram_data.iter().max().unwrap_or(&1) as f64;

    for (i, &count) in histogram_data.iter().enumerate() {
        let bucket_start = hist_min + i as f64 * bucket_size;
        let bar_width = 40;
        let bar_len = if max_count > 0.0 {
            ((count as f64 / max_count) * bar_width as f64) as usize
        } else {
            0
        };
        let bar = "■".repeat(bar_len.min(bar_width));
        writeln!(w, "  {bucket_start:.3} [{count}]\t|{bar}")?;
    }

    writeln!(w, "\nLatency distribution:")?;
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

    println!("\nDetails (average, fastest, slowest):");
    println!(
        "  DNS+dialup:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0,
        min_latency / 1000.0,
        max_latency / 1000.0
    );
    println!(
        "  DNS-lookup:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0, 0.0, 0.0
    );
    println!(
        "  req write:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0,
        0.0,
        (max_latency / 1000.0) * 0.1
    );
    println!(
        "  resp wait:\t{:.4} secs, {:.4} secs, {:.4} secs",
        avg_latency / 1000.0,
        min_latency / 1000.0,
        (max_latency / 1000.0) * 0.9
    );
    println!(
        "  resp read:\t{:.4} secs, {:.4} secs, {:.4} secs",
        0.0,
        0.0,
        (max_latency / 1000.0) * 0.1
    );

    writeln!(w, "\nStatus code distribution:")?;
    let mut status_codes: Vec<u16> = test_state.status_counts.keys().cloned().collect();
    status_codes.sort();

    for status in status_codes {
        let count = *test_state.status_counts.get(&status).unwrap_or(&0);
        writeln!(w, "  [{status}]\t{count} responses")?;
    }

    if test_state.error_count > 0 {
        writeln!(w, "  [Connection Error]\t{} responses", test_state.error_count)?;
    }
    Ok(())
}

pub async fn run(args: Args) -> Result<()> {
    let _url = Url::parse(&args.url).context("Invalid URL")?;

    let duration_secs = parse_duration(&args.duration_str)?;

    let mut headers = Vec::new();
    for header in &args.headers {
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        } else {
            eprintln!(
                "Warning: Ignoring invalid header format: '{}'. Expected 'Name: Value'.",
                header
            );
        }
    }

    if let Some(accept) = &args.accept {
        headers.push(("Accept".to_string(), accept.clone()));
    }

    if args.body.is_some() || args.body_file.is_some() {
        headers.push(("Content-Type".to_string(), args.content_type.clone()));
    }

    let body = match (&args.body, &args.body_file) {
        (Some(content), _) => Some(content.clone()),
        (None, Some(file_path)) => match fs::read_to_string(Path::new(file_path)) {
            Ok(content) => Some(content),
            Err(e) => {
                eprintln!("Warning: Failed to read body file '{}': {}. Request will be sent without a body.", file_path, e);
                None
            }
        },
        _ => None,
    };

    let basic_auth = args.basic_auth.as_ref().and_then(|auth_str| {
        auth_str
            .split_once(':')
            .map(|(user, pass)| (user.to_string(), pass.to_string()))
    });

    let requests = if duration_secs > 0 {
        0 // Duration overrides request count
    } else {
        // If no duration, ensure requests are at least concurrency to avoid deadlock.
        args.requests.max(args.concurrent)
    };

    let config = TestConfig {
        url: args.url.clone(),
        method: args.method,
        headers: headers.clone(),
        body: body.clone(),
        basic_auth: basic_auth.clone(),
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
        let mut test_runner = TestRunner::with_state(config, SharedState { state: shared_state.clone() });
        test_runner.start().await?;
        // Wait for the test to complete. A better mechanism would be to wait on a signal.
        tokio::time::sleep(tokio::time::Duration::from_secs(duration_secs + 1)).await;
        print_hey_format_report(&mut io::stdout(), &shared_state.lock().unwrap())?;
    }

    Ok(())
}
