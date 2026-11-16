# whambam → Terminal Trove: parity plan and differentiators

_Survey date: 2026-11-16. Source: https://terminaltrove.com/categories/benchmarking/ (8 tools)._

## Status (2026-11-16)

Phases 0 and 1 are implemented on branch `feature/terminal-trove-parity`:

- **Phase 0:** one CLI (`src/cli`), modules split under 300 lines, ratatui 0.30 + reqwest 0.13, one `Report` model for every output.
- **Phase 1:** demo GIF/PNG (`demo/record.sh`), Windows x86_64 + ARM64 builds, crates.io/binstall metadata, Scoop manifest, Linux in the Homebrew formula, `-o text|json|csv|quiet` + `--output` + JSON schema, `--qps` with coordinated-omission correction, errors by kind, TLS/network flags (`-k`, `--cacert`, `--cert/--key`, `--connect-timeout`, `--connect-to`, `--host`, `-4/-6`), `--debug`, `--http2` + `-p`, `-r`, `10k` suffixes, `NO_COLOR`, `--fps`, README rewrite.

Still manual: first release run on the new native runners, enabling the Homebrew/crates.io/winget jobs (repository variables + secrets), first winget submission, homebrew-core submission, and the Terminal Trove form itself.

## 1. The competitive field

| Tool | Lang | Stars | What it benchmarks | Direct rival? |
|---|---|---|---|---|
| **oha** | Rust / ratatui | 11K | HTTP load + live TUI (hey-inspired) | **Yes: the main one** |
| **ali** | Go / termdash | 3.9K | HTTP load + real-time charts (vegeta-inspired) | **Yes** |
| ballast | Rust | 48 | API snapshot tests from JSON config, thresholds, compare | Partly (CI angle) |
| hyperfine | Rust | 29K | Shell command timing | No, but sets the bar for stats/export |
| cmdperf | Go | 41 | Shell commands, interactive TUI | No |
| dnspyre | Go | 335 | DNS QPS | No, but has JSON/CSV/histogram charts |
| xfr | Rust / ratatui | 588 | Network throughput (iperf3 alt) | No, but has compare-past-runs, Prometheus, themes |
| s-tui | Python | 5.1K | CPU stress/monitor | No |

whambam's listing will be read side by side with **oha** (same language, same TUI library, same hey heritage, same `-n -c -z -q` flags). Being "another oha" won't get it listed in a memorable way. It needs parity on the basics and **one or two headline features oha lacks**.

### Terminal Trove submission requirements
1. Ideally cross-platform: **whambam has no Windows build today** (oha, ali, hyperfine all do).
2. Standalone binaries preferred: ✅ (6 targets via cargo-release + GH Actions).
3. Image preview (PNG/GIF/MP4) is required. Only a PNG today. Every successful entry has a GIF.
4. Must not already be listed: ✅.

The form also asks for a tagline (~100 chars), description (100-400), 2-3 standout features (60-400), "what makes this different" (50-500), and who it's for (60-300). Drafts are in §5.

## 2. Gap matrix (whambam vs oha vs ali)

✅ = has it, ➖ = partial, ❌ = missing. **Bold** rows are where whambam is behind oha.

| Capability | whambam | oha | ali |
|---|---|---|---|
| `-n` / `-c` / `-z` / `-q` / `-m` / `-H` / `-d` / `-D` / `-A` / `-T` / `-a` / `-x` | ✅ | ✅ | ✅ (different flags) |
| Live TUI: throughput + latency over time | ✅ (dual-axis combined) | ✅ | ✅ |
| Status-code breakdown | ✅ | ✅ | ➖ |
| **Error breakdown by kind** (timeout / refused / TLS / DNS) | ❌ (one count) | ✅ | ➖ |
| **Latency histogram in TUI** | ❌ (text report only) | ✅ | ➖ |
| **Percentiles-over-time chart** | ❌ | ➖ | ✅ |
| **Chart zoom / time window / mouse** | ❌ | ➖ (window keys) | ✅ |
| HDR histogram, accurate mean | ✅ | ✅ | ✅ |
| **Coordinated-omission correction** | ❌ (latency starts at send; per-worker `sleep` rate loop) | ✅ `--latency-correction` | ✅ (vegeta open model) |
| **Global QPS** (vs hey's per-worker) | ❌ | ✅ | ✅ |
| **Burst mode** | ❌ | ✅ | ❌ |
| **JSON / CSV output, `-o file`** | ❌ (text only) | ✅ (+ schema.json) | ✅ `--export-to` |
| **Write per-request rows to SQLite** | ❌ | ✅ `--db-url` | ❌ |
| **HTTP/2 (`--http2`, `-p` parallel streams)** | ❌ | ✅ | ✅ |
| HTTP/3 | ❌ | ✅ (experimental) | ❌ |
| **TLS: `--insecure`, `--cacert`, `--cert/--key`** | ❌ | ✅ | ✅ |
| **`--connect-to`, `--host`, `--ipv4/6`, `--unix-socket`, `--connect-timeout`** | ❌ | ✅ | ➖ (`--resolvers`) |
| **Multiple URLs: `--urls-from-file`, `--rand-regex-url`, `-Z` body per line** | ❌ | ✅ | ❌ |
| **Multipart `-F`** | ❌ | ✅ | ❌ |
| **`--debug` (one request, dump req/resp)** | ❌ | ✅ | ✅ |
| **Wait vs abort in-flight requests at deadline (`-w`)** | ❌ | ✅ | ❌ |
| **Redirect limit (`-r N`)** | ➖ (on/off) | ✅ | ❌ |
| **`10k` / `1m` suffixes for `-n`** | ❌ | ✅ | ❌ |
| AWS SigV4 signing | ❌ | ✅ | ❌ |
| `NO_COLOR`, `--fps`, `--time-unit` | ❌ | ✅ | ➖ |
| Restart test from TUI | ✅ (`r`) | ❌ | ✅ (Enter) |
| **Windows binary** | ❌ | ✅ | ✅ |
| **Distribution**: crates.io / Homebrew core / winget / scoop / AUR / Docker | ➖ (tap only, `publish = false`) | ✅ all | ✅ most |
| **Animated demo GIF** | ❌ | ✅ | ✅ |

## 3. Parity plan

Effort: S ≈ under half a day, M ≈ 1–3 days, L ≈ a week or more.

### Phase 0: Codebase hygiene (do first, it makes everything else cheaper)
| # | Item | Effort | Why |
|---|---|---|---|
| 0.1 | **Collapse the 4 CLI definitions** (`src/args.rs`, `src/lib.rs`, `src/main.rs`, `src/bin/whambam.rs`) into one. They've already drifted (`args.rs` defaults `-n` to 0 and has `-o ui\|hey`; the others default to 200 and use `--no-ui`). | S | Every flag below would otherwise be added 4 times. |
| 0.2 | Split `src/ui/widgets.rs` (945 lines) and `unified_runner.rs` (518) to meet the 300-line rule. | S | Needed before adding tabs. |
| 0.3 | Upgrade **ratatui 0.22 → current** and **reqwest 0.11 → 0.12 (hyper 1)**. | M | Unlocks HTTP/2 control, current widgets, mouse API. Reviewers notice stale deps. |
| 0.4 | Introduce a `Metrics → Report` model that every output (TUI, text, JSON, CSV) renders from. | M | One source of truth for all the export work. |

### Phase 1: Must-have before submitting
| # | Item | Effort |
|---|---|---|
| 1.1 | **Animated GIF + PNG preview** recorded with `vhs` (scripted, reproducible, committed `demo.tape`). Show the dashboard → charts → status tabs against a local server. | S |
| 1.2 | **Windows target** (`x86_64-pc-windows-msvc`) in release.yml + **scoop/winget** manifests. | M |
| 1.3 | **Publish to crates.io** (`cargo install whambam`) + `cargo-binstall` metadata. Submit to **homebrew-core**. | S |
| 1.4 | **`--output-format text\|json\|csv` + `-o FILE`**, with a published JSON schema. | M |
| 1.5 | **Coordinated-omission-correct rate limiting**: a global scheduler (token/tick based, not per-worker sleep), latency measured from *scheduled* time. Keep hey's per-worker `-q` for compatibility; add `--qps` (global) and make correction the default under `--qps`. | M |
| 1.6 | **Error breakdown by kind** (timeout, connect refused, DNS, TLS, body read, HTTP 4xx/5xx) as a TUI tab and in reports. | S |
| 1.7 | **TLS & network flags**: `--insecure`, `--cacert`, `--cert/--key`, `--connect-timeout`, `--host`, `--connect-to`, `--ipv4/--ipv6`. | M |
| 1.8 | **`--debug`**: one request with full request/response dump. | S |
| 1.9 | **`--http2`** (+ `-p` streams per connection), and `-r N` redirect limit. | M |
| 1.10 | `-n 10k` suffixes, `NO_COLOR`/`--no-color`, `--fps`. | S |
| 1.11 | README pass: retitle "drop-in" to "hey-compatible flags" with a short "differences" table (`-q` semantics). Move the "AI-Powered Development / Bedrock cost" section to the website or CONTRIBUTING. | S |

### Phase 2: Full parity (after listing is fine)
| # | Item | Effort |
|---|---|---|
| 2.1 | Latency **histogram tab** and **percentiles-over-time** (p50/p90/p95/p99) chart. | M |
| 2.2 | Chart **time-window keys** (`+`/`-`) and mouse drag-to-zoom. | M |
| 2.3 | `--urls-from-file`, `--rand-regex-url`, `-Z` body-per-line. | M |
| 2.4 | `-F` multipart, `--unix-socket`, `-w` wait-for-in-flight with "aborted at deadline" counter. | M |
| 2.5 | `--burst-delay/--burst-rate`. | S |
| 2.6 | `--db-url` SQLite per-request sink (or per-request CSV/NDJSON, which covers most of the value). | S |
| 2.7 | Docker image on GHCR, AUR package. | S |
| 2.8 | HTTP/3 behind a feature flag. | L |

## 4. Ideas that make whambam stand out

Ranked by **(how different it is from oha/ali) × (how cheap it is)**. My recommendation: ship **A + B + C** as the launch headline. Together they turn "a nicer oha" into "the load tester that tells you *why* and *whether it got worse*".

### A. Live control knobs: change load mid-run ★ headline
Arrow keys / `+` `-` adjust **concurrency** and **QPS** while the test runs; the chart marks each change. You find the server's knee by feel, in one session, instead of re-running 10 times. No tool in the category does this. Pairs naturally with the existing `r` restart. **Effort: M** (the runner already has a worker pool; needs a shared target-concurrency/target-rate the scheduler reads).

### B. Saturation finder: `--find-max`
Automatic step ramp (`--ramp 10..1000 --step 50 --hold 10s`) that stops when p99 or error rate crosses a threshold (`--slo "p99<250ms,errors<1%"`), then reports **max sustainable RPS** and draws the throughput-vs-latency curve. That answers "how much can this box take?", the question people actually run load tests for. **Effort: M** (built on A).

### C. Baselines, compare & CI gates
- `--save run.json` / `whambam compare a.json b.json` → side-by-side table with deltas and a verdict.
- `--baseline run.json` overlays the old run as a ghost line in the live TUI.
- `--assert "p99<200ms,rps>1000,errors<0.5%"` → non-zero exit; ship a tiny **GitHub Action**.
ballast and xfr have pieces of this; no HTTP TUI does it. **Effort: M** (depends on 1.4).

### D. "Where does the time go?" request-phase breakdown
Per-request **DNS / connect / TLS / TTFB / download** timing, as a stacked live chart and in the text report. hey printed these; oha and ali don't, and whambam's own README says it deliberately omits them rather than fake them. Doing it *properly* is on-brand with the "accuracy-first" positioning. **Effort: L** (needs a custom hyper connector with timing hooks).

### E. Header breakdown tab: see the load balancer and the CDN
`--group-by-header x-cache` (or `cf-cache-status`, `server`, `x-served-by`, `x-amzn-trace-id` prefix…) gives a live table of requests, share and latency per header value. Instantly shows cache hit ratio under load, uneven load-balancer distribution, or one slow backend. Cheap and genuinely novel. **Effort: S.**

### F. Paste-from-browser: `whambam curl …` and `--har`
`whambam curl 'curl https://… -H … --data …'` accepts DevTools' "Copy as cURL" verbatim. `--har session.har` replays a recorded browser session as a weighted URL mix. Removes the most tedious part of setting up a realistic test. **Effort: S (curl) / M (HAR).**

### G. One-file HTML report: `--report out.html`
Self-contained page with interactive charts, histogram, phases, status/error breakdown. Easy to attach to a PR or a ticket; hooks into the existing Astro site for a "see an example report" link. **Effort: M.**

### H. Accuracy as a visible feature
Show a small "CO-corrected ✓" / "client saturated ⚠" badge: warn when the *client* is the bottleneck (CPU pegged, scheduler lag > N ms, ephemeral-port exhaustion). Load testers routinely measure themselves instead of the server; telling the user is rare and builds trust. **Effort: S–M.**

### I. Later / moonshots
- **Prometheus `/metrics` or OTLP push** while running, to correlate with server dashboards (xfr has Prometheus). S–M.
- **Distributed mode on AWS Lambda** (`--fleet lambda:50 --region …`) aggregating HDR histograms. Strong fit with your AWS/CDK work. L.
- **Themes + colour-blind palette.** S.

## 5. Draft Terminal Trove submission copy

Only claim what has shipped. The text below assumes Phase 1 + A + C are done.

- **Tagline (~100):** HTTP load testing with a live TUI. Turn the load up mid-run, find your limit, and catch regressions in CI.
- **Description (100–400):** whambam is a fast, hey-compatible HTTP(S) load generator written in Rust with a real-time terminal dashboard. It charts throughput and latency on a shared timeline, breaks down status codes and errors, and reports HDR-accurate percentiles with coordinated-omission correction. Output as text, JSON or CSV.
- **Standout features (60–400):** Adjust concurrency and request rate live from the keyboard while the test runs. Save a run as a baseline, overlay it on the next one, and gate CI with assertions like `p99<200ms`. Group results by any response header to see cache hit rates or load balancer spread.
- **What makes it different (50–500):** Other HTTP load TUIs show you numbers for a fixed load. whambam lets you steer the load interactively, find the saturation point automatically, and compare against previous runs, while being strict about measurement accuracy (no placeholder phase timings, CO-corrected latencies).
- **Who it's for (60–300):** Backend developers, SREs and platform engineers who want to find out how much traffic an endpoint can take, and whether a change made it slower, without leaving the terminal or writing a test script.

## 6. Suggested order
1. Phase 0 (≈2–4 days).
2. Phase 1 (≈1.5–2 weeks).
3. **A** live knobs, then **C** baselines/CI (≈1 week).
4. Record the GIF *showing A and C*, then submit.
5. Then B, E, F, Phase 2, D, G as follow-ups; each is a good "what's new" post that brings people back to the listing.
