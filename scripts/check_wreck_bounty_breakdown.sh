#!/usr/bin/env bash
set -euo pipefail

# Every wreck-bounty must be recorded in BOTH teams' breakdown fields for the log.
#
# A frame's wrecks pay a stack of cash rewards, each priced by its own pure const
# fn in src/gameplay/combat/economy.rs: the most-wanted leader bonus, the
# carrier-takedown bonus, the shutdown bonus, first blood, payback, the clutch
# bonus. resolve_wreck_bounties binds each to a player_<base>/opponent_<base>
# local, folds both into the per-team payout totals (guarded by
# check_wreck_bounty_fold.sh) AND records both in the WreckBounties breakdown
# fields it returns, the per-bonus attribution the wreck log reads to say which
# reward paid what. That breakdown has grown one field-pair at a time, in lockstep
# with the bounties.
#
# Dropping a breakdown field is a silent logging bug the compiler need not see. So
# long as the struct lists the field by name the build stays clean, but the moment
# a future refactor reaches for `..WreckBounties::default()` (the struct derives
# Default) or mistypes `player_clutch: 0`, the field is set yet no longer carries
# its bonus: the reward still pays into the total (so the local is used, no
# unused-variable warning), and every unit test on the other bonuses still passes,
# yet the wreck log mis-attributes the payout, reporting cash it cannot account
# for. It is the logging-side mirror of the payout fold gap
# check_wreck_bounty_fold.sh stops, and the fold guard leans on this very wiring (a
# local dropped from the sum stays "used" only because it still feeds the
# breakdown), so the two guards together pin each bonus's two destinations.
#
# This guard closes the logging gap. It discovers every per-bonus *_wreck_bonus
# function and asserts each is recorded in both the player and the opponent
# breakdown field returned by resolve_wreck_bounties, so a bonus folded into a team
# total but dropped from its breakdown fails the build instead of shipping a wreck
# log that cannot account for the cash it paid.
#
# The rampage streak payout rides payout.{player,opponent}_bounty into the totals
# and the `streaks` field, not a player_<base>/opponent_<base> breakdown pair, so
# it sits outside this set; its own resolve_wreck_streaks proves that arithmetic.

economy="src/gameplay/combat/economy.rs"
fold_fn="resolve_wreck_bounties"

if [[ ! -f "${economy}" ]]; then
  cat >&2 <<ERROR
Missing ${economy}: the wreck-bounty breakdown cannot be verified without it.
ERROR
  exit 1
fi

# The per-bonus rewards: every pub const fn named <base>_wreck_bonus. Each must be
# recorded in both team breakdown fields by the returned struct below.
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

# The body of resolve_wreck_bounties, comments stripped, so a field named only in a
# prose comment never counts as recorded. Captured from the fn signature to its
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

This guard reads the per-team breakdown fields from ${fold_fn}. Finding nothing
means the fold was renamed or restructured and the guard has gone blind: update it
to match rather than letting it pass vacuously.
ERROR
  exit 1
fi

missing=()
for base in "${bases[@]}"; do
  # A breakdown field-initializer sits at the head of its line in the returned
  # struct literal (`player_<base>,` shorthand or `player_<base>: <expr>`), unlike
  # the payout fold where the same name rides behind a `+`. Anchoring at the line
  # start keeps the two destinations distinct, so a name that survives only in the
  # `+` sum still counts as missing from the breakdown.
  if ! grep -Eq "^[[:space:]]*player_${base}[,:]" <<<"${body}"; then
    missing+=("player_${base}")
  fi
  if ! grep -Eq "^[[:space:]]*opponent_${base}[,:]" <<<"${body}"; then
    missing+=("opponent_${base}")
  fi
done

if ((${#missing[@]} > 0)); then
  cat >&2 <<ERROR
Wreck bonus(es) priced but never recorded in a team breakdown (${economy}):

$(printf '  %s\n' "${missing[@]}")

${fold_fn} prices each *_wreck_bonus into a per-team local but must also record it
in the matching player and opponent breakdown field of the WreckBounties it
returns. Each name above is bound and folded into its team total but missing from
its breakdown, so the wreck log can no longer attribute the cash it paid. Add each
to the returned struct in ${fold_fn}.
ERROR
  exit 1
fi

echo "Checked ${#bases[@]} wreck bonuses are recorded in both team breakdowns."
