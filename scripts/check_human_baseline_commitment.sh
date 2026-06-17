#!/usr/bin/env bash
set -euo pipefail

# The human must drive every commitment-scaled feel lever at the neutral baseline.
#
# Death Arena pits one human against a field of virtual players, and each AI car
# carries a driving personality: its cornering commitment (corner_throttle) sets
# how hard it presses the in-flight feel levers, so a keen driver claws a deficit
# back harder, sheds a leader's burden lighter, and digs into a flag-rally,
# escort, chase- and escort-resolve more fiercely than a disciplined one. Every
# such lever is a public *_speed_multiplier taking a final corner_throttle, and
# the field's virtual_player_drive_system feeds each the car's own commitment.
#
# The human has no driving personality. It is meant to drive exactly as the
# neutral all-rounder does, which it gets by passing MIN_THROTTLE (the baseline
# the commitment scales are centred on, where each scales by exactly 1.0) to
# every commitment-scaled lever in car_movement_system. Every player_* helper in
# the human's movement file says so in as many words ("the human has no driving
# personality, so it ... on the neutral MIN_THROTTLE commitment ... keeping its
# urge at the unscaled baseline").
#
# That baseline is hand-maintained at each call site. Nothing stops a future
# change from wiring a new commitment-scaled lever into the human and passing the
# car's throttle (copy-pasted from the field's call), a stray literal, or any
# value but MIN_THROTTLE: the human silently grows a personality and stops
# driving at the all-rounder baseline, a fairness drift no compiler catches (a
# throttle is a valid f32 argument either way) and no unit test catches (each
# lever's suite tests the lever, not the human's call site). The asymmetry only
# ever surfaces as a feel the players can never quite name.
#
# This guard closes that gap. It discovers every commitment-scaled feel lever
# (each public *_speed_multiplier defined outside the two movement systems whose
# signature takes a corner_throttle) and asserts the human applies each one with
# MIN_THROTTLE, so a lever fed any other commitment on the human's side fails the
# build instead of shipping a human that no longer drives at the baseline.

human="src/gameplay/player/movement.rs"
ai="src/gameplay/virtual_player/drive.rs"
baseline="MIN_THROTTLE"

for required in "${human}" "${ai}"; do
  if [[ ! -f "${required}" ]]; then
    cat >&2 <<ERROR
Missing ${required}: the human-baseline commitment cannot be verified without it.
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
    ":(exclude)${human}" \
    ":(exclude)${ai}" |
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
# across drivers and so has no baseline to hold the human to.
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

This guard holds the human to the neutral baseline on exactly those levers.
Finding none means the corner_throttle commitment axis was renamed or removed and
the guard has gone blind: update it to match rather than letting it pass
vacuously.
ERROR
  exit 1
fi

# Every call to a lever in the human's movement file, returned one call per line
# with its multi-line arguments joined, so the baseline check below reads the
# whole argument list of each call. Full-line comments are dropped first so a
# lever named in a doc comment can neither be mistaken for a call nor satisfy
# one; a call is recognised by the `name(` token and captured by paren depth to
# its matching close.
calls_to() {
  local name="$1"
  grep -vE '^[[:space:]]*//' "${human}" |
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
not_baseline=()
for lever in "${commitment_levers[@]}"; do
  readarray -t calls < <(calls_to "${lever}")
  if ((${#calls[@]} == 0)); then
    missing_call+=("${lever}")
    continue
  fi
  for call in "${calls[@]}"; do
    if ! grep -qw "${baseline}" <<<"${call}"; then
      not_baseline+=("${lever}")
      break
    fi
  done
done

status=0

if ((${#missing_call[@]} > 0)); then
  cat >&2 <<ERROR
Commitment-scaled lever(s) the human never applies (${human}):

$(printf '  %s\n' "${missing_call[@]}")

The human must drive under the same feel levers as the field. Apply each lever
above in car_movement_system, passing ${baseline} so the human drives at the
neutral all-rounder baseline. (A lever genuinely missing from the human is also a
speed-modifier parity break; see scripts/check_speed_modifier_parity.sh.)
ERROR
  status=1
fi

if ((${#not_baseline[@]} > 0)); then
  cat >&2 <<ERROR
Commitment-scaled lever(s) the human applies off the neutral baseline (${human}):

$(printf '  %s\n' "${not_baseline[@]}")

The human has no driving personality: it must pass ${baseline} to every
commitment-scaled lever so it drives exactly as the neutral all-rounder does. A
call passing the car's throttle or any other value silently gives the human a
personality and breaks the fairness baseline. Pass ${baseline} as the
corner_throttle argument of each call above.
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

echo "Checked ${#commitment_levers[@]} commitment-scaled levers are applied by the human at the ${baseline} baseline."
