// SPDX-License-Identifier: MIT
//! `--debug`: send one request and print what went over the wire.

use std::io::Write;
use std::time::Instant;

use anyhow::Result;

use super::client::build_single_client;
use super::errors::error_chain;
use super::execute::build_request;
use crate::cli::HttpVersion;
use crate::config::TestConfig;

const BODY_PREVIEW_LIMIT: usize = 64 * 1024;

pub async fn run_debug<W: Write>(config: &TestConfig, out: &mut W) -> Result<()> {
    let client = build_single_client(config)?;
    let request = build_request(&client, &config.request, config.timeout).build()?;

    writeln!(
        out,
        "> {} {} {}",
        request.method(),
        request.url(),
        match config.net.http_version {
            HttpVersion::Http11 => "HTTP/1.1",
            HttpVersion::Http2 => "HTTP/2.0",
        }
    )?;
    for (name, value) in request.headers() {
        writeln!(
            out,
            "> {}: {}",
            name,
            String::from_utf8_lossy(value.as_bytes())
        )?;
    }
    writeln!(out, ">")?;
    if let Some(body) = request.body().and_then(|b| b.as_bytes()) {
        write_body(out, "> ", body)?;
    }

    let started = Instant::now();
    match client.execute(request).await {
        Ok(resp) => {
            writeln!(out, "< {:?} {}", resp.version(), resp.status())?;
            for (name, value) in resp.headers() {
                writeln!(
                    out,
                    "< {}: {}",
                    name,
                    String::from_utf8_lossy(value.as_bytes())
                )?;
            }
            writeln!(out, "<")?;
            let body = resp.bytes().await?;
            let elapsed = started.elapsed();
            write_body(out, "", &body)?;
            writeln!(
                out,
                "\n* {} bytes in {:.2} ms",
                body.len(),
                elapsed.as_secs_f64() * 1e3
            )?;
        }
        Err(e) => writeln!(out, "* request failed: {}", error_chain(&e))?,
    }
    Ok(())
}

fn write_body<W: Write>(out: &mut W, prefix: &str, body: &[u8]) -> Result<()> {
    let shown = &body[..body.len().min(BODY_PREVIEW_LIMIT)];
    for line in String::from_utf8_lossy(shown).lines() {
        writeln!(out, "{prefix}{line}")?;
    }
    if body.len() > BODY_PREVIEW_LIMIT {
        writeln!(
            out,
            "{prefix}… ({} more bytes)",
            body.len() - BODY_PREVIEW_LIMIT
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;
    use test_utils::MockServer;

    #[tokio::test]
    async fn prints_request_and_response() {
        let server = MockServer::start().await;
        let cli =
            Cli::try_parse_from(["whambam", &server.url(), "-m", "PUT", "-d", "ping"]).unwrap();
        let config = TestConfig::from_cli(&cli).unwrap();
        let mut out = Vec::new();
        run_debug(&config, &mut out).await.unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("> PUT http://127.0.0.1"));
        assert!(text.contains("> ping"));
        assert!(text.contains("< HTTP/1.1 200 OK"));
        assert!(text.contains("Hello, World!"));
        assert_eq!(server.request_count(), 1);
    }

    #[tokio::test]
    async fn reports_failures_without_erroring() {
        let cli = Cli::try_parse_from(["whambam", "http://127.0.0.1:1", "-t", "1s"]).unwrap();
        let config = TestConfig::from_cli(&cli).unwrap();
        let mut out = Vec::new();
        run_debug(&config, &mut out).await.unwrap();
        assert!(String::from_utf8(out).unwrap().contains("* request failed"));
    }
}
