#!/usr/bin/env bash
set -euo pipefail

# Every asset path the game loads at runtime must resolve to a file that actually
# ships in assets/.
#
# Bevy's asset_server.load() takes a path as a plain string, so a typo, a rename,
# or a deleted file is invisible to the compiler: the code builds, the -D warnings
# gate is clean, and the break only shows up at runtime. Worse, a missing asset
# does not panic (the crate-root panic guard never fires); Bevy just logs an
# AssetIo error and hands back a handle that never resolves, so the car spawns
# without its sprite, the UI renders without its font, or the engine runs silent.
# That is exactly the "silent degradation" the reliability rules forbid, and it
# would ship straight to the live GitHub Pages demo, where no one is reading the
# browser console. This guard turns that runtime surprise into a build failure.
#
# Two classes of reference are checked:
#
#   1. Static loads: every string literal passed to a .load("...") call in the
#      crate source. These are the always-loaded core assets, the player car, the
#      UI font, the engine sound, the pickup sprite, the arena music and the arena
#      scene file, so a broken path here breaks the game outright. Dynamic loads,
#      .load(&path) built from data at runtime, cannot be resolved statically and
#      are covered transitively where their source value is itself a checked
#      literal (the arena scene file is loaded both ways).
#
#   2. Scene textures: every "relativePath" sprite entry in each *.2dtf scene
#      file, resolved against assets/textures/ exactly as the scene loader builds
#      it (see src/gameplay/arena/scene_loader.rs). A scene referencing a texture
#      that is not on disk renders an incomplete arena.
#
# Colliders are deliberately NOT required. The scene loader optimistically loads a
# sibling .collider for every sprite, but background sprites (the arena floor and
# the layout reference image) legitimately carry none, and the engine handles the
# absence by simply giving that sprite no physics body. A collider is therefore
# optional by design, so requiring one would flag a non-bug.

missing=()

# --- 1. Static .load("literal") paths, resolved against assets/ ---------------

mapfile -t rust_sources < <(git ls-files 'src/*.rs')

if ((${#rust_sources[@]} == 0)); then
  echo "No Rust source files found."
else
  mapfile -t load_paths < <(
    rg --no-filename --no-line-number -o -r '$1' \
      '\.load\(\s*"([^"]+)"' "${rust_sources[@]}" | sort -u
  )
  for path in "${load_paths[@]}"; do
    [[ -z "${path}" ]] && continue
    if [[ ! -f "assets/${path}" ]]; then
      missing+=("assets/${path}  (loaded via .load(\"${path}\"))")
    fi
  done
fi

# --- 2. Scene "relativePath" textures, resolved against assets/textures/ -------

mapfile -t scene_files < <(git ls-files 'assets/*.2dtf')

if ((${#scene_files[@]} == 0)); then
  echo "No scene (*.2dtf) files found."
else
  mapfile -t scene_paths < <(
    rg --no-filename --no-line-number -o -r '$1' \
      '"relativePath"\s*:\s*"([^"]+)"' "${scene_files[@]}" | sort -u
  )
  for path in "${scene_paths[@]}"; do
    [[ -z "${path}" ]] && continue
    if [[ ! -f "assets/textures/${path}" ]]; then
      missing+=("assets/textures/${path}  (scene sprite relativePath \"${path}\")")
    fi
  done
fi

# --- 3. Arena rotation scene literals, resolved against assets/ ----------------
#
# The arena rotation (src/gameplay/arena/selection.rs) names its scene files as
# plain string constants, not as .load("literal") calls: the loader picks one at
# runtime with .load(select_arena(..)), a dynamic load class 1 cannot see. So scan
# the source for every "*.2dtf" string literal and assert each ships in assets/,
# keeping the rotation's arenas covered now that the path is a constant rather than
# an inline load argument. Catches a typo or a removed arena file at build time
# instead of as an empty arena on the live demo.

if ((${#rust_sources[@]} > 0)); then
  mapfile -t arena_paths < <(
    rg --no-filename --no-line-number -o -r '$1' \
      '"([^"]+\.2dtf)"' "${rust_sources[@]}" | sort -u
  )
  for path in "${arena_paths[@]}"; do
    [[ -z "${path}" ]] && continue
    if [[ ! -f "assets/${path}" ]]; then
      missing+=("assets/${path}  (arena rotation scene literal \"${path}\")")
    fi
  done
fi

# --- Report -------------------------------------------------------------------

if ((${#missing[@]} > 0)); then
  cat >&2 <<'ERROR'
Asset paths referenced by the game do not exist in assets/.

A path passed to asset_server.load() (or a scene sprite's relativePath) points at
a file that is not on disk. Bevy will not crash; it will silently fail to load the
asset, so the game ships missing its sprite, font, sound or texture. Fix the path,
restore the file, or remove the reference.

Missing:
ERROR
  for entry in "${missing[@]}"; do
    echo "  ${entry}" >&2
  done
  exit 1
fi

echo "Checked ${#rust_sources[@]} Rust source file(s) and ${#scene_files[@]} scene file(s); every referenced asset exists."
