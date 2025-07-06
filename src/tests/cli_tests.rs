use crate::parse_http_method;
use crate::HttpMethod;
use clap::Parser;

#[test]
fn test_parse_http_method() {
    assert_eq!(parse_http_method("GET").unwrap(), HttpMethod::GET);
    assert_eq!(parse_http_method("get").unwrap(), HttpMethod::GET);
    assert_eq!(parse_http_method("POST").unwrap(), HttpMethod::POST);
    assert_eq!(parse_http_method("post").unwrap(), HttpMethod::POST);
    assert_eq!(parse_http_method("PUT").unwrap(), HttpMethod::PUT);
    assert_eq!(parse_http_method("DELETE").unwrap(), HttpMethod::DELETE);
    assert_eq!(parse_http_method("HEAD").unwrap(), HttpMethod::HEAD);
    assert_eq!(parse_http_method("OPTIONS").unwrap(), HttpMethod::OPTIONS);

    assert!(parse_http_method("INVALID").is_err());
}

#[test]
fn test_parse_duration() {
    use crate::parse_duration;

    // Test seconds
    assert_eq!(parse_duration("10s").unwrap(), 10);
    assert_eq!(parse_duration("0s").unwrap(), 0);
    assert_eq!(parse_duration("999s").unwrap(), 999);

    // Test minutes
    assert_eq!(parse_duration("5m").unwrap(), 300);
    assert_eq!(parse_duration("1m").unwrap(), 60);
    assert_eq!(parse_duration("60m").unwrap(), 3600);

    // Test hours
    assert_eq!(parse_duration("2h").unwrap(), 7200);
    assert_eq!(parse_duration("1h").unwrap(), 3600);
    assert_eq!(parse_duration("24h").unwrap(), 86400);

    // Test raw seconds
    assert_eq!(parse_duration("42").unwrap(), 42);
    assert_eq!(parse_duration("0").unwrap(), 0);

    // Test invalid formats
    assert!(parse_duration("invalid").is_err());
    assert!(parse_duration("10x").is_err());
    assert!(parse_duration("-5s").is_err());
    assert!(parse_duration("5.5s").is_err()); // Fractional seconds
    assert!(parse_duration("m").is_err()); // Missing number
    assert!(parse_duration("").is_err()); // Empty string
}

#[test]
fn test_args_default_values() {
    use crate::Args;

    let args = Args::parse_from(["test", "http://example.com"]);

    assert_eq!(args.url, "http://example.com");
    assert_eq!(args.requests, 200);
    assert_eq!(args.concurrent, 50);
    assert_eq!(args.duration_str, "0");
    assert_eq!(args.rate_limit, 0.0);
    assert_eq!(args.headers.len(), 0);
    assert_eq!(args.timeout, 20);
    assert_eq!(args.output_format, "ui");
    assert_eq!(args.content_type, "text/html");
    assert!(!args.disable_compression);
    assert!(!args.disable_keepalive);
    assert!(!args.disable_redirects);
}

#[test]
fn test_args_custom_values() {
    use crate::Args;

    let args = Args::parse_from([
        "test",
        "https://example.org",
        "-n",
        "100",
        "-c",
        "25",
        "-z",
        "30s",
        "-q",
        "10.5",
        "-m",
        "POST",
        "-t",
        "30",
        "-H",
        "X-Test: value",
        "-A",
        "application/json",
        "-a",
        "user:pass",
        "-d",
        "test-body",
        "-T",
        "application/json",
        "-x",
        "localhost:8080",
        "--disable-compression",
        "--disable-keepalive",
        "--disable-redirects",
        "-o",
        "hey",
    ]);

    assert_eq!(args.url, "https://example.org");
    assert_eq!(args.requests, 100);
    assert_eq!(args.concurrent, 25);
    assert_eq!(args.duration_str, "30s");
    assert_eq!(args.rate_limit, 10.5);
    assert_eq!(args.headers, vec!["X-Test: value".to_string()]);
    assert_eq!(args.timeout, 30);
    assert_eq!(args.output_format, "hey");
    assert_eq!(args.content_type, "application/json");
    assert!(args.disable_compression);
    assert!(args.disable_keepalive);
    assert!(args.disable_redirects);
    assert_eq!(args.body, Some("test-body".to_string()));
    assert_eq!(args.accept, Some("application/json".to_string()));
    assert_eq!(args.basic_auth, Some("user:pass".to_string()));
    assert_eq!(args.proxy, Some("localhost:8080".to_string()));
}
