#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
mkdir -p coverage/rust

# Reuse one instrumented test run for all report formats.
cargo llvm-cov --workspace --locked --no-report
cargo llvm-cov report --lcov --output-path coverage/rust/lcov.info
cargo llvm-cov report --json --summary-only --output-path coverage/rust/coverage-summary.json
cargo llvm-cov report --html --output-dir coverage/rust
