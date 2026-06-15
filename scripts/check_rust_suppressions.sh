#!/usr/bin/env bash
set -euo pipefail

# A crate-level inner attribute (#![allow(...)]) in the crate root applies to the
# WHOLE crate, so a blanket `unused`/`dead_code` allow there silences those
# warnings everywhere and defeats the -D warnings gate, letting dead code and
# stray bindings land unnoticed. Inner attributes in module files are
# module-scoped and a targeted clippy lint (clippy::unused_self) is fine, so this
# guard inspects only the crate root and ignores clippy:: lints.
root_files=()
for candidate in src/main.rs src/lib.rs; do
  if git ls-files --error-unmatch "${candidate}" >/dev/null 2>&1; then
    root_files+=("${candidate}")
  fi
done

if ((${#root_files[@]} == 0)); then
  echo "No crate root found."
  exit 0
fi

# Pull crate-level allow lines, strip clippy:: lint names, then look for a bare
# rustc unused/dead_code lint left behind.
matches="$(rg --line-number --color never '#!\[allow\(' "${root_files[@]}" |
  sed -E 's/clippy::[a-z_]+//g' |
  rg --color never '\b(unused[a-z_]*|dead_code)\b' || true)"

if [[ -n "${matches}" ]]; then
  cat >&2 <<ERROR
Crate-level unused/dead_code lint suppressions are not allowed.

A crate-root #![allow(unused...)] or #![allow(dead_code)] masks warnings across
the ENTIRE crate, hiding dead code and stray bindings from the -D warnings gate.
Fix the underlying code instead, or scope a narrow module-level #[allow(...)].

${matches}
ERROR
  exit 1
fi

echo "Checked ${#root_files[@]} crate root file(s) for blanket unused/dead_code suppressions."
