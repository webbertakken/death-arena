#!/usr/bin/env bash
set -euo pipefail

# The field must drive every commitment-scaled feel lever on its own personality.
#
# Death Arena pits one human against a field of virtual players, and each AI car
# carries a driving personality: its cornering commitment (corner_throttle) sets
# how hard it presses the in-flight feel levers, so a keen driver claws a deficit
# back harder, sheds a leader's burden lighter, and digs into a flag-rally,
# escort, chase- and escort-resolve more fiercely than a disciplined one. Every
# such lever is a public *_speed_multiplier taking a final corner_throttle, and
# the field's virtual_player_drive_system feeds each the car's own commitment
# (ai.corner_throttle) so the roster actually drives on its distinct personalities.
#
# That per-car commitment is hand-wired at each call site in the drive system.
# Nothing stops a future change from passing the neutral MIN_THROTTLE baseline
# (copy-pasted from the human's call), a stray literal, or any value but the car's
# own corner_throttle: the whole field silently collapses onto the all-rounder
# baseline and the roster stops driving on its personalities, a fairness drift no
# compiler catches (a throttle is a valid f32 argument either way) and no unit test
# catches (each lever's suite tests the lever, not the field's call site). The
# asymmetry only ever surfaces as a roster that all drives the same, a flatness the
# players can never quite name.
#
# This guard is the exact mirror of scripts/check_human_baseline_commitment.sh
# (which pins the human to MIN_THROTTLE): where that guard holds the personality-
# less human to the neutral baseline, this one holds the field to its own
# personality. It discovers every commitment-scaled feel lever (each public
# *_speed_multiplier defined outside the two movement systems whose signature takes
# a corner_throttle) and asserts the field applies each one with the car's own
# corner_throttle, so a lever fed the baseline or any other value on the field's
# side fails the build instead of shipping a roster that no longer drives on its
# personalities. Together the two guards pin both ends of the fairness contract.

field="src/gameplay/virtual_player/drive.rs"
human="src/gameplay/player/movement.rs"
personality="corner_throttle"

for required in "${field}" "${human}"; do
  if [[ ! -f "${required}" ]]; then
    cat >&2 <<ERROR
Missing ${required}: the field commitment personality cannot be verified without it.
ERROR
    exit 1
  fi
done

# The feel modules that define a shared *_speed_multiplier, the two movement
# systems excluded so a system's own private helper is never mistaken for a
# shared lever (mirrors scripts/check_speed_modifier_parity.sh).
readarray -t lever_files < <(
  git grep -lE 'pub fn [a-z_]+_speed_multiplier' -- \
    '*.rs' \
    ":(exclude)${field}" \
    ":(exclude)${human}" |
    sort -u
)

if ((${#lever_files[@]} == 0)); then
  cat >&2 <<'ERROR'
No feel modules defining a shared *_speed_multiplier were found.

This guard derives its work from the commitment-scaled feel levers. Finding none
means the naming convention changed and the guard has gone blind: update it to
match rather than letting it pass vacuously.
ERROR
  exit 1
fi

# The commitment-scaled levers: every shared *_speed_multiplier whose signature
# (from its `pub fn` line to the `-> f32` return) takes a corner_throttle. A
# lever without one (the slipstream, wall scrape, carry fatigue) is uniform
# across drivers and so has no personality to hold the field to.
readarray -t commitment_levers < <(
  awk '
    /pub fn [a-z_]+_speed_multiplier/ {
      name = $0
      sub(/^.*pub fn /, "", name)
      sub(/\(.*/, "", name)
      sig = $0
      if ($0 ~ /-> f32/) {
        if (sig ~ /corner_throttle/) print name
        capturing = 0
        next
      }
      pending = name
      capturing = 1
      next
    }
    capturing {
      sig = sig "\n" $0
      if ($0 ~ /-> f32/) {
        if (sig ~ /corner_throttle/) print pending
        capturing = 0
      }
    }
  ' "${lever_files[@]}" |
    sort -u
)

if ((${#commitment_levers[@]} == 0)); then
  cat >&2 <<'ERROR'
No commitment-scaled *_speed_multiplier levers (taking a corner_throttle) found.

This guard holds the field to its own personality on exactly those levers.
Finding none means the corner_throttle commitment axis was renamed or removed and
the guard has gone blind: update it to match rather than letting it pass
vacuously.
ERROR
  exit 1
fi

# Every call to a lever in the field's drive file, returned one call per line with
# its multi-line arguments joined, so the personality check below reads the whole
# argument list of each call. Full-line comments are dropped first so a lever named
# in a doc comment can neither be mistaken for a call nor satisfy one; a call is
# recognised by the `name(` token and captured by paren depth to its matching
# close. The `use` imports (no paren) are skipped for the same reason.
calls_to() {
  local name="$1"
  grep -vE '^[[:space:]]*//' "${field}" |
    awk -v fn="${name}" '
      {
        line = $0
        i = 1
        n = length(line)
        while (i <= n) {
          if (!active) {
            rest = substr(line, i)
            idx = index(rest, fn "(")
            if (idx == 0) { break }
            i = i + idx - 1
            active = 1
            depth = 0
            buf = ""
          }
          c = substr(line, i, 1)
          buf = buf c
          if (c == "(") {
            depth++
          } else if (c == ")") {
            depth--
            if (depth == 0) {
              print buf
              active = 0
              buf = ""
            }
          }
          i++
        }
        if (active) { buf = buf " " }
      }
    '
}

missing_call=()
not_personality=()
for lever in "${commitment_levers[@]}"; do
  readarray -t calls < <(calls_to "${lever}")
  if ((${#calls[@]} == 0)); then
    missing_call+=("${lever}")
    continue
  fi
  for call in "${calls[@]}"; do
    if ! grep -q "${personality}" <<<"${call}"; then
      not_personality+=("${lever}")
      break
    fi
  done
done

status=0

if ((${#missing_call[@]} > 0)); then
  cat >&2 <<ERROR
Commitment-scaled lever(s) the field never applies (${field}):

$(printf '  %s\n' "${missing_call[@]}")

The field must drive under the same feel levers as the human. Apply each lever
above in the drive system, passing the car's own ai.${personality} so each driver
presses the lever on its own commitment. (A lever genuinely missing from the field
is also a speed-modifier parity break; see scripts/check_speed_modifier_parity.sh.)
ERROR
  status=1
fi

if ((${#not_personality[@]} > 0)); then
  cat >&2 <<ERROR
Commitment-scaled lever(s) the field applies off the car's own personality (${field}):

$(printf '  %s\n' "${not_personality[@]}")

Each virtual player has a driving personality: the field must pass the car's own
ai.${personality} to every commitment-scaled lever so the roster drives on its
distinct commitments. A call passing the neutral MIN_THROTTLE baseline or any other
value silently flattens the whole field onto the all-rounder baseline and breaks
the personality system. Pass the car's own ai.${personality} as the corner_throttle
argument of each call above.
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

echo "Checked ${#commitment_levers[@]} commitment-scaled levers are applied by the field on the car's own ${personality}."
