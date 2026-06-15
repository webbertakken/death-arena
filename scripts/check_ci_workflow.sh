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

if ! grep -Fq "cargo clippy --target wasm32-unknown-unknown --all-features -- -D warnings" "${workflow}"; then
  cat >&2 <<'ERROR'
CI must lint the wasm32 ship target, the actual artefact deployed to GitHub Pages.
The check/test/clippy gates all run on the host target, so a wasm-only build break
(a host-only API, a dependency feature that fails on wasm, a target-gated path)
passes every PR gate and only surfaces later when the Pages workflow deploys from
main, breaking the live demo. The Pages workflow itself never runs on a pull
request, so without this step nothing validates the ship target before merge.
Lint the ship target on every push and PR with:
  cargo clippy --target wasm32-unknown-unknown --all-features -- -D warnings
ERROR
  exit 1
fi

if ! grep -Fq "cargo fmt --all -- --check" "${workflow}"; then
  cat >&2 <<'ERROR'
CI must verify formatting across the whole workspace, so a misformatted commit
that bypassed the local hook cannot land unnoticed.
Use: cargo fmt --all -- --check
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
  "bash scripts/check_scheduled_audit_workflow.sh"
  "bash scripts/check_toolchain_pin.sh"
  "bash scripts/check_precommit_hook.sh"
  "bash scripts/check_shell_scripts.sh"
  "bash scripts/check_workflow_lint.sh"
  "bash scripts/check_rust_safety.sh"
  "bash scripts/check_rust_suppressions.sh"
  "bash scripts/check_never_ship_lints.sh"
  "bash scripts/check_debug_leftovers.sh"
  "bash scripts/check_temporary_fix_markers.sh"
  "bash scripts/check_unused_dependencies.sh"
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
