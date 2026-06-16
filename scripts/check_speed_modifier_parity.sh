#!/usr/bin/env bash
set -euo pipefail

# The human and the field must drive on identical terms.
#
# Death Arena pits one human against a field of virtual players, and the promise
# that holds the contest together is that both move under the same physics: the
# slipstream tow, the trailing team's catch-up urge, the flag-carry tax and the
# wall scrape all read the same way for the human's car_movement_system and the
# field's virtual_player_drive_system. Each lives in its own feel module as a
# single public *_speed_multiplier, deliberately bilateral, and the module docs
# say so in as many words ("read by both movement systems, the human's and the
# field's, so the human and the AI ... on the identical terms").
#
# That parity is hand-maintained. Nothing stops a future change from wiring a new
# shared modifier into one movement system and forgetting the other, and the
# result is a silent fairness bug: the field drafts where the human cannot, or
# the human bleeds speed on a wall the field ignores. No unit test catches it,
# because each system passes its own suite; the asymmetry only ever surfaces as a
# feel the players can never quite name.
#
# This guard closes that gap. It discovers every public *_speed_multiplier
# defined outside the two movement systems (the shared feel modules) and asserts
# each one is applied in BOTH systems, so a modifier added to one and missed in
# the other fails the build instead of shipping an unfair match. A modifier that
# is genuinely one-sided belongs as a private helper inside its own consumer
# (like drive.rs's local car_speed_multiplier), not as a shared public modifier.

human="src/gameplay/player/movement.rs"
ai="src/gameplay/virtual_player/drive.rs"

for required in "${human}" "${ai}"; do
  if [[ ! -f "${required}" ]]; then
    cat >&2 <<ERROR
Missing ${required}: the speed-modifier parity cannot be verified without it.
ERROR
    exit 1
  fi
done

# The shared, cross-cutting speed modifiers: every public *_speed_multiplier
# defined anywhere but the two movement systems themselves. The exclude pathspecs
# keep a system's own private helper (drive.rs's car_speed_multiplier) out of the
# bilateral set, since a private helper is local to one system by design.
readarray -t shared_modifiers < <(
  git grep -hoE 'pub (const )?fn [a-z_]+_speed_multiplier' -- \
    '*.rs' \
    ":(exclude)${human}" \
    ":(exclude)${ai}" |
    grep -oE '[a-z_]+_speed_multiplier$' |
    sort -u
)

if ((${#shared_modifiers[@]} == 0)); then
  cat >&2 <<'ERROR'
No shared *_speed_multiplier functions found outside the two movement systems.

The parity guard derives its work from the public per-car speed modifiers in the
feel modules (slipstream, comeback, wall_scrape, ctf). Finding none means the
naming convention changed and this guard has gone blind: update it to match the
new convention rather than letting it pass vacuously.
ERROR
  exit 1
fi

# A modifier counts as applied only where it appears on a non-comment line: a name
# mentioned in prose is not a system that actually drives by it. Clippy already
# denies an unused import, so any modifier genuinely wired in shows up in code.
strip_comments() {
  grep -vE '^[[:space:]]*//' "$1" || true
}

human_code="$(strip_comments "${human}")"
ai_code="$(strip_comments "${ai}")"

missing_in_human=()
missing_in_ai=()
for modifier in "${shared_modifiers[@]}"; do
  if ! grep -qwF "${modifier}" <<<"${human_code}"; then
    missing_in_human+=("${modifier}")
  fi
  if ! grep -qwF "${modifier}" <<<"${ai_code}"; then
    missing_in_ai+=("${modifier}")
  fi
done

status=0

if ((${#missing_in_human[@]} > 0)); then
  cat >&2 <<ERROR
Speed modifier(s) the field applies but the human does not (${human}):

$(printf '  %s\n' "${missing_in_human[@]}")

The human must drive under the same physics as the field. Apply each modifier
above in car_movement_system, or, if it is genuinely field-only, move it out of
the shared feel modules into a private helper in ${ai}.
ERROR
  status=1
fi

if ((${#missing_in_ai[@]} > 0)); then
  cat >&2 <<ERROR
Speed modifier(s) the human applies but the field does not (${ai}):

$(printf '  %s\n' "${missing_in_ai[@]}")

The field must drive under the same physics as the human. Apply each modifier
above in virtual_player_drive_system, or, if it is genuinely human-only, move it
out of the shared feel modules into a private helper in ${human}.
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

echo "Checked ${#shared_modifiers[@]} shared speed modifiers are applied by both the human and the field."
