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

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HttpMethods {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
}

impl std::fmt::Display for HttpMethods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GET => write!(f, "GET"),
            Self::POST => write!(f, "POST"),
            Self::PUT => write!(f, "PUT"),
            Self::DELETE => write!(f, "DELETE"),
            Self::HEAD => write!(f, "HEAD"),
            Self::OPTIONS => write!(f, "OPTIONS"),
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about,
    long_about = "A modern HTTP load testing tool with an interactive UI"
)]
pub struct Args {
    /// URL to test
    pub url: String,

    /// Number of requests to send (0 for unlimited)
    #[arg(short = 'n', long, default_value = "0")]
    pub requests: usize,

    /// Number of concurrent connections
    #[arg(short = 'c', long, default_value = "50")]
    pub concurrent: usize,

    /// Maximum test duration (e.g., 10s, 1m, 1h)
    #[arg(short = 'z', long)]
    pub duration: Option<String>,

    /// Request timeout in seconds
    #[arg(short = 't', long, default_value = "20")]
    pub timeout: u64,

    /// Rate limit in queries per second (QPS)
    #[arg(short = 'q', long, default_value = "0")]
    pub rate_limit: f64,

    /// HTTP method
    #[arg(short = 'm', long, default_value = "get")]
    pub method: HttpMethods,

    /// Custom headers (can be used multiple times)
    #[arg(short = 'H', long)]
    pub headers: Vec<String>,

    /// Request body data as string
    #[arg(short = 'd', long)]
    pub body: Option<String>,

    /// Request body from file
    #[arg(short = 'D', long)]
    pub body_file: Option<PathBuf>,

    /// Accept header
    #[arg(short = 'A', long, default_value = "")]
    pub accept: String,

    /// Content-Type header
    #[arg(short = 'T', long, default_value = "text/html")]
    pub content_type: String,

    /// Basic authentication (username:password)
    #[arg(short = 'a', long)]
    pub auth: Option<String>,

    /// HTTP proxy address (host:port)
    #[arg(short = 'x', long)]
    pub proxy: Option<String>,

    /// Disable compression
    #[arg(long, default_value = "false")]
    pub disable_compression: bool,

    /// Disable keep-alive
    #[arg(long, default_value = "false")]
    pub disable_keepalive: bool,

    /// Disable following redirects
    #[arg(long, default_value = "false")]
    pub disable_redirects: bool,

    /// Output format (ui or hey)
    #[arg(short = 'o', long, default_value = "ui")]
    pub output_format: String,
}
