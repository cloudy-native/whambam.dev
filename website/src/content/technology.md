whambam is a single Rust binary built on [Tokio](https://tokio.rs), [reqwest](https://docs.rs/reqwest) and [ratatui](https://ratatui.rs). This page explains how a test runs, how whambam keeps its numbers honest, and how the live controls change a test that is already running. The code excerpts are from the current source, trimmed a little for reading.

# The shape of a run

<div class="diagram-wrap"><svg class="diagram" viewBox="0 0 760 370" role="img" aria-labelledby="dg-title dg-desc" xmlns="http://www.w3.org/2000/svg"><title id="dg-title">How a whambam run fits together</title><desc id="dg-desc">Flags become a TestConfig. The engine builds a schedule and a pool of workers. Workers claim slots from the schedule, send requests to your server and report results, in batches, to the TestState. The report is built from the TestState. The terminal UI draws the TestState each frame, and its keys change the Control settings, which the pool applies to the workers while the test runs.</desc><defs><marker id="dg-head" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="dg-head"/></marker><marker id="dg-head-accent" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="dg-head accent"/></marker></defs><g class="dg-node"><rect x="20" y="20" width="130" height="56" rx="10"/><text x="85.0" y="45" class="dg-title">Flags</text><text x="85.0" y="63" class="dg-sub">hey-compatible</text></g><g class="dg-node"><rect x="200" y="20" width="150" height="56" rx="10"/><text x="275.0" y="45" class="dg-title">TestConfig</text><text x="275.0" y="63" class="dg-sub">validated settings</text></g><g class="dg-node"><rect x="400" y="20" width="170" height="56" rx="10"/><text x="485.0" y="45" class="dg-title">Schedule</text><text x="485.0" y="63" class="dg-sub">slots · rate · deadline</text></g><g class="dg-node accent"><rect x="20" y="140" width="130" height="56" rx="10"/><text x="85.0" y="165" class="dg-title">Control</text><text x="85.0" y="183" class="dg-sub">workers · rate</text></g><g class="dg-node"><rect x="200" y="140" width="150" height="56" rx="10"/><text x="275.0" y="165" class="dg-title">Pool</text><text x="275.0" y="183" class="dg-sub">spawns and parks</text></g><g class="dg-node"><rect x="400" y="140" width="170" height="56" rx="10"/><text x="485.0" y="165" class="dg-title">Workers × N</text><text x="485.0" y="183" class="dg-sub">claim · wait · send</text></g><g class="dg-node"><rect x="620" y="140" width="120" height="56" rx="10"/><text x="680.0" y="165" class="dg-title">Your server</text><text x="680.0" y="183" class="dg-sub">HTTP/1.1 · HTTP/2</text></g><g class="dg-node accent"><rect x="20" y="260" width="130" height="56" rx="10"/><text x="85.0" y="285" class="dg-title">Terminal UI</text><text x="85.0" y="303" class="dg-sub">+ − ] [ keys</text></g><g class="dg-node"><rect x="200" y="260" width="150" height="56" rx="10"/><text x="275.0" y="285" class="dg-title">Report</text><text x="275.0" y="303" class="dg-sub">text · JSON · CSV</text></g><g class="dg-node"><rect x="400" y="260" width="170" height="56" rx="10"/><text x="485.0" y="285" class="dg-title">TestState</text><text x="485.0" y="303" class="dg-sub">HDR histogram · series</text></g><path d="M150 48H198" class="dg-edge" marker-end="url(#dg-head)"/><path d="M350 48H398" class="dg-edge" marker-end="url(#dg-head)"/><path d="M275 76V138" class="dg-edge" marker-end="url(#dg-head)"/><text x="283" y="112" class="dg-label" text-anchor="start">clients</text><path d="M485 76V138" class="dg-edge" marker-end="url(#dg-head)"/><text x="493" y="112" class="dg-label" text-anchor="start">slot k, due at t</text><path d="M150 168H198" class="dg-edge accent" marker-end="url(#dg-head-accent)"/><path d="M350 168H398" class="dg-edge" marker-end="url(#dg-head)"/><path d="M570 168H618" class="dg-edge" marker-end="url(#dg-head)"/><path d="M485 196V258" class="dg-edge" marker-end="url(#dg-head)"/><text x="493" y="232" class="dg-label" text-anchor="start">results, batched</text><path d="M400 288H352" class="dg-edge" marker-end="url(#dg-head)"/><path d="M85 260V198" class="dg-edge accent" marker-end="url(#dg-head-accent)"/><text x="93" y="232" class="dg-label" text-anchor="start">live changes</text><path d="M485 316C485 356 85 356 85 318" class="dg-edge accent" marker-end="url(#dg-head-accent)"/><text x="285" y="362" class="dg-label" text-anchor="middle">drawn every frame</text></svg></div>

1. **Parse and validate.** clap turns the flags into a `TestConfig`: the request (method, headers, body, auth), network settings (proxy, TLS, `--connect-to`, address family), the load (`-n`, `-c`, `-z`) and the pacing (`-q` or `--qps`). Bad input fails here, before any request is sent.
2. **Build clients.** One shared connection pool for HTTP/1.1, or one client per connection for HTTP/2 (more on that below).
3. **Run the engine.** A pool of Tokio tasks, the *workers*, claim request slots from a shared *schedule*, send, and report each result over a channel.
4. **Collect.** A single consumer task folds results into the live `TestState` that the UI draws from.
5. **Report.** When the test ends, the state becomes a `Report`, written as text, JSON or CSV, compared with a baseline and checked against your `--assert` rules.

# The schedule

Every request is a numbered *slot*. Workers don't receive jobs from a queue. Each one claims the next slot number from an atomic counter, which is all the coordination they need:

```rust
/// Claim the next request slot, or `None` when the budget is spent.
pub fn claim(&self) -> Option<usize> {
    let k = self.next.fetch_add(1, Ordering::Relaxed);
    match self.budget {
        Some(b) if k >= b => None,
        _ => Some(k),
    }
}
```

`-n 10k` sets the budget, so slot 10,000 and beyond return `None` and the workers finish. `-z 30s` sets a deadline instead, and a timer cancels the run when it passes.

The schedule also decides *when* each slot may be sent:

| Pacing | Flag | How it works |
|---|---|---|
| Unlimited | (default) | Each worker sends its next request as soon as the last one finishes. |
| Per worker | `-q 10` | Each worker has its own ticker, as in hey. Total rate ≈ `-q` × `-c`. |
| Global | `--qps 500` | Slot *k* is due at *start + k / 500 s*, a fixed timetable shared by all workers. |

# Honest latency: coordinated omission

Most load generators send a request, wait for the answer, then send the next one. If the server stalls for two seconds, the generator stalls with it and doesn't send the requests it should have sent during those two seconds. Only the one request that hit the stall records a slow time; the hundreds of users who would have arrived during it are never counted. The percentiles look healthy because the bad moments were *omitted*. Gil Tene named this *coordinated omission*.

With `--qps`, whambam measures from when each request was *due*, not from when a worker got around to sending it:

```rust
let sent = Instant::now();
let (outcome, latency) = self.sender.send().await;
// Charge the time spent waiting for a free worker (coordinated omission).
let begun = due.unwrap_or(sent).min(sent);
let metric = RequestMetric {
    start_offset: begun.saturating_duration_since(self.schedule.start()),
    latency: latency + sent.duration_since(begun),
    // ...
};
```

A worked example: at `--qps 100`, a request is due every 10 ms. The server freezes for one second. With a sender-side clock, you'd see one 1-second request and a p99 that barely moves. With the due-time clock, the 100 requests that should have gone out during that second are charged for the time they waited: 1 s, 990 ms, 980 ms, … The p99 now tells you what users would have seen.

The JSON report says which clock was used: `"latency_corrected": true` for `--qps` runs. `-q` keeps hey's per-worker meaning and its uncorrected measurement, so existing scripts behave as they always have.

# Steering a running test

The live controls (`+` / `-` for workers, `]` / `[` for rate) are built on a Tokio [`watch`](https://docs.rs/tokio/latest/tokio/sync/watch/) channel: a single value that many tasks can observe. The UI writes to it, and the pool and every worker hold a receiver.

```rust
pub struct Settings {
    pub workers: usize,
    /// Requests per second: total for `--qps`, per worker for `-q`, `None` when unthrottled.
    pub rate: Option<f64>,
}
```

Each key press scales by 10% (at least one step). What happens next depends on what changed:

- **More workers.** The pool spawns new worker tasks up to the new count. With HTTP/2 they share the existing connections.
- **Fewer workers.** Nothing is cancelled mid-request. Each worker knows its index, and a worker whose index is at or above the live count *parks* after its current request, waiting on the `watch` channel until the count grows again.
- **New `-q` rate.** Each worker rebuilds its ticker at the new interval before claiming its next slot.
- **New `--qps` rate.** This needs more care, because slots are already on a timetable. Workers may be asleep waiting for slots that were scheduled at the old rate. The schedule starts a new *segment* at the first slot that isn't yet due, then wakes every sleeping worker to re-read its due time:

```rust
/// Change the global rate from now: every slot not yet due under the old
/// rate is re-timed at the new rate, starting at `now`. Slots already past
/// due (a backlog) become due immediately rather than carrying the debt.
pub fn set_global_rate(&self, qps: f64, now: Instant) {
    // ...
    *guard = Some(Segment { base_k, base_t: now, qps });
    drop(guard);
    self.retimed.send_replace(());
}
```

Without re-timing, raising the rate would only take effect after the queued slots drained, which could be seconds later. A test checks that a rate change applies straight away.

Every change is recorded as an *adjustment* with its time, worker count and rate. The charts mark adjustments with vertical lines, the text report lists them under "Load changes", and the JSON report includes them in `adjustments`. Stepping the rate up until the latency line bends is a quick way to find a service's capacity in one session.

# Measuring a request

A request's latency runs from sending it to reading the **last byte** of the body, not just the headers. A slow or large body is part of what users wait for:

```rust
let started = Instant::now();
let outcome = match self.builder().send().await {
    Ok(mut resp) => {
        let status = resp.status().as_u16();
        let mut bytes = 0u64;
        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => bytes += chunk.len() as u64,
                Ok(None) => break Outcome { status: Ok(status), bytes_received: bytes },
                Err(e) => break Outcome { status: Err(classify(&e)), bytes_received: bytes },
            }
        }
    }
    Err(e) => Outcome { status: Err(classify(&e)), bytes_received: 0 },
};
(outcome, started.elapsed())
```

The body is counted chunk by chunk and dropped, never buffered whole, so large responses don't cost memory.

Any status below 400 counts as a success. Transport failures are sorted into ten kinds, so a run with 4,000 failures shows *"3,998 connection refused, 2 timeout"* rather than 4,000 lines of error text: `timeout`, `dns`, `connection_refused`, `connection_closed`, `tls`, `connect`, `redirect`, `body`, `request` and `other`.

whambam doesn't report per-phase timings (DNS, connect, TLS, time to first byte). hey prints them, but reqwest doesn't expose those phases, and whambam won't show numbers it didn't measure.

# Connections, DNS and TLS

**HTTP/1.1** sends one request at a time per connection. All workers share one reqwest client whose pool keeps up to one idle connection per worker, so `-c 50` settles at 50 keep-alive connections. `--disable-keepalive` sets the pool size to zero, so every request opens a new connection.

**HTTP/2** carries many requests over one connection, so a single shared client would put every worker on one connection. whambam builds one client per `-c` connection instead. `-p 10` puts 10 workers on each, so `-c 5 -p 10` means 5 connections carrying 50 concurrent streams.

**DNS** goes through whambam's own resolver. It caches lookups and supports `--connect-to` (send traffic for `api.example.com:443` to `10.0.0.5:8443` while keeping the Host header and TLS name) and `-4` / `-6`.

**TLS** is [rustls](https://github.com/rustls/rustls) with the *ring* crypto provider. The usual alternative, aws-lc, needs extra tools to build for Windows and static Linux targets. Mutual TLS (`--cert` / `--key`), extra CAs (`--cacert`), proxies (HTTP and SOCKS5) and compression (gzip, brotli, deflate, zstd) all come from reqwest.

# Collecting statistics

Workers never touch the shared statistics. Each one sends a small `RequestMetric` (a timestamp, the latency, the outcome and byte counts) down a bounded channel. One consumer task receives them in batches of up to 512 and records each batch under a single lock:

```rust
let mut batch = Vec::with_capacity(BATCH);
while rx.recv_many(&mut batch, BATCH).await > 0 {
    {
        let mut guard = state.lock();
        for m in &batch {
            guard.record(m);
        }
    }
    // ...the CSV writer, if any, gets the same batch
    batch.clear();
}
```

The lock is taken once per batch, never per request in a worker, and the only other code that takes it is the UI drawing a frame. That is simpler and faster than per-request atomics or lock-free queues.

`TestState` holds:

- an [HDR histogram](https://hdrhistogram.github.io/HdrHistogram/) of every latency, in microseconds, from 1 µs to one hour with three significant digits. Every percentile, from p50 to p99.9, is computed from all requests, not from a sample.
- counts per status code and per error kind, plus bytes sent and received.
- a time series with one sample per 100 ms (throughput and mean latency), keeping the last minute for the charts.
- the list of live adjustments.

# The interface

The UI is [ratatui](https://ratatui.rs) on crossterm. It runs in a blocking section (Tokio's `block_in_place`), so drawing never holds up the async workers. Each frame, at `--fps` (10 by default), it locks the state, advances the time series and draws one of three tabs: the dashboard, charts, and status codes and errors. Then it waits for a key until the next frame is due.

With `--baseline report.json`, the numbers on the dashboard show their change from the baseline, in red when worse, and the charts draw the baseline as reference lines. `r` runs the same test again without leaving the UI.

When stdout isn't a terminal (piped, redirected, or in CI), whambam skips the UI, prints progress to stderr and writes the report to stdout.

# Reports, baselines and checks

There is one `Report` model, and every output format renders it:

- **text**: a hey-style summary with a histogram, percentiles, status codes, errors and load changes.
- **JSON**: everything, documented by a [JSON Schema](https://github.com/cloudy-native/whambam.dev/blob/main/schema/report.schema.json).
- **CSV**: one row per request, streamed as the test runs (`offset_s, latency_ms, status, error, bytes_received`).

A JSON report doubles as a **baseline**. `--baseline` compares the run with it metric by metric, and `--assert` turns the comparison into a pass or fail:

```bash
whambam https://staging.example.com -z 30s --qps 200 \
  --baseline baseline.json --assert 'p99<+10%,errors<1%'
```

A check is a metric (`p50` … `p99.9`, `mean`, `min`, `max`, `rps`, `success`, `errors`), an operator and a limit: absolute (`200ms`, `1%`, `1000`) or relative to the baseline (`+10%`). If any check fails, whambam exits with status 1, which fails a CI job. `whambam compare old.json new.json` runs the same comparison on two saved reports without sending any requests.

The bundled [GitHub Action](https://github.com/cloudy-native/whambam.dev/blob/main/action.yml) installs the right binary for the runner, runs the test, adds a summary table to the job page and fails the job when a check fails.

# Builds and releases

Each release is built natively on a GitHub-hosted runner of the same OS and CPU, with no cross-compiling, and each binary is run once before it is packaged:

| Target | Runner | Notes |
|---|---|---|
| macOS Apple Silicon, Intel | macos-15, macos-15-intel | |
| Linux x86_64, ARM64 (glibc) | ubuntu-24.04, ubuntu-24.04-arm | also `.deb` packages |
| Linux x86_64, ARM64 (musl) | ubuntu-24.04, ubuntu-24.04-arm | static, any distro |
| Windows x86_64, ARM64 | windows-2025, windows-11-arm | |

A release publishes to GitHub Releases, the Homebrew tap and crates.io, with SHA-256 checksums for every file. It then runs the GitHub Action against the new release on Linux, macOS and Windows.

# Stack

| Crate | Used for |
|---|---|
| tokio, tokio-util | async runtime, timers, channels, cancellation |
| reqwest, rustls (ring) | HTTP/1.1 and HTTP/2 client, TLS |
| hdrhistogram | latency percentiles |
| ratatui | terminal UI |
| clap | command-line flags |
| serde, serde_json, csv | reports |

# Does it work?

A load generator has two jobs: produce enough load, and report what happened accurately. We tested both against hey and [bombardier](https://github.com/codesenberg/bombardier), using the demo server from the whambam repository.

**Setup:** a Mac with an Apple M3 (8 cores, 8 GB), with the server and the load generator on the same machine. whambam 1.1.0, hey from Homebrew and bombardier from `go install`. Every run used 50 connections (`-c 50`), and every test ran twice.

## Accuracy: a server with realistic latency

The demo server answers in 3 to 20 ms on a slow wave, with a 1% tail at about 40 ms and 0.4% `503` errors. Here, the server sets the pace, and a good tool should report it faithfully.

```bash
cargo run --release -p test-utils --bin demo-server -- 8082
whambam http://127.0.0.1:8082/ -z 21s -c 50
```

| Tool | Requests/s | p50 | p90 | p99 | 503s |
|---|---|---|---|---|---|
| hey | 6,180 · 6,042 | 6.0 · 6.2 ms | 14.6 · 14.8 ms | 44.5 · 24.9 ms | 0.40% · 0.39% |
| bombardier | 6,279 · 6,185 | 5.9 · 6.0 ms | 14.6 · 14.6 ms | 44.3 · 44.1 ms | 0.38% · 0.42% |
| whambam | 6,061 · 6,111 | 6.2 · 6.1 ms | 14.8 · 14.8 ms | 44.3 · 32.2 ms | 0.38% · 0.37% |

The three tools agree to within 4% on throughput and about 0.3 ms on p50 and p90. The p99 jumps between runs for every tool because it sits right at the edge of the server's 1% slow tail, so a few requests either way move it between about 25 and 44 ms. That's the server's behaviour, and all three tools saw it.

## Raw speed: a server that answers instantly

`demo-server --fast` replies immediately with a tiny body. Now the load generator is the bottleneck.

```bash
cargo run --release -p test-utils --bin demo-server -- 8081 --fast
whambam http://127.0.0.1:8081/ -z 10s -c 50
```

| Tool | Requests/s | p50 | p99 | Errors |
|---|---|---|---|---|
| hey | 139,992 · 173,054 | 0.20 ms | 1.2 ms | 1,495 · 0 |
| bombardier | 220,794 · 217,595 | 0.20 ms | 0.67 ms | 0 |
| whambam | 151,362 · 151,706 | 0.32 ms | 0.69 ms | 0 |

bombardier, built on [fasthttp](https://github.com/valyala/fasthttp), is the fastest generator here, at about 45% more requests per second than whambam. whambam is in hey's range and steadier from run to run. In hey's first run, 1,495 requests failed with `can't assign requested address`: hey ran out of local ports. hey only reports latency to 0.1 ms, which is why its p99 reads 1.2 ms.

At 150,000 requests per second, whambam is unlikely to be the limit for a real service over a real network. If you need more from one machine, run several copies or use bombardier or wrk.

We first tried Python's `http.server` as a target, as many guides suggest. Under 50 connections it refused or dropped from a quarter to nearly all of the requests, whichever tool sent them, so it tests Python rather than the load generator.
