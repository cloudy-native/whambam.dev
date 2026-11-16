// SPDX-License-Identifier: MIT
//! Build reqwest clients from the test configuration.

use std::sync::{Arc, Once};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::redirect::Policy;
use reqwest::{Certificate, Client, Identity, Proxy};

use super::resolver::{Resolver, SharedResolver};
use crate::cli::HttpVersion;
use crate::config::TestConfig;

/// HTTP/1.1 shares one pooled client. HTTP/2 multiplexes every request over a
/// single connection per client, so each of the `-c` connections gets its own.
pub fn build_clients(config: &TestConfig) -> Result<Vec<Client>> {
    let resolver = SharedResolver(Arc::new(Resolver::new(&config.net)));
    let count = match config.net.http_version {
        HttpVersion::Http2 => config.concurrency,
        HttpVersion::Http11 => 1,
    };
    (0..count)
        .map(|_| build_client(config, resolver.clone()))
        .collect()
}

/// Build a single client (used by --debug).
pub fn build_single_client(config: &TestConfig) -> Result<Client> {
    build_client(config, SharedResolver(Arc::new(Resolver::new(&config.net))))
}

/// Use ring for TLS: it builds on every release target without NASM or CMake.
pub(crate) fn install_crypto_provider() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // Err means another provider is already installed, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn build_client(config: &TestConfig, resolver: SharedResolver) -> Result<Client> {
    install_crypto_provider();
    let net = &config.net;
    let redirect = match net.max_redirects {
        0 => Policy::none(),
        n => Policy::limited(n),
    };
    let mut builder = Client::builder()
        .dns_resolver(Arc::new(resolver))
        .redirect(redirect)
        .gzip(net.compression)
        .brotli(net.compression)
        .deflate(net.compression)
        .zstd(net.compression)
        .tcp_nodelay(true);

    builder = match &net.proxy {
        Some(proxy) => builder.proxy(Proxy::all(proxy).context("invalid proxy")?),
        None => builder.no_proxy(),
    };
    if net.insecure {
        builder = builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
    }
    if let Some(pem) = &net.cacert {
        for cert in Certificate::from_pem_bundle(pem).context("invalid --cacert PEM")? {
            builder = builder.add_root_certificate(cert);
        }
    }
    if let Some(pem) = &net.identity {
        let identity = Identity::from_pem(pem).context("invalid --cert/--key PEM")?;
        builder = builder.identity(identity);
    }
    if let Some(timeout) = net.connect_timeout {
        builder = builder.connect_timeout(timeout);
    }
    builder = match net.http_version {
        HttpVersion::Http11 => builder.http1_only(),
        HttpVersion::Http2 => builder.http2_prior_knowledge(),
    };
    builder = if net.keepalive {
        builder
            .pool_max_idle_per_host(config.workers())
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(60))
    } else {
        builder.pool_max_idle_per_host(0)
    };
    builder.build().context("failed to build HTTP client")
}
