// SPDX-License-Identifier: MIT
//! Flags that shape each HTTP request.

use std::path::PathBuf;

use reqwest::Method;

use super::parse::{parse_basic_auth, parse_header, parse_method};

#[derive(clap::Args, Debug, Clone)]
#[command(next_help_heading = "Request")]
pub struct RequestArgs {
    /// HTTP method, e.g. GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS
    #[arg(short = 'm', long, default_value = "GET", value_parser = parse_method)]
    pub method: Method,

    /// Custom header, repeatable: -H "Accept: text/html" -H "X-Id: 1"
    #[arg(short = 'H', long = "header", value_parser = parse_header)]
    pub headers: Vec<(String, String)>,

    /// Request body
    #[arg(short = 'd', long, conflicts_with = "body_file")]
    pub body: Option<String>,

    /// Request body read from a file (binary safe)
    #[arg(short = 'D', long = "body-file")]
    pub body_file: Option<PathBuf>,

    /// Accept header
    #[arg(short = 'A', long)]
    pub accept: Option<String>,

    /// Content-Type header, sent when there is a body
    #[arg(short = 'T', long = "content-type", default_value = "text/html")]
    pub content_type: String,

    /// Basic authentication as username:password
    #[arg(short = 'a', long = "auth", value_parser = parse_basic_auth)]
    pub basic_auth: Option<(String, String)>,

    /// Override the Host header
    #[arg(long)]
    pub host: Option<String>,

    /// User-Agent header [default: whambam/<version>]
    #[arg(short = 'U', long = "user-agent")]
    pub user_agent: Option<String>,
}
