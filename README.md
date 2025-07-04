# WhambBam

A feature-rich CLI tool for testing HTTP(S) endpoint throughput with an interactive terminal UI.

Visit [whambam.dev](https://whambam.dev) for more information.

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
cargo install whambam
```

Or install from source:

```
cargo install --path .
```

## Project Structure

```
.
├── src
│   ├── main.rs            # Entry point with command-line argument parsing
│   ├── tester
│   │   ├── mod.rs        # Tester module exports
│   │   ├── runner.rs     # Test runner implementation
│   │   └── types.rs      # Core data types and shared state
│   ├── ui
│   │   ├── mod.rs        # UI module exports
│   │   ├── app.rs        # Terminal UI application
│   │   └── widgets.rs    # UI components and layouts
│   └── tests             # Test modules
└── Cargo.toml            # Rust project dependencies

## Usage

```
whambam <URL> [OPTIONS]
```

### Options

- `-n, --requests <REQUESTS>`: Number of requests to send (default: 200, 0 for unlimited)
- `-c, --concurrent <CONCURRENT>`: Number of concurrent connections (default: 50)
- `-z, --duration <DURATION>`: Duration to send requests (default: 0 for unlimited)  
  When duration is reached, application stops and exits.  
  If duration is specified, `-n` is ignored.  
  Supports time units: `-z 10s` (seconds), `-z 3m` (minutes), `-z 1h` (hours)
- `-t, --timeout <SECONDS>`: Timeout for each request in seconds (default: 20, 0 for infinite)
- `-q, --rate-limit <RATE>`: Rate limit in queries per second (QPS) per worker (default: 0, no limit)
- `-m, --method <METHOD>`: HTTP method to use (GET, POST, PUT, DELETE, HEAD, OPTIONS) (default: GET)
- `-A, --accept <HEADER>`: HTTP Accept header
- `-a, --auth <AUTH>`: Basic authentication in username:password format
- `-d, --body <BODY>`: HTTP request body
- `-D, --body-file <FILE>`: HTTP request body from file
- `-H, --header <HEADER>`: Custom HTTP headers (can be specified multiple times)
- `-T, --content-type <TYPE>`: Content-Type header (default: "text/html")
- `-x, --proxy <PROXY>`: HTTP Proxy address as host:port
- `--disable-compression`: Disable compression
- `--disable-keepalive`: Disable keep-alive, prevents re-use of TCP connections between different HTTP requests
- `--disable-redirects`: Disable following of HTTP redirects
- `-o, --output <FORMAT>`: Output format (default: ui)
  - `ui`: Interactive terminal UI with real-time statistics
  - `hey`: Text summary in a format similar to the hey tool
- `-h, --help`: Print help
- `-V, --version`: Print version

**Note:** Total number of requests cannot be less than concurrency level. If specified with `-n`, it will be automatically increased to match the concurrency level.

### Examples

```
# Basic usage with 500 requests and 50 concurrent connections
whambam https://example.com -n 500 -c 50

# POST request with JSON body and custom headers
whambam https://api.example.com/users -m POST -d '{"name":"Test User"}' -H "Content-Type: application/json" -H "Authorization: Bearer token123"

# Time-limited test with custom timeout and rate limiting
whambam https://example.com -z 30s -t 5 -q 100 -c 10

# Using basic authentication
whambam https://secure-site.com -a username:password
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
- `h` or `?`: Toggle help overlay
- `q` or `ESC`: Quit the application (immediately returns to shell prompt)

## Local Testing

For local testing, you can easily set up a simple HTTP server using Node.js:

1. Install the http-server package:

```
brew install http-server
```

2. Start the server in your current directory:

```
http-server .
```

3. Test against the local server:

```
whambam http://localhost:8080 -n 100 -c 10
```

This provides a quick and easy way to test your installation and experiment with different options without making external requests.

## Final Report

After completion, a detailed summary is displayed:
- Total test duration and requests
- Average throughput (requests/sec)
- Latency statistics (min, max, p50, p90, p99)
- Complete HTTP status code distribution

## Contributing

Contributions are welcome! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more information.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.