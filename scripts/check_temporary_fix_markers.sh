#!/usr/bin/env bash
set -euo pipefail

# Temporary-fix markers must never reach a commit in the crate source.
#
# A TODO, FIXME, HACK or XXX marker is a promise to come back later that the
# project's rules forbid outright: fix every issue immediately, no temporary
# fixes, no placeholders. Left in the tree these markers quietly calcify into
# "this is just how it is", hiding deferred work and half-done paths behind a
# comment the build never complains about. clippy denies the todo! and
# unimplemented! macros (see the crate-root attributes in src/main.rs), but a
# plain `// TODO:` comment slips past every existing gate. This guard closes that
# gap, failing the build the moment a marker is introduced. None appears anywhere
# in the source today; this keeps it that way.
#
# Only the crate source under src/ is scanned. This guard script itself, the CI
# workflow and the other tooling legitimately name the markers they forbid, and
# they live outside src/, so they are deliberately out of scope.
#
# Markers are matched in their conventional UPPERCASE form, bounded as whole
# words. A constant or identifier such as HACK_RADIUS keeps its trailing word
# character, so the boundary does not match and a real name is never flagged;
# only a standalone marker like `// FIXME: ...` trips the gate. The lowercase
# clippy::todo lint name in src/main.rs is likewise left untouched.

readarray -d '' rust_sources < <(git ls-files -z 'src/*.rs')

if ((${#rust_sources[@]} == 0)); then
  echo "No Rust source files found."
  exit 0
fi

if matches="$(rg --line-number --color never '\b(TODO|FIXME|HACK|XXX)\b' "${rust_sources[@]}")"; then
  cat >&2 <<ERROR
Temporary-fix markers are not allowed in the crate source.

Fix the issue now rather than leaving a TODO/FIXME/HACK/XXX behind: the project
forbids temporary fixes and placeholders, and an unguarded marker hides deferred
work the build never surfaces. Resolve it, or capture genuine follow-up work in
the project's plans/ folder instead of a stray comment.

${matches}
ERROR
  exit 1
fi

echo "Checked ${#rust_sources[@]} Rust source file(s) for temporary-fix markers."
