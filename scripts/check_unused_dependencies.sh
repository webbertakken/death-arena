#!/usr/bin/env bash
set -euo pipefail

# Scan Cargo.toml against the crate source so a dependency that is declared but
# never used cannot linger in the manifest. Unused dependencies are pure cost:
# they bloat the dependency tree, slow cold builds, widen the surface the security
# advisory scan has to clear, and mislead a reader about what the crate actually
# relies on. This turns the one-off cleanup that removed `image` and `winit` (both
# carried since the first iteration, neither referenced in code, both still pulled
# transitively by Bevy so dropping the direct edges changed no resolved feature)
# into a standing gate that fails the build the moment a fresh unused dependency is
# declared.
#
# cargo-machete reads the manifest and greps the source; it never builds the
# project, so the gate is cheap. A dependency that is present only to pin a feature
# of a transitive crate (and so has no direct `use`) belongs in the manifest's
# `[package.metadata.cargo-machete] ignored = [...]` list, which the tool honours,
# rather than being deleted.

if ! command -v cargo-machete >/dev/null 2>&1; then
  cat >&2 <<'ERROR'
cargo-machete is not installed, so the unused-dependency scan cannot run.
Install it with: cargo install cargo-machete --locked
ERROR
  exit 1
fi

# Exits non-zero when any crate declares a dependency it does not use. Invoked as
# the binary directly (not via `cargo machete`) so the gate does not depend on
# cargo being on PATH to dispatch the subcommand.
cargo-machete

echo "Scanned Cargo.toml for unused dependencies."
