#!/usr/bin/env bash
set -euo pipefail

# Build the crate's documentation with every rustdoc warning denied, so a renamed
# or removed item can never silently rot the heavy intra-doc link culture this
# codebase relies on (e.g. [`pit_retreat_car`], [`compute_steering`]). A broken
# link is a real defect: it leaves the docs pointing at nothing and hides the
# cross-reference a reader was meant to follow.
#
# Flags:
# - --document-private-items keeps links to private helpers valid instead of
#   tripping the private-intra-doc-links lint, matching how the modules actually
#   cross-link public APIs to their private building blocks.
# - --no-deps scopes the build to our crate (dependency docs are not our concern).
# - --all-features mirrors the other Rust gates so the `dev` feature is covered.
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --all-features

echo "Checked crate documentation for broken intra-doc links and rustdoc warnings."
