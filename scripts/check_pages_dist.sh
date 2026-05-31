#!/usr/bin/env bash
set -euo pipefail

index="dist/index.html"

if [[ ! -f "${index}" ]]; then
  cat >&2 <<'ERROR'
Missing dist/index.html.
Run: trunk build --release --public-url /death-arena/
ERROR
  exit 1
fi

if grep -Fq "from 'death-arena/" "${index}"; then
  cat >&2 <<'ERROR'
Generated module import uses a bare specifier.
Expected root-relative imports starting with /death-arena/.
ERROR
  exit 1
fi

if grep -Eq "href=\"death-arena/|module_or_path: 'death-arena/" "${index}"; then
  cat >&2 <<'ERROR'
Generated asset URL is relative to the current page.
Expected root-relative URLs starting with /death-arena/.
ERROR
  exit 1
fi

if grep -Fq "/death-arena/death-arena/" "${index}"; then
  cat >&2 <<'ERROR'
Generated asset URL contains a duplicated GitHub Pages base path.
Expected exactly one /death-arena/ prefix.
ERROR
  exit 1
fi

if ! grep -Eq "from '/death-arena/[^']+\\.js'" "${index}"; then
  cat >&2 <<'ERROR'
Generated module import does not use the GitHub Pages base path.
Expected: from '/death-arena/<asset>.js'
ERROR
  exit 1
fi

if ! grep -Eq "module_or_path: '/death-arena/[^']+_bg\\.wasm'" "${index}"; then
  cat >&2 <<'ERROR'
Generated wasm loader path does not use the GitHub Pages base path.
Expected: module_or_path: '/death-arena/<asset>_bg.wasm'
ERROR
  exit 1
fi
