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
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use url::Url;

pub mod args;
pub mod tester;
pub mod ui;

#[cfg(test)]
pub mod tests;

use tester::{
    HttpMethod, SharedMetrics, SharedState, TestConfig, TestState, UnifiedRunner as TestRunner,
};
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

    /// Disable interactive UI. When specified, the command will exit with an error.
    #[arg(long = "no-ui", default_value = "false")]
    pub no_ui: bool,
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
            let num = num_part
                .parse::<u64>()
                .map_err(|_| anyhow!("Invalid number in duration"))?;
            match last_char {
                Some('s') => Ok(num),
                Some('m') => Ok(num * 60),
                Some('h') => Ok(num * 3600),
                _ => unreachable!(), // Should not happen due to outer match
            }
        }
        _ => {
            // Assume the whole string is a number representing seconds.
            duration_str
                .parse::<u64>()
                .map_err(|_| anyhow!("Invalid duration format"))
        }
    }
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
        interactive: !args.no_ui,
        output_format: String::new(), // Deprecated field
        content_type: args.content_type.clone(),
        proxy: args.proxy.clone(),
    };

    let shared_state = Arc::new(Mutex::new(TestState::new(&config)));

    // Only interactive UI mode is supported
    if !args.no_ui {
        let mut app = App::new(SharedState {
            state: shared_state,
        });
        app.run()?;
    } else {
        // Non-UI mode is not supported
        println!("The --no-ui option is currently not supported.");
        println!("The UI interface is required for this version.");
        return Err(anyhow!("UI mode is required for this version"));
    }

    Ok(())
}
