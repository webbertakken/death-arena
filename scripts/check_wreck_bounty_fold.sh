#!/usr/bin/env bash
set -euo pipefail

# Every wreck-bounty must be folded into BOTH teams' running totals.
#
# A frame's wrecks pay a stack of cash rewards, and each is priced by its own
# pure const fn in src/gameplay/combat/economy.rs: the most-wanted leader bonus,
# the carrier-takedown bonus, the shutdown bonus, first blood, payback, the
# clutch bonus. resolve_wreck_bounties calls each, binds the result to a
# player_<base> and an opponent_<base> local, then must add BOTH into the
# per-team `player:`/`opponent:` totals it returns. That stack has grown one
# bounty at a time, and every addition has to touch the sum in two places.
#
# Forgetting one is a silent payout bug the compiler cannot see. A bound local
# that is dropped from the sum is not unused: it still feeds the WreckBounties
# breakdown field kept for the wreck log, so the function compiles clean with no
# warning at all. No unit test catches it either, because every other bonus's
# suite still passes; the dropped reward simply never reaches a team's wallet,
# and the only symptom is cash that quietly goes unpaid. It is the same
# compiler-invisible fold gap the speed- and effect-multiplier parity guards were
# built to stop, here on the economy side.
#
# This guard closes that gap. It discovers every per-bonus *_wreck_bonus function
# and asserts each is summed into both the player and the opponent total in
# resolve_wreck_bounties, so a bonus wired into the breakdown but missing from a
# team's payout fails the build instead of shipping a reward that never pays.
#
# The rampage streak payout folds in through payout.{player,opponent}_bounty
# rather than a *_wreck_bonus call, so it sits outside this set; its own
# resolve_wreck_streaks already proves that arithmetic.

economy="src/gameplay/combat/economy.rs"
fold_fn="resolve_wreck_bounties"

if [[ ! -f "${economy}" ]]; then
  cat >&2 <<ERROR
Missing ${economy}: the wreck-bounty fold cannot be verified without it.
ERROR
  exit 1
fi

# The per-bonus rewards: every pub const fn named <base>_wreck_bonus. Each must be
# folded into both team totals by the fold below.
readarray -t bases < <(
  grep -oE 'pub const fn [a-z_]+_wreck_bonus' "${economy}" |
    sed -E 's/^pub const fn //; s/_wreck_bonus$//' |
    sort -u
)

if ((${#bases[@]} == 0)); then
  cat >&2 <<ERROR
No *_wreck_bonus functions found in ${economy}.

This guard derives its work from the per-bonus wreck rewards (most_wanted_wreck_bonus,
carrier_takedown_wreck_bonus, and the rest). Finding none means the naming
convention changed and the guard has gone blind: update it to match the new
convention rather than letting it pass vacuously.
ERROR
  exit 1
fi

# The body of resolve_wreck_bounties, comments stripped, so a bonus named only in
# a prose comment never counts as folded. Captured from the fn signature to its
# closing brace at column 0 (the returned struct literal closes at an indented
# brace, so it does not end the capture early).
body="$(
  awk -v marker="fn ${fold_fn}(" '
    index($0, marker) { capturing = 1 }
    capturing {
      print
      if ($0 ~ /^}/) { exit }
    }
  ' "${economy}" | grep -vE '^[[:space:]]*//' || true
)"

if [[ -z "${body}" ]]; then
  cat >&2 <<ERROR
Could not find ${fold_fn} in ${economy}.

This guard reads the per-team payout sum from ${fold_fn}. Finding nothing means
the fold was renamed or restructured and the guard has gone blind: update it to
match rather than letting it pass vacuously.
ERROR
  exit 1
fi

missing=()
for base in "${bases[@]}"; do
  if ! grep -Eq "[+] player_${base}\b" <<<"${body}"; then
    missing+=("player_${base}")
  fi
  if ! grep -Eq "[+] opponent_${base}\b" <<<"${body}"; then
    missing+=("opponent_${base}")
  fi
done

if ((${#missing[@]} > 0)); then
  cat >&2 <<ERROR
Wreck bonus(es) priced but never paid into a team total (${economy}):

$(printf '  %s\n' "${missing[@]}")

${fold_fn} prices each *_wreck_bonus into a per-team local but must also add it
into the matching player and opponent totals it returns. Each name above is bound
(it still feeds the WreckBounties breakdown for the wreck log) but missing from
its team's payout sum, so the reward is computed and then silently dropped. Add
each to the corresponding total in ${fold_fn}.
ERROR
  exit 1
fi

echo "Checked ${#bases[@]} wreck bonuses are folded into both team totals."
