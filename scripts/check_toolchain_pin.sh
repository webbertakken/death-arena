#!/usr/bin/env bash
set -euo pipefail

# Validate that the Rust toolchain is pinned to an exact version everywhere.
#
# Without a pin, every build floats on "latest stable": local machines, the
# pre-commit hook and CI can each compile with a different compiler. A new stable
# release then breaks the build with no code change, most sharply through the
# `-D warnings` clippy gate (nursery + pedantic), which fails the moment a fresh
# lint is added, and through rustfmt, whose formatting can shift between releases.
# A `rust-toolchain.toml` makes rustup select the pinned version automatically, so
# this guard keeps that pin concrete and in lock-step with every workflow.

toolchain_file="rust-toolchain.toml"

if [[ ! -f "${toolchain_file}" ]]; then
  cat >&2 <<ERROR
Missing ${toolchain_file}.
Pin the Rust toolchain so local and CI builds are reproducible and a new stable
release cannot break the -D warnings clippy gate on an unchanged codebase.
ERROR
  exit 1
fi

pinned="$(grep -E '^[[:space:]]*channel[[:space:]]*=' "${toolchain_file}" |
  head -n 1 |
  sed -E 's/.*=[[:space:]]*"([^"]+)".*/\1/')"

if [[ -z "${pinned}" ]]; then
  echo "${toolchain_file} must set a [toolchain] channel." >&2
  exit 1
fi

if ! [[ "${pinned}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  cat >&2 <<ERROR
${toolchain_file} channel must be an exact version (e.g. 1.95.0), not a floating
channel like stable/beta/nightly, so a new release cannot silently change the
compiler, its lints, or rustfmt under an unchanged codebase.
Found: ${pinned}
ERROR
  exit 1
fi

# Every workflow that installs a Rust toolchain must pin the same exact version,
# so CI never floats on "latest stable" while the toolchain file pins a version.
shopt -s nullglob
workflows=(.github/workflows/*.yml)
shopt -u nullglob

mismatch=0
for workflow in "${workflows[@]}"; do
  while IFS= read -r ref; do
    if [[ "${ref}" != "${pinned}" ]]; then
      echo "${workflow}: dtolnay/rust-toolchain@${ref} does not match pinned ${pinned}." >&2
      mismatch=1
    fi
  done < <(grep -oE 'dtolnay/rust-toolchain@[^[:space:]"]+' "${workflow}" | sed -E 's#.*@##')
done

if ((mismatch)); then
  cat >&2 <<ERROR
A workflow installs a Rust toolchain other than the pinned ${pinned}.
Pin every dtolnay/rust-toolchain action to @${pinned} (matching ${toolchain_file})
so local and CI builds use the same compiler.
ERROR
  exit 1
fi

echo "Toolchain pinned to ${pinned} in ${toolchain_file} and all workflows."
