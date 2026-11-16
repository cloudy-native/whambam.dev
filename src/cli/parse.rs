// SPDX-License-Identifier: MIT
//! Value parsers shared by the command-line interface.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use reqwest::Method;

/// Parse a request count such as `200`, `10k` or `1.5m`.
pub fn parse_count(s: &str) -> Result<usize> {
    let s = s.trim().to_ascii_lowercase();
    let (num, mult) = match s.chars().last() {
        Some('k') => (&s[..s.len() - 1], 1_000.0),
        Some('m') => (&s[..s.len() - 1], 1_000_000.0),
        _ => (s.as_str(), 1.0),
    };
    let value: f64 = num
        .parse()
        .map_err(|_| anyhow!("invalid count '{s}': expected e.g. 200, 10k or 1m"))?;
    if !value.is_finite() || value < 0.0 {
        bail!("invalid count '{s}': must be zero or positive");
    }
    Ok((value * mult).round() as usize)
}

/// Parse a duration such as `30` (seconds), `500ms`, `1.5s`, `5m` or `2h`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        bail!("duration cannot be empty");
    }
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let value: f64 = num
        .parse()
        .map_err(|_| anyhow!("invalid duration '{s}': expected e.g. 30s, 500ms, 5m"))?;
    let secs = match unit {
        "" | "s" => value,
        "ms" => value / 1_000.0,
        "us" | "µs" => value / 1_000_000.0,
        "m" => value * 60.0,
        "h" => value * 3_600.0,
        _ => bail!("invalid duration unit '{unit}' in '{s}': use ms, s, m or h"),
    };
    Ok(Duration::from_secs_f64(secs))
}

/// Parse an HTTP method (case-insensitive). Any valid token is accepted.
pub fn parse_method(s: &str) -> Result<Method> {
    Method::from_bytes(s.trim().to_ascii_uppercase().as_bytes())
        .map_err(|_| anyhow!("invalid HTTP method '{s}'"))
}

/// Parse a `Name: Value` header.
pub fn parse_header(s: &str) -> Result<(String, String)> {
    let (name, value) = s
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid header '{s}': expected 'Name: Value'"))?;
    let name = name.trim();
    if name.is_empty() {
        bail!("invalid header '{s}': name is empty");
    }
    Ok((name.to_string(), value.trim().to_string()))
}

/// Parse `user:password` basic-auth credentials.
pub fn parse_basic_auth(s: &str) -> Result<(String, String)> {
    s.split_once(':')
        .map(|(u, p)| (u.to_string(), p.to_string()))
        .ok_or_else(|| anyhow!("invalid basic auth: expected 'username:password'"))
}

/// A `--connect-to` rule: requests for `host:port` go to `target_host:target_port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectTo {
    pub host: String,
    pub port: u16,
    pub target_host: String,
    pub target_port: u16,
}

/// Parse `host:port:target_host:target_port` (IPv6 hosts in brackets).
pub fn parse_connect_to(s: &str) -> Result<ConnectTo> {
    let parts = split_host_ports(s);
    let [host, port, target_host, target_port] = parts.as_slice() else {
        bail!("invalid --connect-to '{s}': expected host:port:target_host:target_port");
    };
    let port_of = |p: &str| {
        p.parse::<u16>()
            .map_err(|_| anyhow!("invalid port '{p}' in --connect-to '{s}'"))
    };
    Ok(ConnectTo {
        host: host.trim_matches(['[', ']']).to_string(),
        port: port_of(port)?,
        target_host: target_host.trim_matches(['[', ']']).to_string(),
        target_port: port_of(target_port)?,
    })
}

/// Split on ':' but keep bracketed IPv6 literals intact.
fn split_host_ports(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_brackets = false;
    for c in s.chars() {
        match c {
            '[' => in_brackets = true,
            ']' => in_brackets = false,
            ':' if !in_brackets => {
                parts.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    parts.push(current);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_accept_suffixes() {
        assert_eq!(parse_count("200").unwrap(), 200);
        assert_eq!(parse_count("10k").unwrap(), 10_000);
        assert_eq!(parse_count("1.5M").unwrap(), 1_500_000);
        assert_eq!(parse_count("0").unwrap(), 0);
        assert!(parse_count("ten").is_err());
        assert!(parse_count("-5").is_err());
    }

    #[test]
    fn durations_accept_units() {
        assert_eq!(parse_duration("30").unwrap(), Duration::from_secs(30));
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("1.5s").unwrap(), Duration::from_millis(1500));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("0").unwrap(), Duration::ZERO);
        for bad in ["", "s", "10x", "abc", "1.2.3s"] {
            assert!(parse_duration(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn methods_are_case_insensitive() {
        assert_eq!(parse_method("get").unwrap(), Method::GET);
        assert_eq!(parse_method("Patch").unwrap(), Method::PATCH);
        assert_eq!(parse_method("PURGE").unwrap().as_str(), "PURGE");
        assert!(parse_method("BAD METHOD").is_err());
    }

    #[test]
    fn headers_split_on_first_colon() {
        assert_eq!(
            parse_header("X-Url: http://a:1").unwrap(),
            ("X-Url".into(), "http://a:1".into())
        );
        assert!(parse_header("novalue").is_err());
        assert!(parse_header(": v").is_err());
    }

    #[test]
    fn basic_auth_requires_colon() {
        assert_eq!(
            parse_basic_auth("u:p:q").unwrap(),
            ("u".into(), "p:q".into())
        );
        assert!(parse_basic_auth("nopass").is_err());
    }

    #[test]
    fn connect_to_parses_ipv4_and_ipv6() {
        let rule = parse_connect_to("example.com:443:127.0.0.1:8443").unwrap();
        assert_eq!(rule.host, "example.com");
        assert_eq!(rule.port, 443);
        assert_eq!(rule.target_host, "127.0.0.1");
        assert_eq!(rule.target_port, 8443);

        let v6 = parse_connect_to("example.com:80:[::1]:8080").unwrap();
        assert_eq!(v6.target_host, "::1");
        assert!(parse_connect_to("a:b").is_err());
        assert!(parse_connect_to("a:x:b:1").is_err());
    }
}
