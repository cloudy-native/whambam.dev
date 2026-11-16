#!/usr/bin/env bash
# Render the Homebrew formula and Scoop manifest for a release.
# Usage: packaging/render-manifests.sh <tag> <dist-dir> <out-dir>
# <dist-dir> holds the release archives built by .github/workflows/build.yml.
set -euo pipefail

tag=$1 dist=$2 out=$3
version=${tag#v}
repo=https://github.com/cloudy-native/whambam.dev
base="$repo/releases/download/$tag"
mkdir -p "$out"

sha() {
  local file="$dist/whambam-$tag-$1.$2"
  [[ -f $file ]] || { echo "missing $file" >&2; exit 1; }
  if command -v sha256sum > /dev/null; then sha256sum "$file"; else shasum -a 256 "$file"; fi | cut -d' ' -f1
}

cat > "$out/whambam.rb" <<RUBY
class Whambam < Formula
  desc "Fast HTTP(S) load testing with a live terminal UI and hey-compatible flags"
  homepage "https://whambam.dev"
  version "$version"
  license "MIT"
  head "$repo.git", branch: "main"

  on_macos do
    on_arm do
      url "$base/whambam-$tag-aarch64-apple-darwin.tar.gz"
      sha256 "$(sha aarch64-apple-darwin tar.gz)"
    end
    on_intel do
      url "$base/whambam-$tag-x86_64-apple-darwin.tar.gz"
      sha256 "$(sha x86_64-apple-darwin tar.gz)"
    end
  end

  on_linux do
    on_arm do
      url "$base/whambam-$tag-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$(sha aarch64-unknown-linux-musl tar.gz)"
    end
    on_intel do
      url "$base/whambam-$tag-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$(sha x86_64-unknown-linux-musl tar.gz)"
    end
  end

  def install
    bin.install "whambam"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/whambam --version")
  end
end
RUBY

cat > "$out/whambam.json" <<JSON
{
  "version": "$version",
  "description": "Fast HTTP(S) load testing with a live terminal UI and hey-compatible flags",
  "homepage": "https://whambam.dev",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "$base/whambam-$tag-x86_64-pc-windows-msvc.zip",
      "hash": "$(sha x86_64-pc-windows-msvc zip)"
    },
    "arm64": {
      "url": "$base/whambam-$tag-aarch64-pc-windows-msvc.zip",
      "hash": "$(sha aarch64-pc-windows-msvc zip)"
    }
  },
  "bin": "whambam.exe",
  "checkver": "github",
  "autoupdate": {
    "architecture": {
      "64bit": { "url": "$repo/releases/download/v\$version/whambam-v\$version-x86_64-pc-windows-msvc.zip" },
      "arm64": { "url": "$repo/releases/download/v\$version/whambam-v\$version-aarch64-pc-windows-msvc.zip" }
    }
  }
}
JSON
echo "wrote $out/whambam.rb and $out/whambam.json"
