#!/usr/bin/env bash
set -euo pipefail

# The human and the field must carry the same stack of timed team effects.
#
# Death Arena's per-frame speed is the product of two kinds of modifier. The
# per-car *_speed_multiplier feel modules (slipstream, comeback, wall scrape,
# flag carry) are guarded for human/field parity by
# scripts/check_speed_modifier_parity.sh. The OTHER kind is the stack of timed
# team effects a match layers on: the nitro boost, engine integrity, the wreck
# stun and surge, and the engine sabotage. Each is a per-team pool read through a
# *_multiplier method, NOT a shared *_speed_multiplier function, so the existing
# parity guard deliberately does not see them (it says so in as many words).
#
# That stack is folded in two hand-maintained places: the human's
# player_effect_multiplier in src/gameplay/player/movement.rs and the field's
# team_movement_multiplier in src/gameplay/virtual_player/drive.rs. Nothing stops
# a future change from adding a sixth timed effect to one and forgetting the
# other, and the result is the same silent fairness bug the speed-modifier guard
# was built to prevent: the field crawls under a sabotage the human shrugs off,
# or the human is stunned by a wreck the field drives through. No unit test
# catches it, because each system passes its own suite.
#
# This guard closes that gap for the timed-effect stack. It reads the set of
# Option<&Effect> resources each of the two folds consumes and asserts they are
# identical, so an effect wired into one and missed in the other fails the build
# instead of shipping an unfair match.

human="src/gameplay/player/movement.rs"
ai="src/gameplay/virtual_player/drive.rs"

for required in "${human}" "${ai}"; do
  if [[ ! -f "${required}" ]]; then
    cat >&2 <<ERROR
Missing ${required}: the timed-effect parity cannot be verified without it.
ERROR
    exit 1
  fi
done

# The timed-effect resource types a fold consumes, read from the Option<&Type>
# parameters of its signature (from the `fn <name>(` line to the `-> f32` return).
# A fold that bundles its resources differently in future yields none here, which
# the emptiness guard below turns into a loud failure rather than a vacuous pass.
effect_types_in() {
  local file="$1" function="$2"
  awk -v marker="fn ${function}(" '
    index($0, marker) { capturing = 1 }
    capturing {
      print
      if ($0 ~ /-> f32/) { exit }
    }
  ' "${file}" |
    grep -oE 'Option<&[A-Za-z0-9_]+>' |
    sed -E 's/^Option<&//; s/>$//' |
    sort -u
}

human_effects="$(effect_types_in "${human}" player_effect_multiplier)"
ai_effects="$(effect_types_in "${ai}" team_movement_multiplier)"

if [[ -z "${human_effects}" || -z "${ai_effects}" ]]; then
  cat >&2 <<'ERROR'
No timed-effect resources found in one of the two movement folds.

The guard derives its work from the Option<&Effect> parameters of the human's
player_effect_multiplier and the field's team_movement_multiplier. Finding none
means a fold was renamed or now bundles its resources differently and this guard
has gone blind: update it to match rather than letting it pass vacuously.
ERROR
  exit 1
fi

readarray -t missing_in_human < <(comm -13 <(echo "${human_effects}") <(echo "${ai_effects}"))
readarray -t missing_in_ai < <(comm -23 <(echo "${human_effects}") <(echo "${ai_effects}"))

status=0

if ((${#missing_in_human[@]} > 0)); then
  cat >&2 <<ERROR
Timed effect(s) the field applies but the human does not (${human}):

$(printf '  %s\n' "${missing_in_human[@]}")

The human must drive under the same timed effects as the field. Fold each effect
above into player_effect_multiplier.
ERROR
  status=1
fi

if ((${#missing_in_ai[@]} > 0)); then
  cat >&2 <<ERROR
Timed effect(s) the human applies but the field does not (${ai}):

$(printf '  %s\n' "${missing_in_ai[@]}")

The field must drive under the same timed effects as the human. Fold each effect
above into team_movement_multiplier.
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

count="$(echo "${human_effects}" | wc -l | tr -d ' ')"
echo "Checked ${count} timed-effect multipliers are folded by both the human and the field."
