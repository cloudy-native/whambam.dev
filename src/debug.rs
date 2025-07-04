use crate::tester::HttpMethod;
use anyhow::Result;
use futures::{stream, StreamExt};
use reqwest::Client;
use std::time::Instant;
use tokio::sync::mpsc;

/// Function to test just the HTTP request functionality
pub async fn run_debug_test(
    url: &str,
    requests: usize,
    concurrent: usize,
    duration_secs: u64,
    method: HttpMethod,
    headers: Vec<(String, String)>,
    timeout: u64,
    body: Option<String>,
    content_type: String,
    basic_auth: Option<(String, String)>,
    proxy: Option<String>,
    http2: bool,
    disable_compression: bool,
    disable_keepalive: bool,
    disable_redirects: bool,
) -> Result<()> {
    // Create client with specified configuration
    let client = {
        let mut client_builder = Client::builder();

        // Configure HTTP/2 if requested
        if http2 {
            client_builder = client_builder.use_rustls_tls().http2_prior_knowledge();
        }

        // Configure proxy if specified
        if let Some(proxy_addr) = &proxy {
            let proxy_url = format!("http://{}", proxy_addr);
            match reqwest::Proxy::http(&proxy_url) {
                Ok(proxy) => {
                    client_builder = client_builder.proxy(proxy);
                }
                Err(_) => {
                    eprintln!("Warning: Invalid proxy URL.");
                }
            }
        }

        // Configure additional HTTP options
        if disable_compression {
            client_builder = client_builder.no_gzip().no_brotli().no_deflate();
        }

        if disable_keepalive {
            client_builder = client_builder.tcp_nodelay(true).pool_max_idle_per_host(0);
        }

        if disable_redirects {
            client_builder = client_builder.redirect(reqwest::redirect::Policy::none());
        }

        // Build the client
        client_builder.build().unwrap_or_else(|_| {
            eprintln!(
                "Warning: Failed to configure client with specified options. Using default client."
            );
            Client::new()
        })
    };
    let mut successful = 0;
    let mut failures = 0;
    let mut status_counts = std::collections::HashMap::new();

    println!("=== Debug Test ===");
    println!("URL: {}", url);
    println!("HTTP Method: {}", method);

    // Display custom headers if any
    if !headers.is_empty() {
        println!("Custom headers:");
        for (name, value) in &headers {
            println!("  {}: {}", name, value);
        }
    }

    // When duration is specified, don't show requests since we're ignoring that parameter
    if duration_secs > 0 {
        println!("Concurrent: {}", concurrent);
        println!("Duration: {} seconds", duration_secs);
    } else {
        println!("Requests: {}, Concurrent: {}", requests, concurrent);
        println!("Duration: Unlimited");
    }

    // Create a channel for results
    let (tx, mut rx) = mpsc::channel(100);
    let start = Instant::now();

    // Spawn a task to process results
    let results_handle = tokio::spawn(async move {
        while let Some((status, is_error)) = rx.recv().await {
            if is_error {
                failures += 1;
                println!("Request error");
            } else {
                successful += 1;
                *status_counts.entry(status).or_insert(0) += 1;
                println!("Response: HTTP {}", status);
            }
        }

        // Return results
        (successful, failures, status_counts)
    });

    // Create and send requests
    let start_time = Instant::now();
    let max_requests = if requests > 0 { requests } else { usize::MAX };
    let max_duration = if duration_secs > 0 {
        Some(std::time::Duration::from_secs(duration_secs))
    } else {
        None
    };

    let stream = stream::iter(0..max_requests)
        .map(|i| {
            let client = &client;
            let url_str = url.to_string();
            let tx = tx.clone();
            let start = start_time.clone();
            let headers_clone = headers.clone(); // Clone headers here to avoid ownership issues

            async move {
                // Check if we should stop due to duration
                if let Some(max_dur) = max_duration {
                    if start.elapsed() >= max_dur {
                        return;
                    }
                }
                println!("Sending request {}", i);
                // Create the request builder based on method
                let mut request_builder = match method {
                    HttpMethod::GET => client.get(&url_str),
                    HttpMethod::POST => client.post(&url_str),
                    HttpMethod::PUT => client.put(&url_str),
                    HttpMethod::DELETE => client.delete(&url_str),
                    HttpMethod::HEAD => client.head(&url_str),
                    HttpMethod::OPTIONS => client.request(reqwest::Method::OPTIONS, &url_str),
                };

                // Add debug header
                request_builder = request_builder.header("X-Debug-ID", i.to_string());

                // Add custom headers from our earlier clone
                for (name, value) in &headers_clone {
                    request_builder = request_builder.header(name, value);
                }

                // Set request timeout if specified
                if timeout > 0 {
                    request_builder =
                        request_builder.timeout(std::time::Duration::from_secs(timeout));
                }

                // Add basic auth if provided
                if let Some((username, password)) = &basic_auth {
                    request_builder = request_builder.basic_auth(username, Some(password));
                }

                // Add request body if provided
                if let Some(body_content) = &body {
                    request_builder = request_builder.body(body_content.clone());
                }

                // Send the request
                let result = request_builder.send().await;

                match result {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let status_class = status / 100;
                        let is_error = status_class != 2; // Consider non-2xx as errors
                        tx.send((status, is_error)).await.unwrap();
                    }
                    Err(_) => {
                        tx.send((0, true)).await.unwrap();
                    }
                }
            }
        })
        .buffer_unordered(concurrent);

    stream.collect::<Vec<()>>().await;

    // Close channel and wait for results processing
    drop(tx);
    let (successful, failures, status_counts) = results_handle.await.unwrap();

    let elapsed = start.elapsed();

    println!("\n=== Debug Test Results ===");
    println!("Total time: {:.2?}", elapsed);
    println!("Successful requests: {}", successful);
    println!("Failed requests: {}", failures);
    println!("Status code distribution:");

    let mut status_codes: Vec<u16> = status_counts.keys().cloned().collect();
    status_codes.sort();

    for status in status_codes {
        let count = status_counts.get(&status).unwrap();
        println!("  HTTP {}: {}", status, count);
    }

    Ok(())
}
