use anyhow::{Context, Result};
use clap::Parser;
use std::sync::{Arc, Mutex};
use url::Url;

mod tester;
mod ui;
mod debug;

use tester::{SharedState, TestConfig, TestRunner, TestState};
use ui::App;

#[derive(Parser, Clone)]
#[command(author, version, about = "Test the throughput of an HTTP(S) endpoint")]
struct Args {
    /// The URL to test
    #[arg(required = true)]
    url: String,

    /// Number of requests to send (0 for unlimited)
    #[arg(short = 'n', long, default_value = "200")]
    requests: usize,

    /// Number of concurrent connections
    #[arg(short, long, default_value = "50")]
    concurrent: usize,
    
    /// Duration of the test in seconds (0 for unlimited)
    #[arg(short, long, default_value = "0")]
    duration: u64,
    
    /// Rate limit in queries per second (QPS) per worker (0 for no limit)
    #[arg(short = 'q', long, default_value = "0")]
    rate_limit: f64,
    
    /// Run in debug mode to diagnose HTTP request issues
    #[arg(long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    let url = Url::parse(&args.url).context("Invalid URL")?;

    println!("Starting throughput test for: {}", url);
    println!("Requests: {}, Concurrent: {}", 
             if args.requests > 0 { args.requests.to_string() } else { "Unlimited".to_string() },
             args.concurrent);
    println!("Duration: {} seconds", 
             if args.duration > 0 { args.duration.to_string() } else { "Unlimited".to_string() });
    println!("Press Ctrl+C to stop the test\n");
    
    // Ensure number of requests is not less than concurrency level
    let requests = if args.requests > 0 && args.requests < args.concurrent {
        println!("Warning: Increasing request count to match concurrency level ({}).", args.concurrent);
        args.concurrent
    } else {
        args.requests
    };
    
    if args.debug {
        // Run in debug mode
        println!("Running in debug mode...");
        debug::run_debug_test(&args.url, requests, args.concurrent).await?
    } else {
        // Normal mode with UI
        let config = TestConfig {
            url: args.url.clone(),
            requests,
            concurrent: args.concurrent,
            duration: args.duration,
            rate_limit: args.rate_limit,
        };
        
        // Create a shared state first
        let state = Arc::new(Mutex::new(TestState::new(&config)));
        
        // Create the UI app using a direct reference to the shared state
        let shared_state = SharedState { state: Arc::clone(&state) };
        let mut app = App::new(shared_state);
        
        // Start the test in a separate task, but only move the config
        let config_clone = config.clone();
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            // Create a test runner inside the task with the shared state
            let mut runner = TestRunner::with_state(config_clone, SharedState { state: state_clone });
            let _ = runner.start().await;
        });
        
        // Don't block the main thread on app.run() so we don't deadlock
        tokio::spawn(async move {
            // This will run the UI in a separate task
            if let Err(e) = app.run() {
                eprintln!("UI error: {:?}", e);
            }
        });
        
        // Keep the main thread alive
        tokio::signal::ctrl_c().await?;
        println!("Shutting down...")
    };
    
    Ok(())
}