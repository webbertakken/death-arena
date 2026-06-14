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

if ! grep -Fq "bash scripts/check_rust_docs.sh" "${workflow}"; then
  cat >&2 <<'ERROR'
CI must validate the crate documentation builds with rustdoc warnings denied, so
a renamed or removed item cannot silently break an intra-doc link.
Add a step that runs: bash scripts/check_rust_docs.sh
ERROR
  exit 1
fi

if ! grep -Fq "bash scripts/check_security_advisories.sh" "${workflow}"; then
  cat >&2 <<'ERROR'
CI must scan Cargo.lock against the RustSec advisory database, so a dependency
with a known security vulnerability cannot ship to players unnoticed.
Add a step that runs: bash scripts/check_security_advisories.sh
ERROR
  exit 1
fi

# The guard scripts that gate commits locally must also run in CI, so the same
# integrity rules (no unsafe, strict shell scripts, intact workflows) are
# enforced on every push and pull request, not just before a local commit.
required_guards=(
  "bash scripts/check_pages_workflow.sh"
  "bash scripts/check_ci_workflow.sh"
  "bash scripts/check_shell_scripts.sh"
  "bash scripts/check_rust_safety.sh"
  "bash scripts/check_rust_suppressions.sh"
)
for guard in "${required_guards[@]}"; do
  if ! grep -Fq "${guard}" "${workflow}"; then
    cat >&2 <<ERROR
CI workflow must run the local quality guard: ${guard}
The guards enforced before a local commit must also run in CI so a bypassed
hook cannot land unsafe code, a lax shell script, or a regressed workflow.
Add a step that runs: ${guard}
ERROR
    exit 1
  fi
done
