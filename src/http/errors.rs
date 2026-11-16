// SPDX-License-Identifier: MIT
//! Classify transport errors into a small set of kinds people can act on.

use std::error::Error as StdError;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Timeout,
    Dns,
    ConnectionRefused,
    ConnectionClosed,
    Tls,
    Connect,
    Redirect,
    Body,
    Request,
    Other,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 10] = [
        ErrorKind::Timeout,
        ErrorKind::Dns,
        ErrorKind::ConnectionRefused,
        ErrorKind::ConnectionClosed,
        ErrorKind::Tls,
        ErrorKind::Connect,
        ErrorKind::Redirect,
        ErrorKind::Body,
        ErrorKind::Request,
        ErrorKind::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ErrorKind::Timeout => "timeout",
            ErrorKind::Dns => "dns",
            ErrorKind::ConnectionRefused => "connection refused",
            ErrorKind::ConnectionClosed => "connection closed",
            ErrorKind::Tls => "tls",
            ErrorKind::Connect => "connect",
            ErrorKind::Redirect => "too many redirects",
            ErrorKind::Body => "body read",
            ErrorKind::Request => "request",
            ErrorKind::Other => "other",
        }
    }
}

/// Map a reqwest error to an [`ErrorKind`].
pub fn classify(err: &reqwest::Error) -> ErrorKind {
    let text = error_chain(err).to_ascii_lowercase();
    classify_text(
        &text,
        Flags {
            timeout: err.is_timeout(),
            redirect: err.is_redirect(),
            connect: err.is_connect(),
            body: err.is_body() || err.is_decode(),
            request: err.is_request(),
        },
    )
}

#[derive(Default, Clone, Copy)]
struct Flags {
    timeout: bool,
    redirect: bool,
    connect: bool,
    body: bool,
    request: bool,
}

fn classify_text(text: &str, f: Flags) -> ErrorKind {
    let has = |needles: &[&str]| needles.iter().any(|n| text.contains(n));
    if f.timeout || has(&["timed out", "deadline has elapsed"]) {
        ErrorKind::Timeout
    } else if has(&[
        "dns error",
        "failed to lookup",
        "no such host",
        "nodename nor servname",
    ]) {
        ErrorKind::Dns
    } else if has(&[
        "certificate",
        "tls",
        "handshake",
        "invalid peer",
        "unknownissuer",
    ]) {
        ErrorKind::Tls
    } else if has(&["connection refused", "actively refused"]) {
        ErrorKind::ConnectionRefused
    } else if has(&[
        "connection reset",
        "broken pipe",
        "connection closed",
        "incomplete message",
    ]) {
        ErrorKind::ConnectionClosed
    } else if f.redirect {
        ErrorKind::Redirect
    } else if f.connect {
        ErrorKind::Connect
    } else if f.body {
        ErrorKind::Body
    } else if f.request {
        ErrorKind::Request
    } else {
        ErrorKind::Other
    }
}

/// Join an error and all of its sources into one line.
pub fn error_chain(err: &dyn StdError) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(s) = source {
        text.push_str(": ");
        text.push_str(&s.to_string());
        source = s.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(text: &str) -> ErrorKind {
        classify_text(&text.to_ascii_lowercase(), Flags::default())
    }

    #[test]
    fn classifies_common_messages() {
        assert_eq!(kind("operation timed out"), ErrorKind::Timeout);
        assert_eq!(kind("dns error: failed to lookup address"), ErrorKind::Dns);
        assert_eq!(
            kind("invalid peer certificate: UnknownIssuer"),
            ErrorKind::Tls
        );
        assert_eq!(
            kind("tcp connect error: Connection refused (os error 61)"),
            ErrorKind::ConnectionRefused
        );
        assert_eq!(
            kind("connection reset by peer"),
            ErrorKind::ConnectionClosed
        );
        assert_eq!(kind("something odd"), ErrorKind::Other);
    }

    #[test]
    fn flags_are_used_when_text_is_vague() {
        let f = Flags {
            connect: true,
            ..Flags::default()
        };
        assert_eq!(
            classify_text("error sending request", f),
            ErrorKind::Connect
        );
        let f = Flags {
            redirect: true,
            ..Flags::default()
        };
        assert_eq!(
            classify_text("error following redirect", f),
            ErrorKind::Redirect
        );
        let f = Flags {
            timeout: true,
            connect: true,
            ..Flags::default()
        };
        assert_eq!(classify_text("x", f), ErrorKind::Timeout);
    }

    #[test]
    fn labels_are_unique() {
        let mut labels: Vec<_> = ErrorKind::ALL.iter().map(|k| k.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), ErrorKind::ALL.len());
    }

    #[tokio::test]
    async fn real_refused_connection_is_classified() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        crate::http::client::install_crypto_provider();
        let err = reqwest::Client::new()
            .get(format!("http://127.0.0.1:{port}"))
            .send()
            .await
            .unwrap_err();
        assert_eq!(classify(&err), ErrorKind::ConnectionRefused);
    }
}
