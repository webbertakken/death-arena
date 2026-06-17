#!/usr/bin/env bash
set -euo pipefail

# The human and the field must reap the same effect from every pickup.
#
# Death Arena hands the same collectibles to both sides: the human's team (blue)
# and the virtual field (red) drive over the same nitro, shield and sabotage
# charges, and the promise that keeps the contest fair is that a given pickup
# arms the same effect whichever side grabs it. That mapping lives in one place,
# collect_for_team in src/gameplay/pickup/system.rs, as a match on the team with a
# blue arm and a red arm. The two arms cannot share a single body because each
# effect is armed on a different pool per side (the blue grab calls
# nitro_boosts.trigger_player(), the red grab nitro_boosts.trigger_opponent()),
# so the same pickup kind is wired twice, once per arm.
#
# That doubling is hand-maintained, and nothing stops a future pickup effect from
# being wired into one arm and forgotten in the other: a new charge that boosts
# the human but does nothing for the field, or bogs the field but spares the
# human. The result is the same silent fairness bug the speed- and
# effect-multiplier parity guards were built to prevent, one arena over. No unit
# test catches the omission either: a branch that was never written has no line
# to leave uncovered, so the coverage gate stays green while the field quietly
# plays by different rules.
#
# This guard closes that gap. It reads the set of PickupKind variants each arm of
# collect_for_team responds to and asserts the two arms cover an identical set,
# so a pickup effect wired into one side and missed on the other fails the build
# instead of shipping an unfair match. It keys on the pickup kind, not the
# per-side method name (which differs by design), so the differing trigger_player
# and trigger_opponent calls never trip it; only a genuinely one-sided pickup
# does.

target="${1:-src/gameplay/pickup/system.rs}"

if [[ ! -f "${target}" ]]; then
  cat >&2 <<ERROR
Missing ${target}: the pickup-effect parity cannot be verified without it.
ERROR
  exit 1
fi

# The collect_for_team match block, captured whole by brace count from the
# "match team {" line to its matching close, so the arm split below sees exactly
# the two team arms and nothing after the match (the repair handling that follows
# is symmetric by construction, a single integrity.repair(team) call).
match_block="$(
  awk '
    /fn collect_for_team\(/ { in_fn = 1 }
    in_fn && /match team \{/ { in_match = 1 }
    in_match {
      print
      opens = gsub(/\{/, "{")
      closes = gsub(/\}/, "}")
      depth += opens - closes
      if (seen_open && depth == 0) { exit }
      if (opens > 0) { seen_open = 1 }
    }
  ' "${target}"
)"

if [[ -z "${match_block}" ]]; then
  cat >&2 <<ERROR
No collect_for_team team match block found in ${target}.

This guard derives its work from that match, the single place a pickup is mapped
to its per-team effect. Finding none means the function was renamed or the match
restructured and this guard has gone blind: update it to match rather than
letting it pass vacuously.
ERROR
  exit 1
fi

# The blue (human) arm is everything before the red arm marker; the red (field)
# arm is everything from the marker to the match close. Each arm's effect set is
# the PickupKind variants it responds to.
blue_kinds="$(awk '/AiTeam::Red =>/ { exit } { print }' <<<"${match_block}" |
  grep -oE 'PickupKind::[A-Za-z0-9_]+' | sort -u || true)"
red_kinds="$(awk 'found { print } /AiTeam::Red =>/ { found = 1 }' <<<"${match_block}" |
  grep -oE 'PickupKind::[A-Za-z0-9_]+' | sort -u || true)"

if [[ -z "${blue_kinds}" || -z "${red_kinds}" ]]; then
  cat >&2 <<ERROR
No PickupKind effects found in one of collect_for_team's two team arms (${target}).

The guard derives its work from the PickupKind variants each arm of the team
match responds to. Finding none in an arm means the arm markers (AiTeam::Blue and
AiTeam::Red) or the PickupKind references changed and this guard has gone blind:
update it to match rather than letting it pass vacuously.
ERROR
  exit 1
fi

readarray -t missing_in_red < <(comm -23 <(echo "${blue_kinds}") <(echo "${red_kinds}"))
readarray -t missing_in_blue < <(comm -13 <(echo "${blue_kinds}") <(echo "${red_kinds}"))

status=0

if ((${#missing_in_red[@]} > 0)); then
  cat >&2 <<ERROR
Pickup effect(s) the human team reaps but the field does not (${target}):

$(printf '  %s\n' "${missing_in_red[@]}")

The field must reap the same effect from every pickup as the human. Wire each
kind above into the AiTeam::Red arm of collect_for_team.
ERROR
  status=1
fi

if ((${#missing_in_blue[@]} > 0)); then
  cat >&2 <<ERROR
Pickup effect(s) the field reaps but the human team does not (${target}):

$(printf '  %s\n' "${missing_in_blue[@]}")

The human must reap the same effect from every pickup as the field. Wire each
kind above into the AiTeam::Blue arm of collect_for_team.
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

kind_count="$(echo "${blue_kinds}" | wc -l | tr -d ' ')"
echo "Checked ${kind_count} pickup effects are reaped by both the human and the field."
