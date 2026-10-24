#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
mkdir -p coverage/rust

# wasm-bindgen's experimental profiler needs nightly and a matching minicov
# profile layout (LLVM 21). Keep both targets on the same pinned compiler.
toolchain=nightly-2026-01-15
native_target="$PWD/target/coverage-native"
wasm_target="$PWD/target/coverage-wasm"

command -v wasm-bindgen-test-runner >/dev/null
command -v clang >/dev/null

# Cleaning workspace artifacts prevents stale test binaries from entering the
# combined report; dependency build caches are retained.
CARGO_TARGET_DIR="$native_target" cargo +"$toolchain" llvm-cov clean --workspace
CARGO_TARGET_DIR="$native_target" cargo +"$toolchain" llvm-cov --workspace --locked --no-report
CARGO_TARGET_DIR="$native_target" cargo +"$toolchain" llvm-cov report --json --summary-only --output-path coverage/rust/native-summary.json

CARGO_TARGET_DIR="$wasm_target" cargo +"$toolchain" llvm-cov clean --workspace
CARGO_TARGET_DIR="$wasm_target" \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS='-Cinstrument-coverage -Zno-profiler-runtime -Clink-args=--no-gc-sections --cfg=wasm_bindgen_unstable_test_coverage' \
cargo +"$toolchain" llvm-cov test -p aiui-aix-web --locked --target wasm32-unknown-unknown --no-report
CARGO_TARGET_DIR="$wasm_target" cargo +"$toolchain" llvm-cov report --target wasm32-unknown-unknown --json --summary-only --output-path coverage/rust/wasm-summary.json

node scripts/report-rust-coverage.cjs "$toolchain" "$native_target/llvm-cov-target" "$wasm_target/llvm-cov-target"
