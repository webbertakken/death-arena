#!/usr/bin/env bash
set -euo pipefail

workflow=".github/workflows/pages.yml"

if grep -Fq "git reset --hard" "${workflow}"; then
  cat >&2 <<'ERROR'
Pages workflow must not use git reset --hard.
Checkout and git lfs pull should leave the workspace ready without destructive cleanup.
ERROR
  exit 1
fi

if ! grep -Fq 'trunk build --release --public-url "/${GITHUB_REPOSITORY#*/}/"' "${workflow}"; then
  cat >&2 <<'ERROR'
Pages workflow must build with an absolute GitHub Pages base path.
Use: trunk build --release --public-url "/${GITHUB_REPOSITORY#*/}/"
ERROR
  exit 1
fi

if grep -Eq "wasm-opt-action|wasm-opt|dist/\\*\\.wasm" "${workflow}"; then
  cat >&2 <<'ERROR'
Pages workflow mutates wasm after Trunk builds dist.
That can invalidate Trunk-generated resource integrity hashes.
Let Trunk own wasm optimisation before it writes final artefacts.
ERROR
  exit 1
fi

if ! grep -Fq "bash scripts/check_pages_dist.sh" "${workflow}"; then
  cat >&2 <<'ERROR'
Pages workflow must validate generated dist asset paths after Trunk builds.
Use: bash scripts/check_pages_dist.sh
ERROR
  exit 1
fi
