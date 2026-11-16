// SPDX-License-Identifier: MIT
//! Connection settings resolved from the command line.

use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::cli::parse::ConnectTo;
use crate::cli::{HttpVersion, NetworkArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpFamily {
    V4,
    V6,
}

#[derive(Debug, Clone)]
pub struct NetConfig {
    pub proxy: Option<String>,
    pub http_version: HttpVersion,
    pub insecure: bool,
    /// PEM bytes of an extra CA certificate.
    pub cacert: Option<Vec<u8>>,
    /// PEM bytes of a client certificate followed by its key.
    pub identity: Option<Vec<u8>>,
    pub connect_timeout: Option<Duration>,
    pub connect_to: Vec<ConnectTo>,
    pub ip_family: Option<IpFamily>,
    pub compression: bool,
    pub keepalive: bool,
    pub max_redirects: usize,
}

impl NetConfig {
    pub fn from_cli(args: &NetworkArgs) -> Result<Self> {
        let identity = match (&args.cert, &args.key) {
            (Some(cert), Some(key)) => {
                let mut pem = read_pem(cert)?;
                pem.push(b'\n');
                pem.extend(read_pem(key)?);
                Some(pem)
            }
            _ => None,
        };
        let ip_family = match (args.ipv4, args.ipv6) {
            (true, _) => Some(IpFamily::V4),
            (_, true) => Some(IpFamily::V6),
            _ => None,
        };
        Ok(NetConfig {
            proxy: args.proxy.as_deref().map(normalize_proxy),
            http_version: args.http_version(),
            insecure: args.insecure,
            cacert: args.cacert.as_deref().map(read_pem).transpose()?,
            identity,
            connect_timeout: args.connect_timeout,
            connect_to: args.connect_to.clone(),
            ip_family,
            compression: !args.disable_compression,
            keepalive: !args.disable_keepalive,
            max_redirects: args.max_redirects(),
        })
    }
}

fn read_pem(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("failed to read {}", path.display()))
}

/// hey takes `host:port`; also accept full proxy URLs.
fn normalize_proxy(proxy: &str) -> String {
    if proxy.contains("://") {
        proxy.to_string()
    } else {
        format!("http://{proxy}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_gets_default_scheme() {
        assert_eq!(normalize_proxy("127.0.0.1:3128"), "http://127.0.0.1:3128");
        assert_eq!(normalize_proxy("socks5://h:1080"), "socks5://h:1080");
    }

    #[test]
    fn missing_pem_is_an_error() {
        assert!(read_pem(Path::new("/definitely/not/here.pem")).is_err());
    }
}
