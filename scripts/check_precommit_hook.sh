#!/usr/bin/env bash
set -euo pipefail

# Keep the rusty-hook pre-commit gate in lock-step with the CI Rust gates.
#
# The pre-commit hook in .rusty-hook.toml exists to catch, before a commit even
# lands, the exact lints CI runs, so feedback arrives at the keyboard instead of
# minutes later in a failed CI run (the project's shift-left rule). Its own
# comment promises to "mirror the CI gates exactly". Nothing enforced that
# promise, so a gate added to CI but not to the hook drifts the two apart
# silently: a wasm-only break, for instance, sails through the local hook and
# only trips in CI (or, worse, on the live demo). This guard closes that gap by
# asserting the hook still carries every Rust gate CI does.
#
# It checks command substrings, mirroring scripts/check_ci_workflow.sh, so a gate
# cannot be dropped or weakened (e.g. losing --all-features or the wasm target)
# without tripping here.

hook_file=".rusty-hook.toml"

if [[ ! -f "${hook_file}" ]]; then
  cat >&2 <<ERROR
Missing ${hook_file}: the pre-commit hook must mirror the CI Rust gates so the
same lints run before a commit lands, not minutes later in CI.
ERROR
  exit 1
fi

# Every Rust gate CI runs (see .github/workflows/ci.yml) must also run in the
# pre-commit hook, so a bypass of CI is the only way a regression reaches a PR.
required_gates=(
  "cargo check --all-targets --all-features"
  "cargo fmt --all -- --check"
  "cargo clippy --all-targets --all-features -- -D warnings"
  "cargo clippy --target wasm32-unknown-unknown --all-features -- -D warnings"
  "cargo test --all-targets --all-features"
)

for gate in "${required_gates[@]}"; do
  if ! grep -Fq "${gate}" "${hook_file}"; then
    cat >&2 <<ERROR
${hook_file} pre-commit hook must mirror the CI gate: ${gate}
The hook catches the CI lints before a commit lands; a gate present in CI but
missing here drifts the two apart, so a regression that gate would catch slips
past the local commit and only surfaces later in CI or on the live demo.
Add it to the pre-commit command in ${hook_file}.
ERROR
    exit 1
  fi
done

echo "Pre-commit hook mirrors the ${#required_gates[@]} CI Rust gates."
