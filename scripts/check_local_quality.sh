#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo clippy -- -D warnings
cargo test
cargo check
