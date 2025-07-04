use reqwest::Client;
use std::io::{self, Write};
use std::time::Instant;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

#[derive(Debug, Default)]
struct RequestStats {
    requests_sent: usize,
    successful_responses: usize,
    error_responses: usize,
    status_counts: std::collections::HashMap<u16, usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration
    let url = "http://localhost:8080";
    let total_requests = 10;
    let concurrent_requests = 5;
    let debug_interval_ms = 500; // Print debug info every 500ms

    println!("=== BLAMO Debug Test ===");
    println!("URL: {}", url);
    println!(
        "Requests: {}, Concurrent: {}",
        total_requests, concurrent_requests
    );
    println!("Press Enter to start the test...");

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    // Shared stats object
    let stats = Arc::new(Mutex::new(RequestStats::default()));
    let client = Client::new();

    // Debug printing task
    let stats_clone = Arc::clone(&stats);
    let debug_handle = tokio::spawn(async move {
        let start = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(debug_interval_ms)).await;
            let stats_guard = stats_clone.lock().await;

            print!("\r\x1B[2K"); // Clear line
            print!(
                "[{:.1}s] Sent: {}, Success: {}, Errors: {}, Status codes: {:?}",
                start.elapsed().as_secs_f32(),
                stats_guard.requests_sent,
                stats_guard.successful_responses,
                stats_guard.error_responses,
                stats_guard.status_counts
            );
            io::stdout().flush().unwrap();

            if stats_guard.requests_sent >= total_requests {
                break;
            }
        }
        println!(); // Final newline
    });

    // Create and send requests
    println!("\nSending requests to {}...", url);
    let start = Instant::now();

    let mut handles = Vec::with_capacity(total_requests);

    for i in 0..total_requests {
        let client = client.clone();
        let stats = Arc::clone(&stats);
        let url = url.to_string();

        // Delay start of some requests to prevent all firing at exactly the same time
        if i > 0 && i % concurrent_requests == 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let handle = tokio::spawn(async move {
            // Update sent count
            {
                let mut stats_guard = stats.lock().await;
                stats_guard.requests_sent += 1;
            }

            // Send request
            let result = client
                .get(&url)
                .header("X-Request-ID", format!("debug-{}", i))
                .send()
                .await;

            // Process response
            {
                let mut stats_guard = stats.lock().await;
                match result {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        *stats_guard.status_counts.entry(status).or_insert(0) += 1;
                        stats_guard.successful_responses += 1;

                        // Debug output for successful response
                        let body = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Failed to get body".to_string());
                        if body.len() > 100 {
                            println!(
                                "\nResponse {}: Status {}, Body length: {} bytes",
                                i,
                                status,
                                body.len()
                            );
                        } else {
                            println!("\nResponse {}: Status {}, Body: {}", i, status, body);
                        }
                    }
                    Err(e) => {
                        stats_guard.error_responses += 1;
                        println!("\nRequest {} error: {}", i, e);
                    }
                }
            }
        });

        handles.push(handle);

        // Limit concurrency
        if handles.len() >= concurrent_requests {
            tokio::join!(handles.remove(0));
        }
    }

    // Wait for all remaining requests to complete
    for handle in handles {
        handle.await?;
    }

    // Wait for debug task to finish
    debug_handle.await?;

    // Final stats
    let elapsed = start.elapsed();
    let final_stats = stats.lock().await;

    println!("\n=== Final Results ===");
    println!("Total time: {:.2?}", elapsed);
    println!("Requests sent: {}", final_stats.requests_sent);
    println!("Successful responses: {}", final_stats.successful_responses);
    println!("Error responses: {}", final_stats.error_responses);
    println!("Status code distribution:");

    let mut status_codes: Vec<u16> = final_stats.status_counts.keys().cloned().collect();
    status_codes.sort();

    for status in status_codes {
        let count = final_stats.status_counts.get(&status).unwrap();
        println!("  HTTP {}: {}", status, count);
    }

    println!(
        "\nNow run the blamo-web-throughput tool with similar parameters and compare results:"
    );
    println!("cargo run -- http://localhost:8080 -n 10 -c 5");

    Ok(())
}
