// SPDX-License-Identifier: MIT
//! Connection, protocol and TLS flags.

use std::path::PathBuf;
use std::time::Duration;

use clap::ValueEnum;

use super::parse::{ConnectTo, parse_connect_to, parse_duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HttpVersion {
    #[value(name = "1.1")]
    Http11,
    #[value(name = "2")]
    Http2,
}

#[derive(clap::Args, Debug, Clone)]
#[command(next_help_heading = "Connection")]
pub struct NetworkArgs {
    /// HTTP proxy as host:port or a full URL (http://, socks5://)
    #[arg(short = 'x', long)]
    pub proxy: Option<String>,

    /// HTTP version to use
    #[arg(long = "http-version", value_enum, conflicts_with = "http2")]
    pub http_version: Option<HttpVersion>,

    /// Use HTTP/2 (shorthand for --http-version 2)
    #[arg(long, alias = "h2")]
    pub http2: bool,

    /// Accept invalid TLS certificates and host names
    #[arg(short = 'k', long)]
    pub insecure: bool,

    /// Extra CA certificate (PEM) to trust, in addition to the system store
    #[arg(long)]
    pub cacert: Option<PathBuf>,

    /// Client certificate (PEM) for mutual TLS; requires --key
    #[arg(long, requires = "key")]
    pub cert: Option<PathBuf>,

    /// Client private key (PEM) for mutual TLS; requires --cert
    #[arg(long, requires = "cert")]
    pub key: Option<PathBuf>,

    /// Timeout for establishing a connection, e.g. 5s
    #[arg(long = "connect-timeout", value_parser = parse_duration)]
    pub connect_timeout: Option<Duration>,

    /// Send requests for host:port to target_host:target_port instead, repeatable
    #[arg(long = "connect-to", value_parser = parse_connect_to)]
    pub connect_to: Vec<ConnectTo>,

    /// Resolve host names to IPv4 addresses only
    #[arg(short = '4', long, conflicts_with = "ipv6")]
    pub ipv4: bool,

    /// Resolve host names to IPv6 addresses only
    #[arg(short = '6', long)]
    pub ipv6: bool,

    /// Disable compression
    #[arg(long = "disable-compression")]
    pub disable_compression: bool,

    /// Disable keep-alive: open a new connection for every request
    #[arg(long = "disable-keepalive")]
    pub disable_keepalive: bool,

    /// Do not follow redirects (same as --redirect 0)
    #[arg(long = "disable-redirects")]
    pub disable_redirects: bool,

    /// Maximum number of redirects to follow
    #[arg(short = 'r', long, default_value_t = 10)]
    pub redirect: usize,
}

impl NetworkArgs {
    pub fn http_version(&self) -> HttpVersion {
        if self.http2 {
            HttpVersion::Http2
        } else {
            self.http_version.unwrap_or(HttpVersion::Http11)
        }
    }

    pub fn max_redirects(&self) -> usize {
        if self.disable_redirects {
            0
        } else {
            self.redirect
        }
    }
}
