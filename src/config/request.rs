// SPDX-License-Identifier: MIT
//! The request every worker sends, resolved once up front.

use std::fs;

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use reqwest::Method;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HOST, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use url::Url;

use super::NetConfig;
use crate::cli::RequestArgs;

#[derive(Debug, Clone)]
pub struct RequestSpec {
    /// URL to connect to (port may be rewritten by --connect-to).
    pub url: Url,
    /// URL as the user typed it, for display.
    pub display_url: String,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Option<Bytes>,
    pub basic_auth: Option<(String, String)>,
}

impl RequestSpec {
    pub fn from_cli(raw_url: &str, args: &RequestArgs, net: &NetConfig) -> Result<Self> {
        let mut url = Url::parse(raw_url).with_context(|| format!("invalid URL '{raw_url}'"))?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!(
                "unsupported URL scheme '{}': use http or https",
                url.scheme()
            );
        }
        if url.host_str().is_none() {
            bail!("URL '{raw_url}' has no host");
        }
        let body = load_body(args)?;
        let mut headers = HeaderMap::new();
        for (name, value) in &args.headers {
            headers.append(header_name(name)?, header_value(value)?);
        }
        let ua = args
            .user_agent
            .clone()
            .unwrap_or_else(|| format!("whambam/{}", env!("CARGO_PKG_VERSION")));
        set_default(&mut headers, USER_AGENT, &ua)?;
        if let Some(accept) = &args.accept {
            set_default(&mut headers, ACCEPT, accept)?;
        }
        if body.is_some() {
            set_default(&mut headers, CONTENT_TYPE, &args.content_type)?;
        }
        if let Some(host) = &args.host {
            headers.insert(HOST, header_value(host)?);
        }
        if let Some(original) = apply_connect_to_port(&mut url, net) {
            set_default(&mut headers, HOST, &original)?;
        }
        Ok(RequestSpec {
            url,
            display_url: raw_url.to_string(),
            method: args.method.clone(),
            headers,
            body,
            basic_auth: args.basic_auth.clone(),
        })
    }

    /// Approximate bytes on the wire for the request line, headers and body.
    pub fn approx_size(&self) -> u64 {
        let line = self.method.as_str().len() + self.url.path().len() + 12;
        let query = self.url.query().map_or(0, str::len);
        let headers: usize = self
            .headers
            .iter()
            .map(|(n, v)| n.as_str().len() + v.len() + 4)
            .sum();
        let host = self.url.host_str().map_or(0, str::len) + 8;
        let body = self.body.as_ref().map_or(0, Bytes::len);
        (line + query + headers + host + body + 2) as u64
    }
}

fn load_body(args: &RequestArgs) -> Result<Option<Bytes>> {
    if let Some(body) = &args.body {
        return Ok(Some(Bytes::from(body.clone())));
    }
    match &args.body_file {
        Some(path) => {
            let data = fs::read(path)
                .with_context(|| format!("failed to read body file {}", path.display()))?;
            Ok(Some(Bytes::from(data)))
        }
        None => Ok(None),
    }
}

/// If a --connect-to rule changes the port, point the URL at the new port and
/// return the original authority to send as the Host header.
fn apply_connect_to_port(url: &mut Url, net: &NetConfig) -> Option<String> {
    let host = url.host_str()?.to_string();
    let port = url.port_or_known_default()?;
    let rule = net
        .connect_to
        .iter()
        .find(|r| r.host.eq_ignore_ascii_case(&host) && r.port == port)?;
    if rule.target_port == port {
        return None;
    }
    let original = match url.port() {
        Some(p) => format!("{host}:{p}"),
        None => host,
    };
    url.set_port(Some(rule.target_port)).ok()?;
    Some(original)
}

fn set_default(headers: &mut HeaderMap, name: HeaderName, value: &str) -> Result<()> {
    if !headers.contains_key(&name) {
        headers.insert(name, header_value(value)?);
    }
    Ok(())
}

fn header_name(name: &str) -> Result<HeaderName> {
    HeaderName::from_bytes(name.as_bytes()).with_context(|| format!("invalid header name '{name}'"))
}

fn header_value(value: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(value).with_context(|| format!("invalid header value '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    fn spec(args: &[&str]) -> Result<RequestSpec> {
        let cli = Cli::try_parse_from(std::iter::once("whambam").chain(args.iter().copied()))?;
        let net = NetConfig::from_cli(&cli.network)?;
        RequestSpec::from_cli(cli.url.as_deref().unwrap(), &cli.request, &net)
    }

    #[test]
    fn default_headers_are_added() {
        let s = spec(&["http://x/a?b=1"]).unwrap();
        assert!(
            s.headers[USER_AGENT]
                .to_str()
                .unwrap()
                .starts_with("whambam/")
        );
        assert!(!s.headers.contains_key(CONTENT_TYPE));
        assert!(s.approx_size() > 0);
    }

    #[test]
    fn content_type_only_with_body_and_user_headers_win() {
        let s = spec(&["http://x", "-d", "{}", "-T", "application/json"]).unwrap();
        assert_eq!(s.headers[CONTENT_TYPE], "application/json");
        assert_eq!(s.body.as_deref(), Some(&b"{}"[..]));

        let s = spec(&[
            "http://x",
            "-d",
            "a",
            "-H",
            "Content-Type: text/csv",
            "-A",
            "x/y",
        ])
        .unwrap();
        assert_eq!(s.headers[CONTENT_TYPE], "text/csv");
        assert_eq!(s.headers[ACCEPT], "x/y");
    }

    #[test]
    fn body_file_is_binary_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.bin");
        fs::write(&path, [0u8, 159, 146, 150]).unwrap();
        let s = spec(&["http://x", "-D", path.to_str().unwrap()]).unwrap();
        assert_eq!(s.body.unwrap().as_ref(), &[0u8, 159, 146, 150]);
        assert!(spec(&["http://x", "-D", "/no/such/file"]).is_err());
    }

    #[test]
    fn connect_to_rewrites_port_and_sets_host() {
        let s = spec(&[
            "http://example.com/p",
            "--connect-to",
            "example.com:80:127.0.0.1:8080",
        ]);
        let s = s.unwrap();
        assert_eq!(s.url.port(), Some(8080));
        assert_eq!(s.headers[HOST], "example.com");
        assert_eq!(s.display_url, "http://example.com/p");

        let same = spec(&[
            "http://example.com",
            "--connect-to",
            "example.com:80:10.0.0.1:80",
        ]);
        assert!(!same.unwrap().headers.contains_key(HOST));
    }
}
