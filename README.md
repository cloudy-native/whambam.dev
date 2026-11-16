# whambam

Fast HTTP(S) load testing with a live terminal UI and hey-compatible flags.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/cloudy-native/whambam.dev/actions/workflows/ci.yml/badge.svg)](https://github.com/cloudy-native/whambam.dev/actions/workflows/ci.yml)
[![Release](https://github.com/cloudy-native/whambam.dev/actions/workflows/release.yml/badge.svg)](https://github.com/cloudy-native/whambam.dev/actions/workflows/release.yml)

**Documentation and examples: [whambam.dev](https://whambam.dev)**

![whambam demo](./docs/images/demo.gif)

## Why whambam?

[hey](https://github.com/rakyll/hey) is no longer maintained. whambam picks up where it left off:

- **hey-compatible flags** for the common cases (`-n -c -z -q -m -H -d -D -A -T -a -x -o csv`), so scripts keep working.
- **A live dashboard**: throughput and latency on one timeline, percentiles, status codes and errors as they happen.
- **Honest numbers**: HDR-histogram percentiles, end-to-end latency (through the last body byte), and `--qps` runs corrected for coordinated omission.
- **Steer the load live**: change workers and request rate from the keyboard while the test runs, and find the knee in one session.
- **Catch regressions**: compare with a saved baseline, and fail CI with checks like `--assert "p99<200ms,errors<1%"` or `--assert "p99<+10%"`.
- **Machine-readable output**: JSON summaries with a [published schema](schema/report.schema.json), and per-request CSV.
- **One static binary** for macOS, Linux and Windows on x86_64 and ARM64. Written in Rust.

## Install

```bash
brew install cloudy-native/whambam/whambam          # macOS and Linux
cargo binstall whambam                              # prebuilt binary via cargo-binstall
cargo install whambam                               # from source
scoop install https://github.com/cloudy-native/whambam.dev/releases/latest/download/whambam.json   # Windows
```

Or download a binary, `.deb` or `.zip` from the [releases page](https://github.com/cloudy-native/whambam.dev/releases).

## Quick start

```bash
whambam https://example.com                          # 200 requests, 50 workers, live UI
whambam https://example.com -n 10k -c 100            # 10,000 requests
whambam https://example.com -z 30s --qps 500         # 500 req/s for 30 s, latency corrected
whambam https://api.example.com/users -m POST \
  -d '{"name":"Ada"}' -T application/json            # POST a JSON body
whambam https://example.com -z 10s --no-ui -o json --output report.json
whambam https://example.com --debug                  # send one request and show it
whambam https://example.com -z 30s --assert 'p99<200ms,errors<1%'   # exit 1 if the checks fail
```

When stdout is not a terminal (piped or redirected), whambam skips the UI and prints the report.

## Options

### Load

| Option | Description | Default |
|---|---|---|
| `-n, --requests <N>` | Requests to send. Accepts `10k`, `1m`. `0` = until stopped. Ignored with `-z`. | 200 |
| `-c, --concurrency <N>` | Workers (concurrent connections) | 50 |
| `-z, --duration <TIME>` | Run for a time instead of a count: `30s`, `5m`, `1h`, `500ms` | – |
| `-q, --rate-limit <QPS>` | Rate limit **per worker**, as in hey (total ≈ `-q` × `-c`) | unlimited |
| `--qps <QPS>` | **Total** request rate, with latency corrected for coordinated omission | – |
| `-t, --timeout <TIME>` | Per-request timeout (`0` = none) | 20s |
| `-p, --http2-parallel <N>` | HTTP/2 streams per connection (needs `--http2`) | 1 |

### Request

| Option | Description | Default |
|---|---|---|
| `-m, --method <METHOD>` | HTTP method | GET |
| `-H, --header <H>` | Header, repeatable: `-H "X-Id: 1"` | – |
| `-d, --body <BODY>` | Request body | – |
| `-D, --body-file <FILE>` | Request body from a file (binary safe) | – |
| `-A, --accept <TYPE>` | Accept header | – |
| `-T, --content-type <TYPE>` | Content-Type, sent with a body | text/html |
| `-a, --auth <USER:PASS>` | Basic authentication | – |
| `--host <HOST>` | Override the Host header | – |
| `-U, --user-agent <UA>` | User-Agent | whambam/&lt;version&gt; |

### Connection

| Option | Description |
|---|---|
| `-x, --proxy <PROXY>` | Proxy as `host:port` or a URL (`http://`, `socks5://`) |
| `--http2`, `--http-version <1.1\|2>` | Protocol (HTTP/2 uses prior knowledge on `http://`) |
| `-k, --insecure` | Accept invalid TLS certificates |
| `--cacert <PEM>` | Extra CA certificate to trust |
| `--cert <PEM> --key <PEM>` | Client certificate for mutual TLS |
| `--connect-timeout <TIME>` | Timeout for opening a connection |
| `--connect-to <H:P:H2:P2>` | Send traffic for `H:P` to `H2:P2`, keeping the Host header and TLS name |
| `-4, --ipv4` / `-6, --ipv6` | Resolve to one address family |
| `--disable-keepalive` | New connection per request |
| `--disable-compression` | Don't ask for gzip/brotli/deflate/zstd |
| `--disable-redirects`, `-r, --redirect <N>` | Redirect limit (default 10) |

### Output

| Option | Description | Default |
|---|---|---|
| `--no-ui` (alias `--no-tui`) | No interactive UI; progress on stderr, report on stdout | off |
| `-o, --output-format <FMT>` | `text`, `json`, `csv` (one row per request) or `quiet` | text |
| `--output <FILE>` | Write the report to a file instead of stdout | stdout |
| `--debug` | Send one request, print the request and response, exit | – |
| `--no-color` | No colors (also honours `NO_COLOR`) | – |
| `--fps <N>` | UI refresh rate | 10 |

### Checks

| Option | Description |
|---|---|
| `--baseline <FILE>` | Compare with a saved report (`-o json`): deltas in the UI, a comparison in the report |
| `--assert <CHECK>` | Exit with status 1 unless the run meets the check; repeatable or comma-separated |

Checks are `metric` `op` `value`, with `op` one of `<` `<=` `>` `>=`:

| Metric | Values | Examples |
|---|---|---|
| `p50` `p90` `p95` `p99` `p99.9` `mean` `min` `max` | latency: `200ms`, `1.5s` (bare numbers are ms) | `p99<200ms`, `max<=2s` |
| `rps` | requests per second | `rps>=1000` |
| `success`, `errors` | percent of requests | `errors<0.5%`, `success>=99.9` |
| any | change from `--baseline`: `+N%` or `-N%` | `p99<+10%`, `rps>-5%` |

`whambam compare BASE.json NEW.json [--assert ...] [-o json]` compares two saved reports without running a test.

## Live load controls

While a test runs:

| Key | Effect |
|---|---|
| `+` / `-` (or `↑` / `↓`) | 10% more or fewer workers |
| `]` / `[` | 10% higher or lower rate (`--qps` total, or `-q` per worker) |

Each change is marked on the charts and listed in the report (`adjustments` in JSON), so you can raise the load step by step and see where latency turns. With `--qps`, a new rate takes effect immediately; requests already queued under the old rate are re-timed.

## Baselines and CI

```bash
whambam https://staging.example.com -z 30s --qps 200 --no-ui -o json --output baseline.json
# ...ship a change...
whambam https://staging.example.com -z 30s --qps 200 --baseline baseline.json \
  --assert 'p99<+10%,errors<1%'
```

In GitHub Actions, use the bundled action. It installs the right binary for the runner, writes a JSON report, adds a summary to the job page and fails the job when a check fails:

```yaml
- uses: cloudy-native/whambam.dev@v1
  with:
    url: https://staging.example.com/health
    args: -z 30s -c 20 --qps 200
    assert: p99<250ms,errors<1%
    baseline: baseline.json   # optional
```

## Differences from hey

- **`-q` keeps hey's meaning** (per worker). For a target total rate use `--qps`, which also corrects for coordinated omission: each request is timed from when it was *due*, so a server stall shows up in the percentiles instead of hiding in fewer requests.
- **Latency is end to end**: from sending the request to reading the last byte of the body.
- **No per-phase breakdown** (DNS, dial, write, wait). whambam doesn't measure those phases, so it doesn't print guesses.
- **Success** means a status below 400. Transport errors are grouped by kind (timeout, DNS, refused, TLS, …) instead of being printed one per line.
- **JSON output** is new; `-o csv` columns are `offset_s,latency_ms,status,error,bytes_received`.

## Interactive UI

| Key | Action |
|---|---|
| `1` `2` `3`, `←` `→`, `Tab` | Dashboard · Charts · Status & errors |
| `+` `-` / `↑` `↓` | More or fewer workers while running |
| `]` `[` | Higher or lower rate while running (`--qps` / `-q`) |
| `r` | Run the same test again (after it finishes) |
| `h`, `?` | Help |
| `q`, `Esc`, `Ctrl-C` | Quit and print the report |

## Try it locally

```bash
python3 -m http.server 8080                 # any local server works
whambam http://localhost:8080 -z 10s -c 50
```

The repo also has a demo server with realistic, wave-shaped latency (it's what the GIF above uses):

```bash
cargo run --release -p test-utils --bin demo-server -- 8080
```

## Architecture

```
src/
├── main.rs        entry point
├── lib.rs         run(): compare, debug, UI or headless mode, then the report
├── checks.rs      --baseline, --assert and `whambam compare`
├── cli/           clap flags (load, request, connection, output) and value parsers
├── config/        validated TestConfig: request spec, network settings, pacing
├── http/          reqwest clients, DNS resolver (--connect-to, -4/-6), request execution, error kinds, --debug
├── engine/        worker pool, live controls, shared schedule (budget, deadlines, rate pacing)
├── stats/         live TestState: HDR histogram, status/error counts, chart series
├── report/        Report model → text, JSON; streaming CSV; metrics, baselines, assertions
└── ui/            ratatui dashboard, charts, status & errors
test-utils/        HTTP/1.1 + h2c mock server for tests, demo server for recordings
schema/            JSON schema for -o json
packaging/         Homebrew formula and Scoop manifest renderer, Action job summary
action.yml         GitHub Action
```

## Development

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
demo/record.sh     # re-record docs/images/demo.gif with vhs
```

Releases build natively on GitHub-hosted runners for macOS, Linux (glibc and static musl) and Windows, each on x86_64 and ARM64. See [CONTRIBUTING.md](CONTRIBUTING.md#releasing).

## Contributing

Contributions are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT; see [LICENSE](LICENSE).

## Acknowledgments

- Inspired by [hey](https://github.com/rakyll/hey).
- Built with [Claude Code](https://www.anthropic.com/claude-code); see [how](CONTRIBUTING.md#ai-assisted-development).
