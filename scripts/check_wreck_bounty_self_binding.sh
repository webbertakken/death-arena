#!/usr/bin/env bash
set -euo pipefail

# Every wreck-bounty breakdown field must bind to its OWN local (self-binding).
#
# resolve_wreck_bounties in src/gameplay/combat/economy.rs prices each per-bonus
# *_wreck_bonus into a player_<base>/opponent_<base> local, folds both into the
# per-team payout totals (guarded by check_wreck_bounty_fold.sh) and records both
# in the WreckBounties breakdown fields it returns (guarded by
# check_wreck_bounty_breakdown.sh). Those two siblings pin a bonus's two
# destinations: that it is PAID and that it is LOGGED. Neither, though, pins that
# a breakdown field carries its OWN local rather than another bonus's.
#
# That is the last silent-attribution gap. A copy-paste slip such as
# `player_shutdown: player_most_wanted` sets the player_shutdown field from the
# wrong local: the breakdown presence check still passes (the field name is at the
# head of its line), the fold check still passes (the local is summed under its
# own name elsewhere), and the local stays used, so the build is clean and other
# bonuses' unit tests are unaffected. Yet the wreck log now attributes the
# shutdown cash to the leader bonus, double-reporting one source and silencing
# another while the totals still add up.
#
# Rust makes the correct wiring trivial: the field-init shorthand `player_<base>,`
# binds the field to the identically-named local by construction, and clippy's
# redundant_field_names (denied under the gate's -D warnings) forbids the only
# other self-binding form, the explicit `player_<base>: player_<base>`. So the one
# colon form that can still compile is `player_<base>: <some other local>`, i.e.
# exactly the mis-wire. This guard therefore asserts every breakdown field is the
# bare shorthand, which is self-binding by construction and the cleanest provable
# invariant: a colon-bound breakdown field fails the build instead of shipping a
# wreck log that credits the wrong bonus.
#
# The rampage streak payout rides the `streaks` field and
# payout.{player,opponent}_bounty, not a player_<base>/opponent_<base> breakdown
# pair, so it sits outside this set; its own resolve_wreck_streaks proves that
# arithmetic.

economy="src/gameplay/combat/economy.rs"
fold_fn="resolve_wreck_bounties"

if [[ ! -f "${economy}" ]]; then
  cat >&2 <<ERROR
Missing ${economy}: the wreck-bounty self-binding cannot be verified without it.
ERROR
  exit 1
fi

# The per-bonus rewards: every pub const fn named <base>_wreck_bonus. Each must be
# bound to its own breakdown field by the returned struct below.
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
# prose comment never counts. Captured from the fn signature to its closing brace
# at column 0 (the returned struct literal closes at an indented brace, so it does
# not end the capture early).
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

mis_wired=()
for base in "${bases[@]}"; do
  # A self-binding breakdown field is the bare shorthand on its own line:
  # `<side>_<base>,` and nothing else (the `let` binding carries ` = ...`, the
  # payout fold rides behind a `+ `, so only the breakdown initializer is a line of
  # just the name and a comma). Its absence means the field was colon-bound to a
  # different local, the one mis-wire clippy's redundant_field_names cannot catch.
  for side in player opponent; do
    if ! grep -Eq "^[[:space:]]*${side}_${base},[[:space:]]*$" <<<"${body}"; then
      mis_wired+=("${side}_${base}")
    fi
  done
done

if ((${#mis_wired[@]} > 0)); then
  cat >&2 <<ERROR
Wreck bonus breakdown field(s) not self-bound to their own local (${economy}):

$(printf '  %s\n' "${mis_wired[@]}")

${fold_fn} must record each bonus in the matching breakdown field using the bare
field-init shorthand (\`player_<base>,\`), which binds the field to its own local
by construction. Each name above is missing that shorthand, so it is colon-bound
to a different local and the wreck log credits the wrong bonus. Restore the
shorthand for each field above in ${fold_fn}.
ERROR
  exit 1
fi

echo "Checked ${#bases[@]} wreck bonuses are self-bound in both team breakdowns."
