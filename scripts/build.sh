#!/usr/bin/env bash
#
# Tests and lints YAL's platform-independent crates on Linux or macOS.
# Building the Windows launcher binary and NSIS installer requires Windows (`./scripts/build.ps1`).

set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy -p tc-core -p tc-platform -p tc-apps -p tc-calc -p tc-search -p tc-snippets --all-targets -- -D warnings

echo "==> cargo test"
cargo test -p tc-core -p tc-platform -p tc-apps -p tc-calc -p tc-search -p tc-snippets

echo ""
echo "The pure-logic crates are clean. For the full Windows build run:"
echo "  ./scripts/build.ps1 -Arch x64"
