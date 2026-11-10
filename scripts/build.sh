#!/usr/bin/env bash
#
# Builds YAL on Linux or macOS.
#
# The pure-logic crates build and test on any host, which is what this script
# is for. The application itself needs Windows: the installer, the palette and
# the Win32 implementation cannot be built here.
#
# Usage:
#   ./scripts/build.sh              # test and lint
#   ./scripts/build.sh --test-only  # just the tests

set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy -p tc-core -p tc-platform -p tc-calc -p tc-search -p tc-snippets --all-targets -- -D warnings

echo "==> cargo test"
cargo test -p tc-core -p tc-platform -p tc-calc -p tc-search -p tc-snippets

if [ "${1:-}" = "--test-only" ]; then
    exit 0
fi

echo "==> cargo run -p xtask -- parity --check"
cargo run --quiet -p xtask -- parity --check

echo ""
echo "The pure-logic crates are clean. The full build needs Windows:"
echo "  ./scripts/build.ps1"
