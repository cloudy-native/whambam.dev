# Why?

You can't be serious...another web performance testing tool? You're right to roll your eyes, but give us a chance to justify why we made whambam.

```bash
$ brew doctor
Warning: Some installed formulae are deprecated or disabled.
You should find replacements for the following formulae:
    hey
$ brew info hey
==> hey: stable 0.1.4 (bottled)
HTTP load generator, ApacheBench (ab) replacement
https://github.com/rakyll/hey
Deprecated because it is not maintained upstream! It will be disabled on 2026-01-12.
```

Wait, what? But we love [hey](https://github.com/rakyll/hey)! That's too bad.

We looked around and there are more web testing tools than you can imagine. But it's hard to find a tool that's a good replacement for hey, which has the ideal mix of speed, simplicity, and features.

Although one of the things we liked looking at alternatives was a way to see progress. We're not signing up for a full UI, but can we do something simpler and still have it feel like a terminal application?

We gave it a shot and we hope you like what we came up with.

Here's our design and implementation focus.

- As fast as `hey`
- Command-line argument compatibility with `hey`
- A simple progress UI
- A cleanroom implementation

# Install

If you're on a Mac and using homebrew, just do this.

```bash
brew tap cloudy-native/whambam
brew install whambam
```

If you're not using homebrew, this is your chance and it will definitely be worth your while. Follow instructions at [brew.sh](https://brew.sh).

Linux releases are available at [GitHub releases](https://github.com/cloudy-native/whambam.dev/releases) in tarballs and Debian packages. Requests for alternative packaging are welcome.

_Windows releases are coming soon!_

# Usage

Get started with whambam by pointing it at a URL and letting good defaults do the rest.

```bash
whambam -z 10s https://example.com
```

Will pummel `https://example.com` for 10 seconds with the default 50 concurrent connections.

## Core Options

| Option                     | Description                          | Default   | Examples & Explanation                                                                 |
|----------------------------|--------------------------------------|-----------|-----------------------------------------------------------------------------------------|
| `-n, --requests <N>`       | Number of requests to send           | 200       | `-n 1000` sends exactly 1000 requests. The test ends when all requests are complete. Cannot be used with `-z`. |
| `-c, --concurrent <N>`     | Concurrent connections               | 50        | `-c 100` simulates 100 users making requests simultaneously.                            |
| `-z, --duration <TIME>`    | Test duration (e.g., 30s, 5m, 1h)    | unlimited | `-z 1m` runs the test for exactly 1 minute. Cannot be used with `-n`.                  |
| `-t, --timeout <SEC>`      | Request timeout in seconds           | 20        | `-t 5` aborts any request that takes longer than 5 seconds.                            |
| `-q, --rate-limit <QPS>`   | Rate limit (queries per second)      | unlimited | `-q 100` attempts to send 100 requests per second. If `-c` is too low, the actual rate may be lower. |

## HTTP Configuration

| Option                      | Description                                                                 | Default   | Examples & Explanation                                                                 |
|-----------------------------|-----------------------------------------------------------------------------|-----------|-----------------------------------------------------------------------------------------|
| `-m, --method <METHOD>`     | HTTP method (`GET`, `POST`, `PUT`, `DELETE`, `HEAD`, `OPTIONS`)            | GET       | `-m POST` sends a POST request. Usually used with `-d` or `-D`.                         |
| `-d, --body <BODY>`         | Request body                                                                | -         | `-d '{"key":"value"}'` sends the given JSON string as the request body.              |
| `-D, --body-file <FILE>`    | Request body from file                                                      | -         | `-D /path/to/body.json` sends the contents of the file as the request body.            |
| `-H, --header <HEADER>`     | Custom headers (repeatable)                                                 | -         | `-H 'X-My-Header: 123' -H 'User-Agent: whambam'`                                       |
| `-A, --accept <HEADER>`     | Accept header                                                               | -         | `-A 'application/json'` sets the Accept header.                                        |
| `-T, --content-type <TYPE>` | Content-Type header                                                         | text/html | `-T 'application/json'`. Important when sending a request body.                        |
| `-a, --auth <USER:PASS>`    | Basic authentication                                                        | -         | `-a admin:s3cr3t` sends an Authorization header with the credentials.                  |

## Network Options

| Option                        | Description                    | Examples & Explanation                                                          |
|-------------------------------|--------------------------------|----------------------------------------------------------------------------------|
| `-x, --proxy <HOST:PORT>`     | HTTP proxy                     | `-x http://127.0.0.1:8080` routes all requests through the specified proxy.     |
| `--disable-compression`       | Disable compression            | Prevents whambam from requesting compressed responses (e.g., gzip).             |
| `--disable-keepalive`         | Disable connection reuse       | Forces a new TCP connection for each request. Simulates clients without keep-alive. |
| `--disable-redirects`         | Disable redirect following     | If the server returns a 3xx redirect, whambam will not follow it.               |

## Output Options

| Option                    | Description                                                                                             | Default |
|---------------------------|---------------------------------------------------------------------------------------------------------|---------|
| `-o, --output <FORMAT>`   | Output format: `ui` for a simple terminal UI or `hey` for mostly hey-compatible text output            | `ui`    |

Note: temporarily disabled in current version. `-o hey` is useful for scripting or logging, as it prints a simple text summary.

## Interactive UI Guide

The interactive UI provides real-time feedback on your load test. Use these keys to navigate:

### Navigation

- `1`, `2`, `3`: Switch between Dashboard, Charts, and Status Codes tabs
- `h` or `?`: Toggle help overlay
- `Ctrl-C`, `q`, or `ESC`: Exit application

### Dashboard Tab

![Dashboard tab](/images/ui-tab-1.png)

Real-time performance metrics including:

- Throughput: Requests per second
- Success Rate: Percentage of successful requests
- Response Times: Min, max, and average latency
- Live Charts: Visual representation of performance trends

### Charts Tab

![Charts tab](/images/ui-tab-2.png)

Full-screen visualization of:

- Throughput over time
- Latency distribution
- Request completion trends

### Status Codes Tab

![Status Codes tab](/images/ui-tab-3.png)

Detailed breakdown of HTTP responses:

- Color-coded by status class (2xx, 3xx, 4xx, 5xx)
- Percentage distribution
- Real-time updates
