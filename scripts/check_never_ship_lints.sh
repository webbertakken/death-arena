#!/usr/bin/env bash
set -euo pipefail

# Guard the crate-root never-ship lint block from silent removal.
#
# src/main.rs denies a set of "never-ship" lints that each turn a latent defect
# into an instant abort hard-crashing the WASM canvas the moment it runs inside
# an ECS system every frame: a stray dbg!/print!/println! (clippy::dbg_macro /
# print_stdout / print_stderr), a placeholder panic (clippy::todo /
# clippy::unimplemented), an explicit panic (clippy::panic / clippy::unreachable)
# and the implicit panic a .unwrap()/.expect() smuggles in (clippy::unwrap_used /
# clippy::expect_used). Those denies ARE the panic-guard gate: clippy only fails
# on them while the attributes sit in the crate root. Delete or downgrade one and
# clippy passes green again with the gap wide open, because there is then nothing
# left to warn about.
#
# This is the mirror of scripts/check_rust_suppressions.sh: that guard stops a bad
# crate-wide #![allow(unused/dead_code)] being ADDED; this one stops a good
# crate-root #![deny(...)] never-ship lint being REMOVED. Together they keep the
# crate-root lint posture tamper-evident, so the panic-guard gate cannot erode
# without a gate failing.

required_lints=(
  clippy::dbg_macro
  clippy::print_stdout
  clippy::print_stderr
  clippy::todo
  clippy::unimplemented
  clippy::panic
  clippy::unreachable
  clippy::unwrap_used
  clippy::expect_used
)

root_files=()
for candidate in src/main.rs src/lib.rs; do
  if git ls-files --error-unmatch "${candidate}" >/dev/null 2>&1; then
    root_files+=("${candidate}")
  fi
done

if ((${#root_files[@]} == 0)); then
  echo >&2 "No crate root (src/main.rs or src/lib.rs) found to check for never-ship deny lints."
  exit 1
fi

# Pull the content of every crate-level #![deny(...)] block, flattened across
# lines, so each lint is checked wherever rustfmt happens to lay the block out
# (one lint per line today, but a future reflow must not blind the guard).
deny_blocks="$(rg --multiline --multiline-dotall --only-matching --no-filename \
  --no-line-number --color never '#!\[deny\(.*?\)\]' "${root_files[@]}" || true)"

missing=()
for lint in "${required_lints[@]}"; do
  if ! grep -Eq "${lint}"'([^a-z0-9_]|$)' <<<"${deny_blocks}"; then
    missing+=("${lint}")
  fi
done

if ((${#missing[@]} > 0)); then
  cat >&2 <<ERROR
Crate-root never-ship lint(s) missing from a #![deny(...)] block:

$(printf '  %s\n' "${missing[@]}")

These denies are the panic-guard gate (see src/main.rs): each turns a WASM-canvas
hard-crash into a clippy error caught at the door. Removing or downgrading one
silently reopens the gap, because clippy then has nothing to warn about. Restore
the lint to the crate-root #![deny(...)] block instead of dropping it.
ERROR
  exit 1
fi

echo "Checked ${#root_files[@]} crate root file(s) for ${#required_lints[@]} never-ship deny lints."
