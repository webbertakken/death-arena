#!/usr/bin/env bash
set -euo pipefail

# Every per-frame gameplay system must down tools the instant the match is won.
#
# A CTF round ends the frame CtfMatchResult.winner is set, by a capture, a
# sudden-death golden goal or the clock expiring. From that frame on the world
# must freeze: the human's car stops, the field stops, and integrity stops being
# ground down, so the result the player is shown is the result that stands. Each
# of the three per-frame systems that mutate that world enforces this itself,
# with an identical early-return the moment a winner exists:
#
#   if match_result.as_ref().is_some_and(|result| result.winner.is_some()) {
#       return;
#   }
#
# That halt is hand-maintained in three separate files. Drop it from one and the
# match no longer ends cleanly: the field keeps driving while the human is frozen
# (or the other way round), or ram_damage_system keeps wrecking cars and paying
# bounties after the final whistle, banking cash on a round already decided. It
# is the same silent fairness bug the speed- and effect-multiplier parity guards
# were built to stop, one freeze gate out of step across systems that must move
# as one.
#
# The compiler will not catch it. ram_damage_system takes match_result as a plain
# function parameter, and Rust does not lint an unused parameter, so deleting its
# gate compiles clean with no warning at all; the two movement systems read it
# from a destructured tuple whose other fields stay live, so a dropped gate need
# not leave anything unused either. No unit test catches it: each system passes
# its own suite with or without the gate. This guard closes that gap, asserting
# every per-frame world-mutating system still carries the match-over halt.

# The per-frame systems that mutate the arena and so must halt once a winner is
# settled, each as a "file::function" pair. A new system that drives cars or
# wears integrity down every frame belongs on this list.
files=(
  "src/gameplay/player/movement.rs"
  "src/gameplay/virtual_player/drive.rs"
  "src/gameplay/combat/mod.rs"
)
functions=(
  "car_movement_system"
  "virtual_player_drive_system"
  "ram_damage_system"
)

# The shared resolution-halt signal: the winner check every gating system reads.
# Matched as a fixed string on a code line, so a system that still drives after
# the match is decided fails the build instead of shipping a round that never
# cleanly ends.
gate="winner.is_some()"

# How many code lines past a system's `fn name(` to scan for its halt gate. The
# gate sits at the very top of each system (right after the signature), well
# inside this window; comment lines are skipped so the budget counts real code.
window=45

missing=()
for index in "${!files[@]}"; do
  file="${files[${index}]}"
  function="${functions[${index}]}"

  if [[ ! -f "${file}" ]]; then
    cat >&2 <<ERROR
Missing ${file}: the match-over halt parity cannot be verified without it.
ERROR
    exit 1
  fi

  # The opening window of the system's definition, comment lines dropped so a
  # commented-out marker or a doc line can neither trigger nor satisfy the scan.
  block="$(awk -v marker="fn ${function}(" -v window="${window}" '
    /^[[:space:]]*\/\// { next }
    index($0, marker) && !started { started = 1; remaining = window }
    started && remaining > 0 { print; remaining-- }
  ' "${file}")"

  if [[ -z "${block}" ]]; then
    cat >&2 <<ERROR
Could not find ${function} in ${file}.

This guard scans the opening of each per-frame system for its match-over halt.
Finding nothing means the system was renamed or moved and the guard has gone
blind: update the files/functions list rather than letting it pass vacuously.
ERROR
    exit 1
  fi

  if ! grep -qF "${gate}" <<<"${block}"; then
    missing+=("${file}::${function}")
  fi
done

if ((${#missing[@]} > 0)); then
  cat >&2 <<ERROR
Per-frame system(s) missing the match-over halt:

$(printf '  %s\n' "${missing[@]}")

Each system above mutates the arena every frame but no longer stops when the
round is won, so play carries on past the final whistle. Reinstate the gate at
the top of the system:

  if match_result.as_ref().is_some_and(|result| result.winner.is_some()) {
      return;
  }

If the halt was deliberately reshaped, update this guard to match the new form.
ERROR
  exit 1
fi

echo "Checked ${#files[@]} per-frame gameplay systems halt once the match is won."
