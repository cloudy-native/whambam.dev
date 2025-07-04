use crate::Args;
use clap::Parser;
use crate::HttpMethod;

#[test]
fn test_comprehensive_command_line() {
    // Test all possible arguments together
    let args = Args::parse_from([
        "blamo-web-throughput",
        "https://api.example.com/test/endpoint",
        "-n", "1000",
        "-c", "50",
        "-z", "60s",
        "-q", "100.5",
        "-m", "POST",
        "-t", "30",
        "-H", "Content-Type: application/json",
        "-H", "User-Agent: blamo-test",
        "-H", "Authorization: Bearer token123",
        "-A", "application/json",
        "-a", "username:password123",
        "-d", "{\"test\":\"data\"}",
        "-T", "application/json",
        "-x", "proxy.example.com:8080",
        "--disable-compression",
        "--disable-keepalive",
        "--disable-redirects",
        "-o", "hey"
    ]);
    
    // Verify all arguments were parsed correctly
    assert_eq!(args.url, "https://api.example.com/test/endpoint");
    assert_eq!(args.requests, 1000);
    assert_eq!(args.concurrent, 50);
    assert_eq!(args.duration_str, "60s");
    assert_eq!(args.rate_limit, 100.5);
    assert!(matches!(args.method, HttpMethod::POST));
    assert_eq!(args.timeout, 30);
    
    // Check headers
    assert_eq!(args.headers.len(), 3);
    assert_eq!(args.headers[0], "Content-Type: application/json");
    assert_eq!(args.headers[1], "User-Agent: blamo-test");
    assert_eq!(args.headers[2], "Authorization: Bearer token123");
    
    // Check other HTTP options
    assert_eq!(args.accept, Some("application/json".to_string()));
    assert_eq!(args.basic_auth, Some("username:password123".to_string()));
    assert_eq!(args.body, Some("{\"test\":\"data\"}".to_string()));
    assert_eq!(args.content_type, "application/json");
    assert_eq!(args.proxy, Some("proxy.example.com:8080".to_string()));
    
    // Check flags
    assert!(args.disable_compression);
    assert!(args.disable_keepalive);
    assert!(args.disable_redirects);
    assert_eq!(args.output_format, "hey");
}

#[test]
fn test_minimum_command_line() {
    // Test with only the required URL argument
    let args = Args::parse_from([
        "blamo-web-throughput",
        "http://example.com"
    ]);
    
    // Verify defaults are applied correctly
    assert_eq!(args.url, "http://example.com");
    assert_eq!(args.requests, 200);
    assert_eq!(args.concurrent, 50);
    assert_eq!(args.duration_str, "0");
    assert_eq!(args.rate_limit, 0.0);
    assert!(matches!(args.method, HttpMethod::GET));
    assert_eq!(args.timeout, 20);
    assert_eq!(args.headers.len(), 0);
    assert_eq!(args.accept, None);
    assert_eq!(args.basic_auth, None);
    assert_eq!(args.body, None);
    assert_eq!(args.body_file, None);
    assert_eq!(args.content_type, "text/html");
    assert_eq!(args.proxy, None);
    assert!(!args.disable_compression);
    assert!(!args.disable_keepalive);
    assert!(!args.disable_redirects);
    assert_eq!(args.output_format, "ui");
}