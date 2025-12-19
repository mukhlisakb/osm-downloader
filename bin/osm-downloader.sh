#!/usr/bin/env bash

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="$PROJECT_ROOT/target/release/osm-downloader"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Error: cargo is not installed."
  echo "Please install Rust and Cargo from https://rustup.rs/ and then re-run this script."
  exit 1
fi

echo "Building osm-downloader in release mode..."
(
  cd "$PROJECT_ROOT"
  cargo build --release
)

if [ ! -x "$BIN_PATH" ]; then
  echo "Error: built binary not found at: $BIN_PATH"
  exit 1
fi

echo "Starting osm-downloader..."
exec "$BIN_PATH"

