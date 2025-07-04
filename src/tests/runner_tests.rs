use crate::tests::MockServer;
use crate::tester::{HttpMethod, TestConfig, TestRunner, SharedState};
use std::{sync::{Arc, Mutex}, time::Duration};
use tokio::time::sleep;

#[tokio::test]
async fn test_runner_basic_functionality() {
    // Start mock server
    let server = MockServer::start().await;
    
    // Create test config
    let config = TestConfig {
        url: server.url(),
        method: HttpMethod::GET,
        requests: 10,
        concurrent: 2,
        duration: 0,  // No duration limit
        rate_limit: 0.0, // No rate limit
        headers: vec![("X-Test".to_string(), "test-value".to_string())],
        timeout: 1,
        body: None,
        content_type: "text/html".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
    };
    
    // Create shared state and test runner
    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState { state: Arc::clone(&state) };
    let mut runner = TestRunner::with_state(config, shared_state.clone());
    
    // Start the test
    runner.start().await.expect("Runner failed to start");
    
    // Wait for test to complete (using sleep since this is just a test)
    for _ in 0..10 {
        {
            let test_state = state.lock().unwrap();
            if test_state.completed_requests >= 10 {
                break;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    
    // Verify results
    let test_state = state.lock().unwrap();
    assert_eq!(test_state.completed_requests, 10);
    assert_eq!(test_state.error_count, 0);
    assert!(test_state.status_counts.contains_key(&200));
    assert_eq!(test_state.status_counts[&200], 10);
    assert!(test_state.is_complete);
    
    // Verify server received our requests
    assert_eq!(server.request_count(), 10);
    let headers = server.get_received_headers();
    assert!(headers.contains_key("x-test"));
    assert_eq!(headers.get("x-test").unwrap()[0], "test-value");
}

#[tokio::test]
async fn test_runner_with_errors() {
    // Start mock server
    let server = MockServer::start().await;
    
    // Configure server to return errors for half of the requests
    server.set_response_status(500);
    
    // Create test config
    let config = TestConfig {
        url: server.url(),
        method: HttpMethod::GET,
        requests: 10,
        concurrent: 2,
        duration: 0,
        rate_limit: 0.0,
        headers: vec![],
        timeout: 1,
        body: None,
        content_type: "text/html".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
    };
    
    // Create shared state and test runner
    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState { state: Arc::clone(&state) };
    let mut runner = TestRunner::with_state(config, shared_state.clone());
    
    // Start the test
    runner.start().await.expect("Runner failed to start");
    
    // Wait for test to complete
    for _ in 0..10 {
        {
            let test_state = state.lock().unwrap();
            if test_state.completed_requests >= 10 {
                break;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    
    // Verify results
    let test_state = state.lock().unwrap();
    assert_eq!(test_state.completed_requests, 10);
    assert_eq!(test_state.error_count, 10); // All should be errors since status code is 500
    assert!(test_state.status_counts.contains_key(&500));
    assert_eq!(test_state.status_counts[&500], 10);
    assert!(test_state.is_complete);
    
    // Verify server received our requests
    assert_eq!(server.request_count(), 10);
}

#[tokio::test]
async fn test_runner_duration_limit() {
    // Start mock server
    let server = MockServer::start().await;
    
    // Add a delay to the server responses to ensure we hit the duration limit
    server.set_response_delay(100);
    
    // Create test config with 1 second duration limit
    let config = TestConfig {
        url: server.url(),
        method: HttpMethod::GET,
        requests: 100, // More than we should be able to complete in 1 second
        concurrent: 5,
        duration: 1,   // 1 second duration limit
        rate_limit: 0.0,
        headers: vec![],
        timeout: 2,
        body: None,
        content_type: "text/html".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
    };
    
    // Create shared state and test runner
    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState { state: Arc::clone(&state) };
    let mut runner = TestRunner::with_state(config, shared_state.clone());
    
    // Start the test
    runner.start().await.expect("Runner failed to start");
    
    // Wait for duration plus a bit more to ensure test completes
    sleep(Duration::from_millis(1500)).await;
    
    // Verify results
    let test_state = state.lock().unwrap();
    assert!(test_state.is_complete);
    assert!(test_state.completed_requests < 100); // Should not have completed all requests
    assert!(test_state.completed_requests > 0);   // But should have completed some
    
    // Verify server received our requests
    assert_eq!(server.request_count(), test_state.completed_requests);
}