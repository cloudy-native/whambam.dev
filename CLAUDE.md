# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

blamo-web-throughput is a super-low-latency web throughput testing framework. It is designed to measure the performance of web applications in terms of the number of requests that can be processed per second.

## Repository Structure

This is a standard Rust project with a Cargo.toml file and a src directory.

Don't let code files get to be more than 300 lines long. Always refactor to keep files short and focused. If you find yourself writing a long function, consider breaking it down into smaller functions. Always factor similar code into reusable functions.

## Development Commands

### Making updates

Always run `cargo fmt`, `cargo check` and `cargo clippy` after making updates to the codebase.

### Building the Project
`cargo build`

### Running the Project
`cargo run`

### Testing
`cargo test`

### Linting and Formatting
`cargo fmt`
`cargo clippy`

## Architecture Overview

The architecture of blamo-web-throughput is currently minimal, with only a main.rs file in the src directory.

## Important Notes

- Always write unit and integration tests for new code.
- Update this file as the codebase evolves to provide accurate guidance
- Always use the latest stable version of Rust
- Always use the latest stable version of Cargo
  