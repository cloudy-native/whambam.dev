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

use crate::tester::{print_hey_format_report, HttpMethod, TestConfig, TestState};
use std::time::{Duration, Instant};

#[test]
fn test_print_hey_format_report_basic() {
    let config = TestConfig {
        url: "http://localhost".to_string(),
        method: HttpMethod::GET,
        requests: 100,
        concurrent: 1,
        duration: 10,
        rate_limit: 0.0,
        headers: vec![],
        timeout: 30,
        body: None,
        content_type: "".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
        interactive: false,
        output_format: "hey".to_string(),
    };
    let mut test_state = TestState::new(&config);
    test_state.is_complete = true;
    test_state.start_time = Instant::now() - Duration::from_secs(10);
    test_state.end_time = Some(Instant::now());
    test_state.completed_requests = 100;
    test_state.total_bytes_received = 10240;
    test_state.min_latency = 10.0;
    test_state.max_latency = 100.0;
    test_state.status_counts.insert(200, 95);
    test_state.status_counts.insert(500, 5);
    test_state.error_count = 5;

    // Real samples in histogram (microseconds = ms * 1000)
    for _ in 0..50 {
        test_state.latency_histogram.record(10_000).unwrap(); // 10ms
    }
    for _ in 0..40 {
        test_state.latency_histogram.record(50_000).unwrap(); // 50ms
    }
    for _ in 0..10 {
        test_state.latency_histogram.record(100_000).unwrap(); // 100ms
    }

    let mut buf = Vec::new();
    print_hey_format_report(&mut buf, &test_state).unwrap();

    let output = String::from_utf8(buf).unwrap();

    assert!(output.contains("Summary:"));
    assert!(output.contains("Total:\t10.0000 secs"));
    assert!(output.contains("Requests/sec:"));
    assert!(output.contains("Latency distribution:"));
    assert!(output.contains("Status code distribution:"));
    // True mean of samples, not percentile blend
    assert!(output.contains("Average:\t"));
    // No fabricated phase breakdown
    assert!(
        !output.contains("DNS+dialup:"),
        "must not print unmeasured phase details"
    );
    assert!(
        !output.contains("DNS-lookup:"),
        "must not print unmeasured phase details"
    );
    // Real histogram has sample counts (not empty)
    assert!(output.contains("Response time histogram:"));
    assert!(output.contains("[200]\t95 responses"));
}

#[test]
fn test_print_hey_format_report_no_requests() {
    let config = TestConfig {
        url: "http://localhost".to_string(),
        method: HttpMethod::GET,
        requests: 0,
        concurrent: 1,
        duration: 10,
        rate_limit: 0.0,
        headers: vec![],
        timeout: 30,
        body: None,
        content_type: "".to_string(),
        basic_auth: None,
        proxy: None,
        disable_compression: false,
        disable_keepalive: false,
        disable_redirects: false,
        interactive: false,
        output_format: "hey".to_string(),
    };
    let mut test_state = TestState::new(&config);
    test_state.is_complete = true;
    test_state.start_time = Instant::now() - Duration::from_secs(10);
    test_state.end_time = Some(Instant::now());
    test_state.completed_requests = 0;

    let mut buf = Vec::new();
    print_hey_format_report(&mut buf, &test_state).unwrap();

    let output = String::from_utf8(buf).unwrap();

    assert!(output.contains("Summary:"));
    assert!(output.contains("Requests/sec:"));
    assert!(output.contains("Average:\t0.0000 secs"));
    assert!(output.contains("(no samples)") || output.contains("Response time histogram:"));
    assert!(!output.contains("DNS+dialup:"));
}
