# Blamo Web Throughput

A feature-rich CLI tool for testing HTTP(S) endpoint throughput with an interactive terminal UI.

## Features

- Test the throughput of any HTTP(S) endpoint
- Configure number of requests and concurrent connections
- Interactive terminal dashboard with real-time metrics
- Live charts for throughput and latency visualization
- Detailed breakdown by HTTP status code
- Track successful vs failed requests with percentiles

## Installation

```
cargo install --path .
```

## Usage

```
blamo-web-throughput <URL> [OPTIONS]
```

### Options

- `-r, --requests <REQUESTS>`: Number of requests to send (default: 1000, 0 for unlimited)
- `-c, --concurrent <CONCURRENT>`: Number of concurrent connections (default: 10)
- `-d, --duration <DURATION>`: Test duration in seconds (default: 0 for unlimited)
- `-h, --help`: Print help
- `-V, --version`: Print version

### Example

```
blamo-web-throughput https://example.com -r 1000 -c 50
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

## Final Report

After completion, a detailed summary is displayed:
- Total test duration and requests
- Average throughput (requests/sec)
- Latency statistics (min, max, p50, p90, p99)
- Complete HTTP status code distribution