# Contributing to WhamBam

Thank you for your interest in contributing to Whambam! We appreciate your help in making this project better.

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

## Releasing

We use [`cargo-release`](https://github.com/crate-ci/cargo-release) to manage releases. The process is automated end-to-end: bump the version, tag, push, and CI handles cross-platform builds, GitHub Releases, and Homebrew formula updates.

### Prerequisites

```bash
cargo install cargo-release
```

### Release Commands

```bash
# Dry run first (always recommended)
cargo release patch --dry-run

# Patch release: 0.2.1 → 0.2.2
cargo release patch

# Minor release: 0.2.1 → 0.3.0
cargo release minor

# Major release: 0.2.1 → 1.0.0
cargo release major
```

### What Happens

1. `cargo-release` bumps the version in `Cargo.toml`
2. Commits with message "Release {version}"
3. Creates a git tag `v{version}`
4. Pushes the commit and tag to the remote
5. The `v*` tag triggers the [Release workflow](.github/workflows/release.yml), which:
   - Runs the [CI checks](.github/workflows/ci.yml) (format, clippy, tests, crate packaging)
   - Builds every target **natively** with the reusable [build workflow](.github/workflows/build.yml), one job per OS and architecture:

     | Target | Runner | Artifacts |
     |---|---|---|
     | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | `.tar.gz`, `.deb` |
     | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz`, `.deb` |
     | `x86_64-unknown-linux-musl` (static) | `ubuntu-24.04` | `.tar.gz` |
     | `aarch64-unknown-linux-musl` (static) | `ubuntu-24.04-arm` | `.tar.gz` |
     | `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
     | `aarch64-apple-darwin` | `macos-15` | `.tar.gz` |
     | `x86_64-pc-windows-msvc` | `windows-2025` | `.zip` |
     | `aarch64-pc-windows-msvc` | `windows-11-arm` | `.zip` |

   - Creates a GitHub Release with every archive, per-file `.sha256` files, a combined `SHA256SUMS`, and a Scoop manifest (`whambam.json`)
   - Optionally updates the Homebrew tap, publishes to crates.io and submits to winget (see below)

TLS uses rustls with the `ring` provider, so no target needs OpenSSL, NASM or CMake. Windows ARM64 needs clang for `ring`; the build installs LLVM if the runner lacks it.

### Dry run

Before tagging, check that every target still builds: **Actions → Release → Run workflow**, keep **dry_run** ticked, and run it. It runs the CI checks and all eight native builds, renders the Homebrew formula and Scoop manifest, verifies the checksums, and lists the files in the run summary. Archives and manifests are uploaded as workflow artifacts; nothing is released or published. Untick **dry_run** only to cut a real release from a manual run (normally, pushing a `v*` tag does that).

### Optional publishing

These jobs are off until you set a repository variable to `true` (Settings → Secrets and variables → Actions):

| Variable | Secret | What it does |
|---|---|---|
| `HOMEBREW_TAP_ENABLED` | `HOMEBREW_TAP_TOKEN` | Commits the rendered formula (macOS + Linux) to `cloudy-native/homebrew-whambam` |
| `CRATES_IO_ENABLED` | `CARGO_REGISTRY_TOKEN` | `cargo publish` (enables `cargo install` and `cargo binstall`) |
| `WINGET_ENABLED` | `WINGET_TOKEN` | Opens a winget-pkgs PR. The first version must be submitted by hand with `wingetcreate new`. |

The release workflow ends with the [action self-test](.github/workflows/action-test.yml), which runs `action.yml` against the new release on Linux (x86_64, ARM64), macOS and Windows. After a release, move the floating major tag (`v1`) to it so `uses: cloudy-native/whambam.dev@v1` picks up the new version.

The formula and Scoop manifest come from [`packaging/render-manifests.sh`](packaging/render-manifests.sh), which you can run locally against a folder of release archives.

### Configuration

Release behavior is configured in `Cargo.toml` under `[package.metadata.release]`:

- `publish = false`: `cargo-release` does not publish; the `crates-io` job does when enabled
- `tag-name = "v{{version}}"`: tags as `v1.1.0`, etc.
- `pre-release-commit-message = "Release {{version}}"`: commit message format

## AI-Assisted Development

whambam was built with [Claude Code](https://www.anthropic.com/claude-code). What worked:

1. **Start with a plan.** Have the AI outline the approach before it writes code.
2. **Make incremental changes.** Small, testable steps avoid large rollbacks.
3. **Write tests eagerly**, before major refactoring.
4. **Use AI for systemic changes.** It is good at large, mechanical transformations.
5. **Iterate.** Ask it to fix its own mistakes.

The first version cost $34.92 on AWS Bedrock (`us.anthropic.claude-3-7-sonnet-20250219-v1:0`), with roughly 30% human-written structure and 70% AI-generated implementation.

## License

By contributing to this project, you agree that your contributions will be licensed under the same license as the project.