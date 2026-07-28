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

#![allow(deprecated)]

use crate::tester::{HttpMethod, SharedState, TestConfig, UnifiedRunner as TestRunner};
use crate::tests::MockServer;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::sleep;

#[tokio::test]
async fn test_runner_sends_request_body() {
    let server = MockServer::start().await;

    let config = TestConfig {
        url: server.url(),
        method: HttpMethod::POST,
        requests: 1,
        concurrent: 1,
        duration: 0,
        rate_limit: 0.0,
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        timeout: 2,
        body: Some("hello-body".to_string()),
        content_type: "text/plain".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
        interactive: false,
        output_format: "hey".to_string(),
    };

    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState {
        state: Arc::clone(&state),
    };
    let mut runner = TestRunner::with_state(config, shared_state);

    runner.start().await.expect("Runner failed to start");

    let mut iterations = 0;
    let max_iterations = 50;
    while server.request_count() < 1 {
        iterations += 1;
        if iterations >= max_iterations {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    let body = server
        .get_last_body()
        .expect("Expected mock server to capture a request body");
    assert_eq!(String::from_utf8_lossy(&body), "hello-body");
}

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
        duration: 0,     // No duration limit
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
        interactive: false,
        output_format: "hey".to_string(),
    };

    // Create shared state and test runner
    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState {
        state: Arc::clone(&state),
    };
    let mut runner = TestRunner::with_state(config, shared_state.clone());

    // Start the test
    runner.start().await.expect("Runner failed to start");

    // Wait for test to complete (using sleep since this is just a test)
    let mut iterations = 0;
    let max_iterations = 50; // Wait up to 5 seconds

    loop {
        {
            let test_state = state.lock().unwrap();
            if test_state.is_complete || test_state.completed_requests >= 10 {
                break;
            }
        }

        iterations += 1;
        if iterations >= max_iterations {
            break; // Safety timeout
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
        interactive: false,
        output_format: "hey".to_string(),
    };

    // Create shared state and test runner
    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState {
        state: Arc::clone(&state),
    };
    let mut runner = TestRunner::with_state(config, shared_state.clone());

    // Start the test
    runner.start().await.expect("Runner failed to start");

    // Wait for test to complete
    let mut iterations = 0;
    let max_iterations = 50; // Wait up to 5 seconds

    loop {
        {
            let test_state = state.lock().unwrap();
            if test_state.is_complete || test_state.completed_requests >= 10 {
                break;
            }
        }

        iterations += 1;
        if iterations >= max_iterations {
            break; // Safety timeout
        }

        sleep(Duration::from_millis(100)).await;
    }

    // Verify results
    let test_state = state.lock().unwrap();
    assert_eq!(test_state.completed_requests, 10);
    assert_eq!(test_state.error_count, 10); // All errors (status 500)
    assert!(test_state.status_counts.contains_key(&500));
    assert_eq!(test_state.status_counts[&500], 10);
    assert!(test_state.is_complete);

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
        duration: 1, // 1 second duration limit
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
        interactive: false,
        output_format: "hey".to_string(),
    };

    // Create shared state and test runner
    let state = Arc::new(Mutex::new(crate::tester::TestState::new(&config)));
    let shared_state = SharedState {
        state: Arc::clone(&state),
    };
    let mut runner = TestRunner::with_state(config, shared_state.clone());

    // Start the test
    runner.start().await.expect("Runner failed to start");

    // Wait for duration plus drain to finish
    let mut iterations = 0;
    let max_iterations = 50; // Wait up to 5 seconds

    loop {
        {
            let test_state = state.lock().unwrap();
            if test_state.is_complete {
                break;
            }
        }

        iterations += 1;
        if iterations >= max_iterations {
            break; // Safety timeout
        }

        sleep(Duration::from_millis(100)).await;
    }

    // Verify results
    let test_state = state.lock().unwrap();
    assert!(
        test_state.is_complete,
        "duration-limited run should mark is_complete after drain"
    );
    assert!(test_state.completed_requests < 100); // Should not have completed all requests
    assert!(test_state.completed_requests > 0); // But should have completed some

    assert!(server.request_count() > 0);
}

#[tokio::test]
async fn test_runner_rejects_zero_concurrency() {
    let config = TestConfig {
        url: "http://127.0.0.1:9/".to_string(),
        method: HttpMethod::GET,
        requests: 1,
        concurrent: 0,
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
        interactive: false,
        output_format: "hey".to_string(),
    };

    let mut runner = TestRunner::new(config);
    let err = runner
        .start()
        .await
        .expect_err("concurrent=0 should fail to start");
    assert!(
        err.to_string().contains("concurrent"),
        "unexpected error: {err}"
    );
}
