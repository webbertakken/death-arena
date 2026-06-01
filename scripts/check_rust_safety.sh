#!/usr/bin/env bash
set -euo pipefail

readarray -d '' rust_files < <(git ls-files -z '*.rs')

if ((${#rust_files[@]} == 0)); then
  echo "No Rust files found."
  exit 0
fi

if matches="$(rg --line-number --color never '\bunsafe\s*(\{|fn\b|impl\b|trait\b)' "${rust_files[@]}")"; then
  cat >&2 <<ERROR
Rust unsafe usage is not allowed without explicit approval.

${matches}
ERROR
  exit 1
fi

echo "Checked ${#rust_files[@]} Rust files for unsafe usage."
