# Blamo Web Throughput

A feature-rich CLI tool for testing HTTP(S) endpoint throughput with an interactive terminal UI.

## Features

- Test the throughput of any HTTP(S) endpoint
- Configure number of requests and concurrent connections
- Interactive terminal dashboard with real-time metrics
- Live charts for throughput and latency visualization
- Detailed breakdown by HTTP status code
- Track successful vs failed requests with percentiles
- Optional rate limiting for controlled load testing

## Installation

```
cargo install --path .
```

## Usage

```
blamo-web-throughput <URL> [OPTIONS]
```

### Options

- `-n, --requests <REQUESTS>`: Number of requests to send (default: 200, 0 for unlimited)
- `-c, --concurrent <CONCURRENT>`: Number of concurrent connections (default: 50)
- `-q, --rate-limit <RATE>`: Rate limit in queries per second (QPS) per worker (default: 0, no limit)
- `-z, --duration <DURATION>`: Duration to send requests (default: 0 for unlimited)  
  When duration is reached, application stops and exits.  
  If duration is specified, `-n` is ignored.  
  Supports time units: `-z 10s` (seconds), `-z 3m` (minutes), `-z 1h` (hours)
- `-o, --output <FORMAT>`: Output format (default: ui)
  - `ui`: Interactive terminal UI with real-time statistics
  - `hey`: Text summary in a format similar to the hey tool
- `--debug`: Run in debug mode with detailed output (no UI)
- `-h, --help`: Print help
- `-V, --version`: Print version

**Note:** Total number of requests cannot be less than concurrency level. If specified with `-n`, it will be automatically increased to match the concurrency level.

### Example

```
blamo-web-throughput https://example.com -n 500 -c 50
```

## Interactive UI

The tool features a rich terminal UI with:

1. **Dashboard Tab**
   - Real-time throughput and request statistics
   - Live charts for throughput and latency trends
   - Key performance metrics (requests/sec, success rate)
   
2. **Charts Tab**
   - Full-size charts for detailed visualization
   - Time-series data for throughput and latency
   
3. **Status Codes Tab**
   - Live breakdown of all HTTP status codes
   - Percentage distribution of responses
   - Color-coded by status class (2xx, 3xx, 4xx, 5xx)

### Keyboard Controls

- `1`, `2`, `3`: Switch between tabs
- `h`: Toggle help overlay
- `q` or `ESC`: Quit the application

## Debug Mode

When troubleshooting, use the `--debug` flag for simplified output:

```
blamo-web-throughput https://example.com -n 10 -c 5 --debug
```

This bypasses the UI and provides direct output for each request, useful for diagnosing connection issues.

## Development Tools

The project includes additional tools for development:

- **Debug HTTP Server**: A simple server that logs incoming requests (`node debug_server.js`)
- **Debug Helper**: A standalone HTTP client for testing connections (`cargo run --bin debug_helper`)

## Final Report

After completion, a detailed summary is displayed:
- Total test duration and requests
- Average throughput (requests/sec)
- Latency statistics (min, max, p50, p90, p99)
- Complete HTTP status code distribution