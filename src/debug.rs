use anyhow::Result;
use reqwest::Client;
use std::time::Instant;
use tokio::sync::mpsc;
use futures::{stream, StreamExt};

/// Function to test just the HTTP request functionality
pub async fn run_debug_test(url: &str, requests: usize, concurrent: usize) -> Result<()> {
    let client = Client::new();
    let mut successful = 0;
    let mut failures = 0;
    let mut status_counts = std::collections::HashMap::new();
    
    println!("=== Debug Test ===");
    println!("URL: {}", url);
    println!("Requests: {}, Concurrent: {}", requests, concurrent);

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
    let stream = stream::iter(0..requests)
        .map(|i| {
            let client = &client;
            let url_str = url.to_string();
            let tx = tx.clone();
            
            async move {
                println!("Sending request {}", i);
                let result = client.get(&url_str)
                    .header("X-Debug-ID", i.to_string())
                    .send()
                    .await;
                
                match result {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        tx.send((status, false)).await.unwrap();
                    },
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