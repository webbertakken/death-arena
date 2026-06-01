#!/usr/bin/env bash
set -euo pipefail

bash scripts/check_pages_workflow.sh
bash scripts/check_ci_workflow.sh
bash scripts/check_shell_scripts.sh
env -u NO_COLOR trunk build --release --public-url /death-arena/
bash scripts/check_pages_dist.sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo check --all-targets --all-features
