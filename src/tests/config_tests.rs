use crate::tester::{HttpMethod, RequestMetric, TestConfig, TestState};
use std::time::Instant;

#[test]
fn test_test_config_initialization() {
    let config = TestConfig {
        url: "http://example.com".to_string(),
        method: HttpMethod::GET,
        requests: 100,
        concurrent: 10,
        duration: 30,
        rate_limit: 5.0,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        timeout: 20,
        body: Some("test body".to_string()),
        content_type: "application/json".to_string(),
        basic_auth: Some(("username".to_string(), "password".to_string())),
        proxy: Some("localhost:8080".to_string()),
        disable_compression: true,
        disable_keepalive: true,
        disable_redirects: true,
        interactive: true,
        output_format: "ui".to_string(),
    };

    assert_eq!(config.url, "http://example.com");
    assert!(matches!(config.method, HttpMethod::GET));
    assert_eq!(config.requests, 100);
    assert_eq!(config.concurrent, 10);
    assert_eq!(config.duration, 30);
    assert_eq!(config.rate_limit, 5.0);
    assert_eq!(config.headers.len(), 1);
    assert_eq!(config.headers[0].0, "Content-Type");
    assert_eq!(config.headers[0].1, "application/json");
    assert_eq!(config.timeout, 20);
    assert_eq!(config.body, Some("test body".to_string()));
    assert_eq!(config.content_type, "application/json");
    assert_eq!(
        config.basic_auth,
        Some(("username".to_string(), "password".to_string()))
    );
    assert_eq!(config.proxy, Some("localhost:8080".to_string()));
    assert!(config.disable_compression);
    assert!(config.disable_keepalive);
    assert!(config.disable_redirects);
}

#[test]
fn test_test_state_initialization() {
    let config = TestConfig {
        url: "http://example.com".to_string(),
        method: HttpMethod::GET,
        requests: 100,
        concurrent: 10,
        duration: 30,
        rate_limit: 5.0,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        timeout: 20,
        body: Some("test body".to_string()),
        content_type: "application/json".to_string(),
        basic_auth: Some(("username".to_string(), "password".to_string())),
        proxy: Some("localhost:8080".to_string()),
        disable_compression: true,
        disable_keepalive: true,
        disable_redirects: true,
        interactive: true,
        output_format: "ui".to_string(),
    };

    let test_state = TestState::new(&config);

    assert_eq!(test_state.url, "http://example.com");
    assert!(matches!(test_state.method, HttpMethod::GET));
    assert_eq!(test_state.target_requests, 100);
    assert_eq!(test_state.concurrent_requests, 10);
    assert_eq!(test_state.duration, 30);
    assert_eq!(test_state.headers.len(), 1);
    assert_eq!(test_state.headers[0].0, "Content-Type");
    assert_eq!(test_state.headers[0].1, "application/json");
    assert_eq!(test_state.completed_requests, 0);
    assert_eq!(test_state.error_count, 0);
    assert_eq!(test_state.status_counts.len(), 0);
    assert_eq!(test_state.recent_latencies.len(), 0);
    assert_eq!(test_state.recent_throughput.len(), 0);
    assert_eq!(test_state.min_latency, f64::MAX);
    assert_eq!(test_state.max_latency, 0.0);
    assert_eq!(test_state.p50_latency, 0.0);
    assert_eq!(test_state.p90_latency, 0.0);
    assert_eq!(test_state.p95_latency, 0.0);
    assert_eq!(test_state.p99_latency, 0.0);
    assert_eq!(test_state.current_throughput, 0.0);
    assert!(!test_state.is_complete);
    assert!(!test_state.should_quit);
    assert!(test_state.end_time.is_none());
}

#[test]
fn test_test_state_update() {
    let config = TestConfig {
        url: "http://example.com".to_string(),
        method: HttpMethod::GET,
        requests: 100,
        concurrent: 10,
        duration: 30,
        rate_limit: 5.0,
        headers: vec![],
        timeout: 20,
        body: None,
        content_type: "text/html".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
        interactive: true,
        output_format: "ui".to_string(),
    };

    let mut test_state = TestState::new(&config);

    // Test updating with successful request
    let metric_success = RequestMetric {
        timestamp: 0.1,
        latency_ms: 50.0,
        status_code: 200,
        is_error: false,
        bytes_sent: 100,
        bytes_received: 500,
    };

    test_state.update(metric_success);

    assert_eq!(test_state.completed_requests, 1);
    assert_eq!(test_state.error_count, 0);
    assert_eq!(test_state.status_counts.get(&200), Some(&1));
    assert_eq!(test_state.recent_latencies.len(), 1);
    assert_eq!(test_state.recent_latencies[0], 50.0);
    assert_eq!(test_state.min_latency, 50.0);
    assert_eq!(test_state.max_latency, 50.0);
    assert_eq!(test_state.total_bytes_sent, 100);
    assert_eq!(test_state.total_bytes_received, 500);
    assert!(!test_state.is_complete);

    // Test updating with error request
    let metric_error = RequestMetric {
        timestamp: 0.2,
        latency_ms: 100.0,
        status_code: 500,
        is_error: true,
        bytes_sent: 150,
        bytes_received: 200,
    };

    test_state.update(metric_error);

    assert_eq!(test_state.completed_requests, 2);
    assert_eq!(test_state.error_count, 1);
    assert_eq!(test_state.status_counts.get(&500), Some(&1));
    assert_eq!(test_state.recent_latencies.len(), 2);
    assert_eq!(test_state.recent_latencies[1], 100.0);
    assert_eq!(test_state.min_latency, 50.0);
    assert_eq!(test_state.max_latency, 100.0);
    assert_eq!(test_state.total_bytes_sent, 250); // 100 + 150
    assert_eq!(test_state.total_bytes_received, 700); // 500 + 200
    assert!(!test_state.is_complete);
}

#[test]
fn test_test_state_reset() {
    let config = TestConfig {
        url: "http://example.com".to_string(),
        method: HttpMethod::GET,
        requests: 100,
        concurrent: 10,
        duration: 30,
        rate_limit: 5.0,
        headers: vec![],
        timeout: 20,
        body: None,
        content_type: "text/html".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
        interactive: true,
        output_format: "ui".to_string(),
    };

    let mut test_state = TestState::new(&config);

    // Add some data
    let metric = RequestMetric {
        timestamp: 0.1,
        latency_ms: 50.0,
        status_code: 200,
        is_error: false,
        bytes_sent: 120,
        bytes_received: 800,
    };

    test_state.update(metric);
    test_state.is_complete = true;
    test_state.should_quit = true;
    test_state.end_time = Some(Instant::now());

    // Reset and verify all values are reset
    test_state.reset();

    assert_eq!(test_state.completed_requests, 0);
    assert_eq!(test_state.error_count, 0);
    assert_eq!(test_state.status_counts.len(), 0);
    assert_eq!(test_state.recent_latencies.len(), 0);
    assert_eq!(test_state.recent_throughput.len(), 0);
    assert_eq!(test_state.min_latency, f64::MAX);
    assert_eq!(test_state.max_latency, 0.0);
    assert_eq!(test_state.p50_latency, 0.0);
    assert_eq!(test_state.p90_latency, 0.0);
    assert_eq!(test_state.p95_latency, 0.0);
    assert_eq!(test_state.p99_latency, 0.0);
    assert_eq!(test_state.current_throughput, 0.0);
    assert_eq!(test_state.total_bytes_sent, 0);
    assert_eq!(test_state.total_bytes_received, 0);
    assert!(!test_state.is_complete);
    assert!(!test_state.should_quit);
    assert!(test_state.end_time.is_none());
}

#[test]
fn test_http_method_display() {
    assert_eq!(HttpMethod::GET.to_string(), "GET");
    assert_eq!(HttpMethod::POST.to_string(), "POST");
    assert_eq!(HttpMethod::PUT.to_string(), "PUT");
    assert_eq!(HttpMethod::DELETE.to_string(), "DELETE");
    assert_eq!(HttpMethod::HEAD.to_string(), "HEAD");
    assert_eq!(HttpMethod::OPTIONS.to_string(), "OPTIONS");
}
