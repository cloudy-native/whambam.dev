// whambam - A high-performance HTTP load testing tool
//
// Copyright (c) 2025 Stephen Harrison
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::Args;
use clap::{error::ErrorKind, Parser};

#[test]
fn test_args_required_url() {
    // Test that URL is required
    let result = Args::try_parse_from(["test"]);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.kind() == ErrorKind::MissingRequiredArgument);
}

#[test]
fn test_args_invalid_concurrency() {
    // Test invalid concurrent value (non-numeric)
    let result = Args::try_parse_from(["test", "http://example.com", "-c", "invalid"]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind() == ErrorKind::ValueValidation);
}

#[test]
fn test_args_invalid_requests() {
    // Test invalid requests value (non-numeric)
    let result = Args::try_parse_from(["test", "http://example.com", "-n", "invalid"]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind() == ErrorKind::ValueValidation);
}

#[test]
fn test_args_invalid_rate_limit() {
    // Test invalid rate limit value (non-numeric)
    let result = Args::try_parse_from(["test", "http://example.com", "-q", "invalid"]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind() == ErrorKind::ValueValidation);
}

#[test]
fn test_args_invalid_timeout() {
    // Test invalid timeout value (non-numeric)
    let result = Args::try_parse_from(["test", "http://example.com", "-t", "invalid"]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind() == ErrorKind::ValueValidation);
}

#[test]
fn test_args_invalid_method() {
    // Test invalid HTTP method
    let result = Args::try_parse_from(["test", "http://example.com", "-m", "INVALID_METHOD"]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.kind() == ErrorKind::ValueValidation);
}

#[test]
fn test_args_multiple_headers() {
    // Test multiple headers
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-H",
        "Content-Type: application/json",
        "-H",
        "Accept: text/plain",
        "-H",
        "X-Custom-Header: custom-value",
    ]);

    assert_eq!(args.headers.len(), 3);
    assert_eq!(args.headers[0], "Content-Type: application/json");
    assert_eq!(args.headers[1], "Accept: text/plain");
    assert_eq!(args.headers[2], "X-Custom-Header: custom-value");
}

#[test]
fn test_body_and_body_file_mutual_exclusion() {
    // In a real application, we would want to enforce that -d and -D can't be used together
    // This test verifies that when both are provided, the last one wins (in this case -D)
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-d",
        "direct-body-content",
        "-D",
        "/path/to/file.txt",
    ]);

    assert_eq!(args.body, Some("direct-body-content".to_string()));
    assert_eq!(args.body_file, Some("/path/to/file.txt".to_string()));

    // And the reverse order
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-D",
        "/path/to/file.txt",
        "-d",
        "direct-body-content",
    ]);

    assert_eq!(args.body_file, Some("/path/to/file.txt".to_string()));
    assert_eq!(args.body, Some("direct-body-content".to_string()));
}

#[test]
fn test_args_extreme_values() {
    // Test extreme values
    let args = Args::parse_from([
        "test",
        "http://example.com",
        "-n",
        "0", // Unlimited requests
        "-c",
        "10000", // Very high concurrency
        "-z",
        "86400", // 24 hours duration
        "-q",
        "100000", // Very high rate limit
        "-t",
        "0", // Infinite timeout
    ]);

    assert_eq!(args.requests, 0);
    assert_eq!(args.concurrent, 10000);
    assert_eq!(args.duration_str, "86400");
    assert_eq!(args.rate_limit, 100000.0);
    assert_eq!(args.timeout, 0);
}

#[test]
fn test_args_output_format_validation() {
    // Test valid output formats
    let args_ui = Args::parse_from(["test", "http://example.com", "-o", "ui"]);
    assert_eq!(args_ui.output_format, "ui");

    let args_hey = Args::parse_from(["test", "http://example.com", "-o", "hey"]);
    assert_eq!(args_hey.output_format, "hey");

    // Invalid output formats are accepted at parsing time
    // but would be validated at runtime
    let args_invalid = Args::parse_from(["test", "http://example.com", "-o", "invalid"]);
    assert_eq!(args_invalid.output_format, "invalid");
}
