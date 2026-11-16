// SPDX-License-Identifier: MIT
//! Command-line interface. Flags follow hey where hey has them.

mod network;
mod output;
pub mod parse;
mod request;

use std::time::Duration;

use clap::Parser;

pub use network::{HttpVersion, NetworkArgs};
pub use output::{OutputArgs, OutputFormat};
pub use request::RequestArgs;

use parse::{parse_count, parse_duration};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "whambam",
    author,
    version,
    about = "Fast HTTP(S) load testing with a live terminal UI and hey-compatible flags",
    after_help = "Examples:\n  whambam https://example.com\n  whambam https://example.com -n 10k -c 100\n  whambam https://example.com -z 30s --qps 500\n  whambam https://api.example.com -m POST -d '{\"a\":1}' -T application/json\n  whambam https://example.com -z 10s --no-ui -o json --output report.json"
)]
pub struct Cli {
    /// Target URL
    pub url: String,

    #[command(flatten)]
    pub load: LoadArgs,

    #[command(flatten)]
    pub request: RequestArgs,

    #[command(flatten)]
    pub network: NetworkArgs,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(clap::Args, Debug, Clone)]
#[command(next_help_heading = "Load")]
pub struct LoadArgs {
    /// Number of requests to run. Accepts suffixes: 10k, 1m. Ignored when -z is set.
    #[arg(short = 'n', long, default_value = "200", value_parser = parse_count)]
    pub requests: usize,

    /// Number of workers (concurrent connections) to run
    #[arg(short = 'c', long, alias = "concurrent", default_value_t = 50)]
    pub concurrency: usize,

    /// Run for this long instead of a request count, e.g. 30s, 5m, 1h
    #[arg(short = 'z', long, value_parser = parse_duration)]
    pub duration: Option<Duration>,

    /// Rate limit in requests per second PER WORKER, like hey (0 = unlimited)
    #[arg(short = 'q', long = "rate-limit", default_value_t = 0.0)]
    pub rate_limit: f64,

    /// Total request rate across all workers. Latencies are corrected for
    /// coordinated omission: they are measured from when each request was due.
    #[arg(long, conflicts_with = "rate_limit")]
    pub qps: Option<f64>,

    /// Timeout for each request, e.g. 20, 500ms, 1m (0 = no timeout)
    #[arg(short = 't', long, default_value = "20", value_parser = parse_duration)]
    pub timeout: Duration,

    /// HTTP/2 streams per connection; runs concurrency × parallel workers
    #[arg(short = 'p', long = "http2-parallel", default_value_t = 1)]
    pub http2_parallel: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("whambam").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_match_hey() {
        let cli = parse(&["http://localhost"]).unwrap();
        assert_eq!(cli.load.requests, 200);
        assert_eq!(cli.load.concurrency, 50);
        assert_eq!(cli.load.timeout, Duration::from_secs(20));
        assert_eq!(cli.load.rate_limit, 0.0);
        assert!(cli.load.qps.is_none());
        assert!(cli.load.duration.is_none());
        assert_eq!(cli.request.content_type, "text/html");
        assert_eq!(cli.network.redirect, 10);
        assert_eq!(cli.output.output_format, OutputFormat::Text);
        assert!(!cli.output.no_ui);
    }

    #[test]
    fn hey_style_flags_parse() {
        let cli = parse(&[
            "http://localhost",
            "-n",
            "10k",
            "-c",
            "8",
            "-z",
            "1m",
            "-q",
            "5",
            "-m",
            "post",
            "-H",
            "A: b",
            "-H",
            "C: d",
            "-d",
            "body",
            "-A",
            "text/plain",
            "-T",
            "application/json",
            "-a",
            "u:p",
            "-x",
            "127.0.0.1:3128",
            "-o",
            "csv",
            "--disable-keepalive",
            "--disable-compression",
            "--disable-redirects",
            "--no-ui",
        ])
        .unwrap();
        assert_eq!(cli.load.requests, 10_000);
        assert_eq!(cli.load.concurrency, 8);
        assert_eq!(cli.load.duration, Some(Duration::from_secs(60)));
        assert_eq!(cli.load.rate_limit, 5.0);
        assert_eq!(cli.request.method.as_str(), "POST");
        assert_eq!(cli.request.headers.len(), 2);
        assert_eq!(cli.output.output_format, OutputFormat::Csv);
        assert!(cli.network.disable_keepalive && cli.network.disable_redirects);
        assert!(cli.output.no_ui);
    }

    #[test]
    fn conflicting_flags_are_rejected() {
        assert!(parse(&["http://x", "-q", "1", "--qps", "10"]).is_err());
        assert!(parse(&["http://x", "-d", "a", "-D", "file"]).is_err());
        assert!(parse(&["http://x", "--ipv4", "--ipv6"]).is_err());
        assert!(parse(&["http://x", "--cert", "c.pem"]).is_err());
    }

    #[test]
    fn invalid_values_are_rejected() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["http://x", "-c", "-1"]).is_err());
        assert!(parse(&["http://x", "-n", "lots"]).is_err());
        assert!(parse(&["http://x", "-z", "10x"]).is_err());
        assert!(parse(&["http://x", "-H", "nocolon"]).is_err());
        assert!(parse(&["http://x", "-m", "NOT VALID"]).is_err());
        assert!(parse(&["http://x", "-o", "xml"]).is_err());
    }

    #[test]
    fn aliases_work() {
        let cli = parse(&["http://x", "--concurrent", "3", "--no-tui", "--h2"]).unwrap();
        assert_eq!(cli.load.concurrency, 3);
        assert!(cli.output.no_ui);
        assert!(cli.network.http2);
    }
}
