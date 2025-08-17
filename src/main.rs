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

use tester::{HttpMethod, SharedState, TestConfig, TestState, UnifiedRunner as TestRunner};
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

    /// Interactive UI for real-time display of test results
    #[arg(long = "no-ui", default_value = "false")]
    no_ui: bool,
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
    #[allow(deprecated)]
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
        interactive: !args.no_ui,
        output_format: String::new(), // No longer used
    };

    // Run in interactive mode unless --no-ui is specified
    if !args.no_ui {
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
    } else {
        // Non-UI mode - just print a message and exit
        println!("The --no-ui option is currently not supported.");
        println!("The UI interface is required for this version.");
        return Ok(());
    }

    Ok(())
}
