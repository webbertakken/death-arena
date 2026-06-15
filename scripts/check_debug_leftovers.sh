#!/usr/bin/env bash
set -euo pipefail

# Debug-output macros must never reach a commit in the crate source.
#
# A stray dbg! call is a debugging leftover, and a raw print!/println!/eprint!/
# eprintln! dumps straight to stdout, bypassing the Bevy/log logging the engine
# and the reliability rules rely on: every diagnostic must stay surfaceable
# through the log, not scattered across stdout where it is invisible to the
# usual channels. Default clippy denies none of these macros, so without this
# guard one could land and silently weaken the logging discipline. None appears
# anywhere in the source today; this gate keeps it that way, failing the build
# the moment one is introduced.
#
# Only the crate source under src/ is scanned. A Cargo build script legitimately
# prints cargo: directives to stdout, so the root build.rs is deliberately left
# out of scope.
#
# The pattern matches the macro INVOCATION form (a name, then an opening paren),
# so a prose mention in a doc comment (prefer info! over println!) does not trip
# it; only an actual call such as println!(...) does.

readarray -d '' rust_sources < <(git ls-files -z 'src/*.rs')

if ((${#rust_sources[@]} == 0)); then
  echo "No Rust source files found."
  exit 0
fi

if matches="$(rg --line-number --color never '\b(dbg|print|println|eprint|eprintln)!\s*\(' "${rust_sources[@]}")"; then
  cat >&2 <<ERROR
Debug-output macros are not allowed in the crate source.

Use Bevy's logging macros (info!, warn!, error!, debug!, trace!) instead of dbg!
or a raw print!/println!/eprint!/eprintln!, so every diagnostic stays surfaceable
through the log rather than dumped to stdout.

${matches}
ERROR
  exit 1
fi

echo "Checked ${#rust_sources[@]} Rust source file(s) for debug-output leftovers."
