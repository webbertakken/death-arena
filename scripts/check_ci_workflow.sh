#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/ci.yml"

if grep -Fq "git reset --hard" "${workflow}"; then
  cat >&2 <<'ERROR'
CI workflow must not use git reset --hard.
Checkout and git lfs pull should leave the workspace ready without destructive cleanup.
ERROR
  exit 1
fi

if ! grep -Fq "cargo check --all-targets --all-features" "${workflow}"; then
  cat >&2 <<'ERROR'
CI cargo check must cover all targets and all features.
Use: cargo check --all-targets --all-features
ERROR
  exit 1
fi

if ! grep -Fq "cargo test --all-targets --all-features" "${workflow}"; then
  cat >&2 <<'ERROR'
CI cargo test must cover all targets and all features.
Use: cargo test --all-targets --all-features
ERROR
  exit 1
fi

if ! grep -Fq "cargo clippy --all-targets --all-features -- -D warnings" "${workflow}"; then
  cat >&2 <<'ERROR'
CI cargo clippy must cover all targets and all features with warnings denied.
Use: cargo clippy --all-targets --all-features -- -D warnings
ERROR
  exit 1
fi
