#!/usr/bin/env bash
set -euo pipefail

bash scripts/check_pages_workflow.sh
bash scripts/check_ci_workflow.sh
bash scripts/check_scheduled_audit_workflow.sh
bash scripts/check_toolchain_pin.sh
bash scripts/check_shell_scripts.sh
bash scripts/check_workflow_lint.sh
bash scripts/check_rust_safety.sh
bash scripts/check_rust_suppressions.sh
bash scripts/check_unused_dependencies.sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo check --all-targets --all-features
bash scripts/check_rust_docs.sh
bash scripts/check_security_advisories.sh
env -u NO_COLOR trunk build --release --public-url /death-arena/
bash scripts/check_pages_dist.sh
