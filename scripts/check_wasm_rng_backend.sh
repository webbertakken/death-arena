#!/usr/bin/env bash
set -euo pipefail

# Guarantee the WebAssembly build keeps a working random-number backend.
#
# The browser build (wasm32-unknown-unknown), the artefact deployed to the live
# GitHub Pages demo, has no operating-system entropy source. `rand` reaches for
# entropy through `getrandom`, whose default wasm backend is `Unsupported`: every
# call errors, so `rand::thread_rng()` / `rand::random()` panic at runtime. The
# arena loader picks its arena with `rand::random()` the instant a match starts
# (on entering AppState::Loading), so without a real wasm entropy backend the demo
# crashes on the first match, a failure no `cargo check`, clippy or test gate can
# see because it only surfaces at runtime in a browser.
#
# The fix is `getrandom`'s `js` feature, which wires entropy to the browser's Web
# Crypto API. This guard keeps that configuration from silently regressing: it
# fails unless Cargo.toml enables `getrandom`'s `js` feature for the wasm target.
# It is a cheap manifest check (no network, no wasm target install) so it runs in
# the same guards job as the other manifest guards.

manifest="Cargo.toml"

if [[ ! -f "${manifest}" ]]; then
  echo "Missing ${manifest}: cannot verify the wasm random-number backend." >&2
  exit 1
fi

target_table="[target.'cfg(target_arch = \"wasm32\")'.dependencies]"
if ! grep -Fq "${target_table}" "${manifest}"; then
  cat >&2 <<ERROR
${manifest} must declare a wasm target dependency table so the browser build can
pin a working getrandom backend:
  ${target_table}
Without it, rand::random() panics at runtime in the browser the moment a match
starts, crashing the live GitHub Pages demo.
ERROR
  exit 1
fi

if ! grep -Eq '^[[:space:]]*getrandom[[:space:]]*=.*features[[:space:]]*=[[:space:]]*\[[^]]*"js"[^]]*\]' "${manifest}"; then
  cat >&2 <<ERROR
${manifest} must enable getrandom's "js" feature for the wasm target, e.g.:
  getrandom = { version = "0.2", features = ["js"] }
The js feature routes entropy to the browser's Web Crypto API; without it the
default wasm backend is Unsupported and rand::random() panics at runtime, so the
arena loader crashes the live demo on the first match.
ERROR
  exit 1
fi

echo "wasm build enables getrandom's js backend (rand works in the browser)."
