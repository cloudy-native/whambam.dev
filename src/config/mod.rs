// SPDX-License-Identifier: MIT
//! Validated test configuration built from the command line.

mod network;
mod request;

use std::time::Duration;

use anyhow::{Result, bail};

use crate::cli::{Cli, HttpVersion};

pub use network::{IpFamily, NetConfig};
pub use request::RequestSpec;

/// How request starts are paced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pacing {
    /// Send as fast as workers allow.
    Unlimited,
    /// hey semantics: each worker sends at most this many requests per second.
    PerWorker(f64),
    /// Open-model total rate; latency is measured from each request's due time.
    Global(f64),
}

impl Pacing {
    fn from_cli(rate_limit: f64, qps: Option<f64>) -> Result<Self> {
        match qps {
            Some(q) if !(q.is_finite() && q > 0.0) => bail!("--qps must be greater than 0"),
            Some(q) => Ok(Pacing::Global(q)),
            None if !(rate_limit.is_finite() && rate_limit >= 0.0) => {
                bail!("-q must be zero or positive")
            }
            None if rate_limit > 0.0 => Ok(Pacing::PerWorker(rate_limit)),
            None => Ok(Pacing::Unlimited),
        }
    }

    /// True when latencies include time spent waiting for a free worker.
    pub fn is_corrected(&self) -> bool {
        matches!(self, Pacing::Global(_))
    }
}

/// Everything the engine needs to run one test.
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub request: RequestSpec,
    /// Request budget; `None` means run until the duration ends or the user quits.
    pub requests: Option<usize>,
    pub duration: Option<Duration>,
    pub concurrency: usize,
    pub http2_parallel: usize,
    pub pacing: Pacing,
    pub timeout: Option<Duration>,
    pub net: NetConfig,
}

impl TestConfig {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let load = &cli.load;
        if load.concurrency == 0 {
            bail!("-c must be at least 1");
        }
        if load.http2_parallel == 0 {
            bail!("-p must be at least 1");
        }
        let net = NetConfig::from_cli(&cli.network)?;
        if load.http2_parallel > 1 && net.http_version != HttpVersion::Http2 {
            bail!("-p requires --http2");
        }
        let request = RequestSpec::from_cli(&cli.url, &cli.request, &net)?;
        let duration = load.duration.filter(|d| !d.is_zero());
        let requests = match (duration, load.requests) {
            (Some(_), _) | (None, 0) => None,
            (None, n) => Some(n),
        };
        Ok(TestConfig {
            request,
            requests,
            duration,
            concurrency: load.concurrency,
            http2_parallel: load.http2_parallel,
            pacing: Pacing::from_cli(load.rate_limit, load.qps)?,
            timeout: Some(load.timeout).filter(|t| !t.is_zero()),
            net,
        })
    }

    /// Total number of request workers.
    pub fn workers(&self) -> usize {
        self.concurrency * self.http2_parallel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn config(args: &[&str]) -> Result<TestConfig> {
        let cli = Cli::try_parse_from(std::iter::once("whambam").chain(args.iter().copied()))?;
        TestConfig::from_cli(&cli)
    }

    #[test]
    fn duration_overrides_request_count() {
        let c = config(&["http://x", "-n", "5", "-z", "2s"]).unwrap();
        assert_eq!(c.requests, None);
        assert_eq!(c.duration, Some(Duration::from_secs(2)));
        assert_eq!(config(&["http://x", "-n", "0"]).unwrap().requests, None);
        assert_eq!(config(&["http://x", "-n", "7"]).unwrap().requests, Some(7));
    }

    #[test]
    fn pacing_is_selected_from_flags() {
        assert_eq!(config(&["http://x"]).unwrap().pacing, Pacing::Unlimited);
        assert_eq!(
            config(&["http://x", "-q", "3"]).unwrap().pacing,
            Pacing::PerWorker(3.0)
        );
        let global = config(&["http://x", "--qps", "100"]).unwrap().pacing;
        assert_eq!(global, Pacing::Global(100.0));
        assert!(global.is_corrected());
        assert!(config(&["http://x", "--qps", "0"]).is_err());
    }

    #[test]
    fn rejects_invalid_combinations() {
        assert!(config(&["http://x", "-c", "0"]).is_err());
        assert!(config(&["http://x", "-p", "4"]).is_err());
        assert!(config(&["not a url"]).is_err());
        assert!(config(&["ftp://x"]).is_err());
        let c = config(&["http://x", "--http2", "-p", "4", "-c", "2"]).unwrap();
        assert_eq!(c.workers(), 8);
    }

    #[test]
    fn zero_timeout_means_none() {
        assert_eq!(config(&["http://x", "-t", "0"]).unwrap().timeout, None);
        assert_eq!(
            config(&["http://x", "-t", "500ms"]).unwrap().timeout,
            Some(Duration::from_millis(500))
        );
    }
}
