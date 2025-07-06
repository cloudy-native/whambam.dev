# Whambam 🚀

An open-source, unobtrusive,lightning-fast CLI tool for HTTP(S) endpoint performance testing with a handy interactive terminal UI.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[![CI](https://github.com/cloudy-native/whambam.dev/actions/workflows/ci.yml/badge.svg)](https://github.com/cloudy-native/whambam.dev/actions/workflows/ci.yml)
[![Release](https://github.com/cloudy-native/whambam.dev/actions/workflows/release.yml/badge.svg)](https://github.com/cloudy-native/whambam.dev/actions/workflows/release.yml)

**Visit [whambam.dev](https://whambam.dev) for comprehensive documentation and examples.**

![Whambam Terminal UI](./docs/images/ui.png)

## Why Whambam?

```
Warning: Some installed formulae are deprecated or disabled.
You should find replacements for the following formulae:
  hey
```

The beloved HTTP testing tool [hey](https://github.com/rakyll/hey) is no longer maintained, leaving a gap in the developer toolkit. Whambam fills that void:

- Has **Drop-in compatibility** with hey's command-line arguments
- Adds **Modern interactive UI** with real-time metrics and charts
- Is **Actively maintained** and receives continuous improvements
- Is a **clean-room implementation** in Rust for reliability and performance

Built with the same terminal-focused philosophy that made hey popular, but designed for the modern development workflow.

## ✨ Key Features

### Performance Testing
- **Blazing fast** HTTP(S) endpoint testing
- **Configurable concurrency** and request counts
- **Rate limiting** for controlled load testing
- **Multiple HTTP methods** (GET, POST, PUT, DELETE, HEAD, OPTIONS)
- **Custom headers and authentication** support

### Interactive Dashboard
- **Real-time metrics** with live updates
- **Beautiful charts** for throughput and latency visualization
- **Status code breakdown** with color-coded responses
- **Multiple view modes** (Dashboard, Charts, Status Codes)

### Developer Experience
- **hey-compatible** command-line interface
- **Flexible output formats** (interactive UI or text summary)
- **Comprehensive error handling** and timeout controls
- **Proxy support** for complex network setups

## 📦 Installation

### Homebrew (Recommended)
```bash
# Add tap and install
brew tap cloudy-native/whambam
brew install whambam

# Or install directly
brew install cloudy-native/whambam/whambam
```

### From Source
```bash
git clone https://github.com/cloudy-native/whambam.dev.git
cd whambam.dev
cargo build --release
```

## 🚀 Quick Start

```bash
# Basic performance test
whambam https://example.com

# Custom configuration
whambam https://example.com -n 1000 -c 20

# POST request with JSON payload
whambam https://api.example.com/users \
  -m POST \
  -d '{"name":"Test User"}' \
  -H "Content-Type: application/json"

# Time-limited test with rate limiting
whambam https://example.com -z 30s -q 100 -c 10

# hey-compatible text output
whambam https://example.com -n 100 -c 10 --output hey
```

## 📖 Usage Reference

```bash
whambam <URL> [OPTIONS]
```

### Core Options
| Option | Description | Default |
|--------|-------------|---------|
| `-n, --requests <N>` | Number of requests to send | 200 |
| `-c, --concurrent <N>` | Concurrent connections | 50 |
| `-z, --duration <TIME>` | Test duration (e.g., 30s, 5m, 1h) | unlimited |
| `-t, --timeout <SEC>` | Request timeout in seconds | 20 |
| `-q, --rate-limit <QPS>` | Rate limit (queries per second) | unlimited |

### HTTP Configuration
| Option | Description | Default |
|--------|-------------|---------|
| `-m, --method <METHOD>` | HTTP method | GET |
| `-d, --body <BODY>` | Request body | - |
| `-D, --body-file <FILE>` | Request body from file | - |
| `-H, --header <HEADER>` | Custom headers (repeatable) | - |
| `-A, --accept <HEADER>` | Accept header | - |
| `-T, --content-type <TYPE>` | Content-Type header | text/html |
| `-a, --auth <USER:PASS>` | Basic authentication | - |

### Network Options
| Option | Description |
|--------|-------------|
| `-x, --proxy <HOST:PORT>` | HTTP proxy |
| `--disable-compression` | Disable compression |
| `--disable-keepalive` | Disable connection reuse |
| `--disable-redirects` | Disable redirect following |

### Output Options
| Option | Description |
|--------|-------------|
| `-o, --output <FORMAT>` | Output format: `ui` (default) or `hey` (text) |

## 🎯 Interactive UI Guide

### Navigation
- **`1`, `2`, `3`**: Switch between Dashboard, Charts, and Status Codes tabs
- **`h` or `?`**: Toggle help overlay
- **`Ctrl-C`, `q`, or `ESC`**: Exit application

### Dashboard Tab
Real-time performance metrics including:
- **Throughput**: Requests per second
- **Success Rate**: Percentage of successful requests
- **Response Times**: Min, max, and average latency
- **Live Charts**: Visual representation of performance trends

### Charts Tab
Full-screen visualization of:
- **Throughput over time**
- **Latency distribution**
- **Request completion trends**

### Status Codes Tab
Detailed breakdown of HTTP responses:
- **Color-coded by status class** (2xx, 3xx, 4xx, 5xx)
- **Percentage distribution**
- **Real-time updates**

## 🧪 Local Testing Setup

Quickly test your installation with a local HTTP server:

```bash
# Install and start a simple HTTP server
brew install http-server
http-server .

# Test against local server
whambam http://localhost:8080 -n 100 -c 10
```

## 🏗️ Architecture

```
src/
├── main.rs              # CLI parsing and application entry point
├── tester/
│   ├── runner.rs        # Async HTTP test execution engine
│   └── types.rs         # Core data structures and shared state
├── ui/
│   ├── app.rs          # Terminal UI application logic
│   └── widgets.rs      # UI components and layouts
└── tests/              # Comprehensive test suite
    ├── cli_tests.rs
    ├── runner_tests.rs
    └── mock_server.rs
```

## 🤖 AI-Powered Development

This project was built using [Claude Code](https://www.anthropic.com/claude-code), demonstrating effective AI-assisted development practices:

### Key Learnings
1. **Always start with a plan** - Get AI to outline the approach before coding
2. **Make incremental changes** - Small, testable steps prevent large rollbacks
3. **Write tests eagerly** - Add comprehensive tests before major refactoring
4. **Leverage AI for systemic changes** - AI excels at large-scale code transformations
5. **Iterate and improve** - Don't hesitate to ask AI to fix its own mistakes

### Development Stats
- **Total AWS Bedrock cost**: $34.92
- **Model used**: `us.anthropic.claude-3-7-sonnet-20250219-v1:0`
- **Code split**: ~30% human-written structure, ~70% AI-generated implementation
- **Test coverage**: Comprehensive suite with AI-generated tests

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork the repository**
2. **Create a feature branch** (`git checkout -b feature/amazing-feature`)
3. **Make your changes** with tests
4. **Run the test suite** (`cargo test`)
5. **Submit a pull request**

Please read our [Contributing Guidelines](CONTRIBUTING.md) for detailed information.

## 📋 Roadmap

- [ ] **WebSocket support** for real-time protocol testing
- [ ] **Custom report generation** with exportable metrics
- [ ] **Plugin system** for extended functionality
- [ ] **Distributed testing** across multiple machines
- [ ] **Performance regression detection**

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by the excellent [hey](https://github.com/rakyll/hey) tool
- Built with [Claude Code](https://www.anthropic.com/claude-code) AI assistance
- Thanks to the Rust community for excellent HTTP libraries

---

**Ready to test your APIs?** Install whambam and start performance testing in seconds!

```bash
brew install cloudy-native/whambam/whambam
whambam https://your-api.com
```