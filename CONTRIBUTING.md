# Contributing to WhambBam

Thank you for your interest in contributing to WhambBam! We appreciate your help in making this project better.

This document provides guidelines and instructions for contributing to this project.

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

## How to Contribute

### Reporting Bugs

If you find a bug, please create a new issue using the bug report template. Include as much detail as possible:

- A clear and concise description of the bug
- Steps to reproduce the behavior
- Expected behavior
- Screenshots (if applicable)
- Your environment details (OS, Rust version, etc.)

### Requesting Features

For feature requests, please create a new issue using the feature request template. Clearly describe the problem the feature would solve and the proposed solution.

### Pull Requests

1. Fork the repository
2. Create a new branch from `main`
   ```
   git checkout -b feature/your-feature-name
   ```
3. Make your changes
4. Run tests to make sure everything is working
   ```
   cargo test
   ```
5. Make sure your code follows our style guidelines
   ```
   cargo clippy
   ```
6. Commit your changes following the [conventional commits](https://www.conventionalcommits.org/) format
7. Push your branch to your fork
8. Open a pull request against the `main` branch

## Development Setup

1. Install Rust and Cargo (https://rustup.rs/)
2. Clone the repository
   ```
   git clone https://github.com/yourusername/whambam.git
   cd whambam
   ```
3. Build the project
   ```
   cargo build
   ```
4. Run tests
   ```
   cargo test
   ```

## Code Style

- Follow standard Rust style conventions
- Use `cargo fmt` to format your code
- Run `cargo clippy` to catch common mistakes and improve code quality
- Write meaningful commit messages following the conventional commits format

## Testing

- Write tests for new features and bug fixes
- Ensure all tests pass before submitting a pull request
- For significant changes, consider adding integration tests

## License

By contributing to this project, you agree that your contributions will be licensed under the same license as the project.