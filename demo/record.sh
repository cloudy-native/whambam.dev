#!/usr/bin/env bash
# Record the demo GIF/PNG with vhs (https://github.com/charmbracelet/vhs).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p whambam -p test-utils --bin whambam --bin demo-server
PATH="$PWD/target/release:$PATH"
demo-server 8080 & server=$!
trap 'kill $server' EXIT
sleep 1
vhs demo/demo.tape
