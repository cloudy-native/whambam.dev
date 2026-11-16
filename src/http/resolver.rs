// SPDX-License-Identifier: MIT
//! DNS resolver with --connect-to overrides, IPv4/IPv6 filtering and a
//! per-run cache so repeated lookups do not distort latency.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::config::{IpFamily, NetConfig};

#[derive(Debug, Default)]
pub struct Resolver {
    family: Option<IpFamily>,
    /// Lower-case host → target host to resolve instead.
    overrides: HashMap<String, String>,
    cache: Arc<Mutex<HashMap<String, Vec<SocketAddr>>>>,
}

impl Resolver {
    pub fn new(net: &NetConfig) -> Self {
        let overrides = net
            .connect_to
            .iter()
            .map(|r| (r.host.to_ascii_lowercase(), r.target_host.clone()))
            .collect();
        Resolver {
            family: net.ip_family,
            overrides,
            cache: Arc::default(),
        }
    }

    async fn lookup(&self, host: String) -> io::Result<Vec<SocketAddr>> {
        if let Some(hit) = self.cache.lock().ok().and_then(|c| c.get(&host).cloned()) {
            return Ok(hit);
        }
        let target = self
            .overrides
            .get(&host)
            .cloned()
            .unwrap_or_else(|| host.clone());
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((target.as_str(), 0))
            .await?
            .filter(|a| family_matches(self.family, a))
            .collect();
        if addrs.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("dns error: no addresses of the requested IP family for {target}"),
            ));
        }
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(host, addrs.clone());
        }
        Ok(addrs)
    }
}

fn family_matches(family: Option<IpFamily>, addr: &SocketAddr) -> bool {
    match family {
        Some(IpFamily::V4) => addr.is_ipv4(),
        Some(IpFamily::V6) => addr.is_ipv6(),
        None => true,
    }
}

/// Shared handle so all clients in a run use one cache.
#[derive(Debug, Clone)]
pub struct SharedResolver(pub Arc<Resolver>);

impl Resolve for SharedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = Arc::clone(&self.0);
        let host = name.as_str().to_ascii_lowercase();
        Box::pin(async move {
            let addrs = resolver.lookup(host).await?;
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parse::parse_connect_to;

    fn net(family: Option<IpFamily>, rules: &[&str]) -> NetConfig {
        let args = <crate::cli::Cli as clap::Parser>::try_parse_from(["whambam", "http://x"])
            .unwrap()
            .network;
        let mut net = NetConfig::from_cli(&args).unwrap();
        net.ip_family = family;
        net.connect_to = rules.iter().map(|r| parse_connect_to(r).unwrap()).collect();
        net
    }

    #[tokio::test]
    async fn override_redirects_lookup() {
        let r = Resolver::new(&net(None, &["example.invalid:80:127.0.0.1:80"]));
        let addrs = r.lookup("example.invalid".into()).await.unwrap();
        assert_eq!(addrs[0].ip().to_string(), "127.0.0.1");
        // Second lookup is served from the cache.
        assert_eq!(r.lookup("example.invalid".into()).await.unwrap(), addrs);
    }

    #[tokio::test]
    async fn family_filter_can_reject_all() {
        let r = Resolver::new(&net(
            Some(IpFamily::V6),
            &["v4only.invalid:80:127.0.0.1:80"],
        ));
        let err = r.lookup("v4only.invalid".into()).await.unwrap_err();
        assert!(err.to_string().contains("dns error"));
    }
}
