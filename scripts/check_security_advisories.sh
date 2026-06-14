#!/usr/bin/env bash
set -euo pipefail

# Scan Cargo.lock against the RustSec advisory database so a dependency carrying a
# known security *vulnerability* cannot ship to players unnoticed. This automates
# the manual `cargo audit` pass that previously cleared four advisories by hand
# (bytes, mio, shlex, tracing-subscriber), turning a one-off chore into a standing
# gate that fails the build the moment a new vulnerability is disclosed against a
# crate in the tree.
#
# Why plain `cargo audit` (no `--deny warnings`):
# - A *vulnerability* is actionable and fails the gate: it almost always clears
#   with a semver-compatible, lockfile-only bump (`cargo update -p <crate>`), the
#   exact fix the four prior advisories took, so blocking on it is a real defect
#   caught early, not noise.
# - The *unmaintained-crate* warnings that remain (adler, fxhash, and friends) are
#   transitive through the pinned Bevy 0.9 / bevy_rapier2d fork and cannot be
#   resolved without a major engine upgrade that is deliberately out of scope.
#   cargo audit prints them for visibility but exits 0, so they surface in the log
#   without gating the build behind an unfixable dependency. Denying them would
#   force a suppression list for advisories no lockfile bump can clear.

if ! command -v cargo-audit >/dev/null 2>&1; then
  cat >&2 <<'ERROR'
cargo-audit is not installed, so the security advisory scan cannot run.
Install it with: cargo install cargo-audit --locked
ERROR
  exit 1
fi

# Fails (non-zero) on any crate with a known vulnerability; unmaintained-crate
# warnings are reported but do not fail the gate (see the rationale above).
cargo audit

echo "Scanned Cargo.lock for crates with known security vulnerabilities."
