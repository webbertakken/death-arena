#!/usr/bin/env bash
set -euo pipefail

# Pin every git dependency to an immutable commit, never a moving branch or tag.
#
# Cargo.toml pulls the physics engine bevy_rapier2d straight from a fork's git
# repository. A git dependency keyed only by `branch = "..."` (or `tag = "..."`)
# floats on whatever that ref points at: the moment the fork pushes a new commit,
# a `cargo update` silently swaps the whole physics/determinism engine under an
# otherwise unchanged codebase, exactly the drift the rust-toolchain.toml pin and
# the committed Cargo.lock exist to prevent. Cargo.lock pins today's commit, but
# the manifest intent still reads "latest on this branch", so the next
# `cargo update` is one keystroke from a different engine.
#
# This guard is the dependency mirror of scripts/check_toolchain_pin.sh: it
# asserts every git dependency in Cargo.toml carries a `rev = "<40-hex commit>"`
# pin, so the build is reproducible by intent and bumping the dependency is a
# deliberate, reviewed manifest edit rather than a silent side effect of an
# unrelated update. A short rev can grow ambiguous as history lands, so the pin
# must be a full 40-character commit SHA.

manifest="Cargo.toml"

if [[ ! -f "${manifest}" ]]; then
  echo "Missing ${manifest}: cannot verify git dependencies are pinned." >&2
  exit 1
fi

# Collapse each inline table { ... } onto one logical line and drop full-line
# comments, so a dependency declaration is scannable on a single line whether it
# is written inline (as today) or wrapped across several lines. Brace-depth
# tracking joins the continuation lines; the leading-# strip keeps a commented
# example from being read as a live declaration.
logical="$(awk '
  { sub(/^[[:space:]]*#.*$/, "") }
  {
    if (depth > 0) { buffer = buffer " " $0 } else { buffer = $0 }
    opens = gsub(/{/, "{")
    closes = gsub(/}/, "}")
    depth += opens - closes
    if (depth <= 0) {
      print buffer
      buffer = ""
      depth = 0
    }
  }
' "${manifest}")"

# A git dependency is any declaration carrying a `git = "..."` source key.
mapfile -t git_deps < <(grep -E 'git[[:space:]]*=[[:space:]]*"' <<<"${logical}" || true)

unpinned=()
for dep in "${git_deps[@]}"; do
  if ! grep -Eq 'rev[[:space:]]*=[[:space:]]*"[0-9a-fA-F]{40}"' <<<"${dep}"; then
    unpinned+=("${dep}")
  fi
done

if ((${#unpinned[@]} > 0)); then
  cat >&2 <<ERROR
Git dependency declared without an immutable rev pin in ${manifest}:

$(printf '  %s\n' "${unpinned[@]}")

A git dependency keyed by branch/tag (or with no rev at all) floats on a moving
ref, so a cargo update can silently swap the dependency for a different commit
under an unchanged codebase. Pin it to the exact commit with a full
rev = "<40-hex SHA>" (matching the commit Cargo.lock already resolves) so the
build is reproducible by intent and a bump is a deliberate, reviewed manifest
edit. This mirrors the rust-toolchain.toml pin enforced by
scripts/check_toolchain_pin.sh.
ERROR
  exit 1
fi

echo "Checked ${#git_deps[@]} git dependency declaration(s) in ${manifest} are pinned to an immutable 40-hex rev."
