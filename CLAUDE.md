# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

whambam is a super-low-latency web throughput testing framework. It is designed to measure the performance of web applications in terms of the number of requests that can be processed per second.

## Repository Structure

A Cargo workspace: the `whambam` crate (root) plus `test-utils` (mock HTTP/1.1 + h2c server for tests, `demo-server` binary for recordings).

- `src/cli/` clap flags, `src/config/` validated `TestConfig`, `src/http/` clients/resolver/execution/error kinds, `src/engine/` worker pool, live `Control` (watch channel), schedule, `src/stats/` live `TestState`, `src/report/` `Report` model (text/JSON/CSV) plus metrics/baselines/assertions, `src/checks.rs` --baseline/--assert/`compare`, `src/ui/` ratatui views.
- There is exactly one CLI definition (`src/cli`). The binary (`src/main.rs`) only calls `whambam::run`.
- TLS is rustls with the `ring` provider (installed in `http::client`); do not enable reqwest's `rustls` (aws-lc) feature, it complicates Windows/musl builds.
- `schema/report.schema.json` documents `-o json`; keep it in sync with `src/report/mod.rs` (a test checks required keys).
- `action.yml` is the GitHub Action; `.github/workflows/action-test.yml` tests it against each published release.
- `demo/record.sh` re-records the README GIF and website screenshots with vhs.
- CI (`.github/workflows/ci.yml`) runs on Linux x86_64 only. Releases build natively per target via `build.yml`; never trigger `release.yml` without the user asking. Manual runs default to `dry_run` (build and package only, publish nothing).

Don't let code files get to be more than 300 lines long. Always refactor to keep files short and focused. If you find yourself writing a long function, consider breaking it down into smaller functions. Always factor similar code into reusable functions. Implement separation of concerns in code. Use the DRY and KISS achitectural principles. 

## Development Commands

### Making updates

Always run `cargo fmt`, `cargo check` and `cargo clippy` after making updates to the codebase.

### Building the Project
`cargo build`

### Running the Project
`cargo run`

### Testing
`cargo test --workspace`

### Linting and Formatting
`cargo fmt`
`cargo clippy --workspace --all-targets -- -D warnings`

## Important Notes

- Always write unit and integration tests for new code.
- Update this file as the codebase evolves to provide accurate guidance
- Always use the latest stable version of Rust
- Always use the latest stable version of Cargo
  