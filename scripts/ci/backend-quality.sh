#!/bin/sh
set -eu

export CHRONACLE_SKIP_RUNTIME_DOWNLOADS=1

scripts/ci/test-pipeline.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
