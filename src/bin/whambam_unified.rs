use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::fs;
use std::path::Path;
use std::time::Instant;
use url::Url;

use whambam::tester::{
    print_hey_format_report, unified_print_final_report, HttpMethod, SharedMetrics, TestConfig,
    UnifiedRunner,
};

#[derive(Parser, Clone, Debug)]
#[command(
    author,
    version,
    about = "HTTP load testing tool with unified optimizations"
)]
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

    /// Duration of test (examples: 10s, 5m, 2h)
    #[arg(short = 'z', long)]
    duration: Option<String>,

    /// Legacy duration string support
    #[arg(long, default_value = "0", hide = true)]
    duration_str: String,

    /// Rate limit in queries per second (QPS) per worker (0 for no limit)
    #[arg(short = 'q', long, default_value = "0")]
    rate_limit: f64,

    /// HTTP method to use (GET, POST, PUT, DELETE, HEAD, OPTIONS)
    #[arg(short = 'm', long, default_value = "GET")]
    method: String,

    /// HTTP Accept header
    #[arg(short = 'A', long)]
    accept: Option<String>,

    /// Basic authentication in username:password format
    #[arg(short = 'a', long)]
    auth: Option<String>,

    /// Request body as a string
    #[arg(short = 'd', long)]
    body: Option<String>,

    /// Request body from a file
    #[arg(short = 'D', long)]
    body_file: Option<String>,

    /// Custom HTTP headers (can specify multiple)
    #[arg(short = 'H', long, action = clap::ArgAction::Append)]
    headers: Vec<String>,

    /// Content-Type header
    #[arg(short = 'T', long = "content-type", default_value = "")]
    content_type: String,

    /// HTTP proxy as host:port
    #[arg(short = 'x', long)]
    proxy: Option<String>,

    /// Request timeout in seconds
    #[arg(short = 't', long, default_value = "20")]
    timeout: u64,

    /// Disable HTTP compression
    #[arg(long)]
    disable_compression: bool,

    /// Disable HTTP keep-alive
    #[arg(long)]
    disable_keepalive: bool,

    /// Disable following HTTP redirects
    #[arg(long)]
    disable_redirects: bool,

    /// Output format (ui or hey)
    #[arg(short = 'o', long, default_value = "hey")]
    output_format: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    let url = Url::parse(&args.url).context("Invalid URL")?;

    // Parse duration string
    let duration_secs = if let Some(duration_str) = args.duration {
        match duration_str.chars().last() {
            Some('s') => {
                let num: String = duration_str.chars().take(duration_str.len() - 1).collect();
                num.parse::<u64>()?
            }
            Some('m') => {
                let num: String = duration_str.chars().take(duration_str.len() - 1).collect();
                num.parse::<u64>()? * 60
            }
            Some('h') => {
                let num: String = duration_str.chars().take(duration_str.len() - 1).collect();
                num.parse::<u64>()? * 3600
            }
            _ => args.duration_str.parse::<u64>().unwrap_or(0),
        }
    } else {
        0
    };

    // Parse HTTP method
    let method = match args.method.to_uppercase().as_str() {
        "GET" => HttpMethod::GET,
        "POST" => HttpMethod::POST,
        "PUT" => HttpMethod::PUT,
        "DELETE" => HttpMethod::DELETE,
        "HEAD" => HttpMethod::HEAD,
        "OPTIONS" => HttpMethod::OPTIONS,
        _ => return Err(anyhow!("Unsupported HTTP method: {}", args.method)),
    };

    // Parse headers
    let mut headers = Vec::new();
    for header in &args.headers {
        if let Some(idx) = header.find(':') {
            let (name, value) = header.split_at(idx);
            headers.push((name.trim().to_string(), value[1..].trim().to_string()));
        }
    }

    // Add default headers
    headers.push(("User-Agent".to_string(), "whambam/0.1.12".to_string()));
    if let Some(accept) = &args.accept {
        headers.push(("Accept".to_string(), accept.clone()));
    }

    // Handle request body (direct or from file)
    let body = match (&args.body, &args.body_file) {
        (Some(content), _) => Some(content.clone()),
        (None, Some(file_path)) => {
            let path = Path::new(file_path);
            if path.exists() {
                Some(fs::read_to_string(path)?)
            } else {
                None
            }
        }
        _ => None,
    };

    // Add content-type header if body is provided
    if body.is_some() && args.content_type != "" {
        headers.push(("Content-Type".to_string(), args.content_type.clone()));
    }

    // Parse basic auth
    let basic_auth = args.auth.as_ref().and_then(|auth_str| {
        auth_str
            .split_once(':')
            .map(|(user, pass)| (user.to_string(), pass.to_string()))
    });

    // Determine request count
    let requests = if duration_secs > 0 {
        // Duration takes precedence
        0
    } else if args.requests == 0 {
        // Default to 200 requests if not specified
        200
    } else {
        args.requests.max(args.concurrent)
    };

    // Create test configuration
    let config = TestConfig {
        url: args.url.clone(),
        method,
        headers: headers.clone(),
        body,
        basic_auth,
        duration: duration_secs,
        requests,
        concurrent: args.concurrent,
        timeout: args.timeout,
        rate_limit: args.rate_limit,
        disable_compression: args.disable_compression,
        disable_keepalive: args.disable_keepalive,
        disable_redirects: args.disable_redirects,
        interactive: args.output_format.to_lowercase() == "ui",
        output_format: args.output_format.clone(),
        content_type: args.content_type.clone(),
        proxy: args.proxy.clone(),
    };

    // Create unified runner with worker pool and lock-free metrics
    let mut runner = UnifiedRunner::new(config);

    // Get metrics before starting
    let metrics = runner.metrics();

    // Print information about the test
    println!("Running unified test with worker pool and lock-free metrics collection");
    println!("URL: {}", url);
    println!("Concurrency: {}", args.concurrent);
    if requests > 0 {
        println!("Number of requests: {}", requests);
    }
    if duration_secs > 0 {
        println!("Duration: {} seconds", duration_secs);
    }
    println!("---");

    // Start the test
    let start = Instant::now();
    runner.start().await?;

    // Monitor progress
    let mut prev_count = 0;
    let mut is_complete = false;

    while !is_complete {
        metrics.process_metrics();
        let current_count = metrics.metrics.completed_requests();

        if current_count > prev_count {
            print!("\rRequests completed: {}   ", current_count);
            prev_count = current_count;
        }

        is_complete = metrics.metrics.is_complete()
            || metrics.metrics.elapsed_seconds() >= duration_secs as f64 + 2.0;
        if !is_complete {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    println!(
        "\nTest completed in {:.2} seconds!",
        start.elapsed().as_secs_f64()
    );

    // Print the final report in the selected format
    if args.output_format.to_lowercase() == "hey" {
        print_hey_format_report(&metrics);
    } else {
        unified_print_final_report(&metrics);
    }

    Ok(())
}
