#!/usr/bin/env bash
set -euo pipefail

# The player's team and the field must bank the same rewards on the same terms.
#
# Death Arena keeps two running tallies of match earnings in
# src/gameplay/pickup/mod.rs: Score for the human player's team and OpponentScore
# for the virtual-player field. They are deliberately mirror images. Every reward
# a match can pay (a collected pickup, a flag capture, an enemy-flag steal, a home
# return, a wreck bounty, the end-of-match purse, the comeback capture bonus) is
# banked through a method that exists, with the same name, on BOTH structs, and
# both structs hold the same set of tally fields. The two are separate Bevy
# resources so a system can reward one team without touching the other, and that
# separation is exactly what leaves their parity hand-maintained.
#
# Nothing stops a future change from adding a reward method or a tally field to one
# struct and forgetting its twin, and the result is a silent fairness bug of the
# same family the speed-, effect- and wreck-bounty parity guards were built to
# stop, here on the scoring side: a reward the player can bank but the field never
# can (or the reverse), or a tally one team keeps and the other quietly drops. The
# compiler cannot see it (each struct compiles fine on its own) and no unit test
# catches it (each struct's own suite still passes); the asymmetry only ever
# surfaces as one team earning on terms the other does not.
#
# This guard closes that gap. It derives the tally-field set and the public
# reward-method set of each struct from the source and asserts the two structs
# expose the identical sets, so a reward or tally added to one and missed on the
# other fails the build instead of shipping an uneven economy.

source_file="src/gameplay/pickup/mod.rs"

if [[ ! -f "${source_file}" ]]; then
  cat >&2 <<ERROR
Missing ${source_file}: the score-tally parity cannot be verified without it.
ERROR
  exit 1
fi

# The body of a `struct <name> {` or `impl <name> {` block, comments stripped, from
# its opening marker to the first brace at column 0 that closes it. Comment lines
# are dropped so a name mentioned only in prose never counts as a real field or
# method.
block_body() {
  local marker="$1"
  awk -v marker="${marker}" '
    index($0, marker) { capturing = 1 }
    capturing {
      print
      if ($0 ~ /^}/) { exit }
    }
  ' "${source_file}" | grep -vE '^[[:space:]]*//' || true
}

# The tally fields of a struct: every `pub <name>:` declaration in its block.
fields_of() {
  block_body "pub struct $1 {" |
    grep -oE 'pub [a-z_]+:' |
    sed -E 's/^pub //; s/:$//' |
    sort -u
}

# The public reward methods of a struct: every `pub [const] fn <name>` in its impl.
methods_of() {
  block_body "impl $1 {" |
    grep -oE 'pub (const )?fn [a-z_]+' |
    sed -E 's/^pub (const )?fn //' |
    sort -u
}

player_fields="$(fields_of Score)"
opponent_fields="$(fields_of OpponentScore)"
player_methods="$(methods_of Score)"
opponent_methods="$(methods_of OpponentScore)"

if [[ -z "${player_fields}" || -z "${opponent_fields}" ]]; then
  cat >&2 <<'ERROR'
No tally fields found on one of Score / OpponentScore.

This guard derives its work from the `pub <name>:` fields of the two score structs
in src/gameplay/pickup/mod.rs. Finding none means a struct was renamed or
restructured and the guard has gone blind: update it to match the new shape rather
than letting it pass vacuously.
ERROR
  exit 1
fi

if [[ -z "${player_methods}" || -z "${opponent_methods}" ]]; then
  cat >&2 <<'ERROR'
No public reward methods found on one of Score / OpponentScore.

This guard derives its work from the `pub fn` reward methods in the two score
structs' impl blocks. Finding none means an impl was renamed or restructured and
the guard has gone blind: update it to match the new shape rather than letting it
pass vacuously.
ERROR
  exit 1
fi

readarray -t fields_only_player < <(comm -23 <(echo "${player_fields}") <(echo "${opponent_fields}"))
readarray -t fields_only_opponent < <(comm -13 <(echo "${player_fields}") <(echo "${opponent_fields}"))
readarray -t methods_only_player < <(comm -23 <(echo "${player_methods}") <(echo "${opponent_methods}"))
readarray -t methods_only_opponent < <(comm -13 <(echo "${player_methods}") <(echo "${opponent_methods}"))

status=0

if ((${#fields_only_player[@]} > 0)); then
  cat >&2 <<ERROR
Tally field(s) on Score that OpponentScore is missing:

$(printf '  %s\n' "${fields_only_player[@]}")

The two score tallies must mirror each other so the player and the field earn on
identical terms. Add each field above to OpponentScore.
ERROR
  status=1
fi

if ((${#fields_only_opponent[@]} > 0)); then
  cat >&2 <<ERROR
Tally field(s) on OpponentScore that Score is missing:

$(printf '  %s\n' "${fields_only_opponent[@]}")

The two score tallies must mirror each other so the player and the field earn on
identical terms. Add each field above to Score.
ERROR
  status=1
fi

if ((${#methods_only_player[@]} > 0)); then
  cat >&2 <<ERROR
Reward method(s) on Score that OpponentScore is missing:

$(printf '  %s\n' "${methods_only_player[@]}")

The two score tallies must mirror each other so the player and the field bank the
same rewards. Add each method above to OpponentScore.
ERROR
  status=1
fi

if ((${#methods_only_opponent[@]} > 0)); then
  cat >&2 <<ERROR
Reward method(s) on OpponentScore that Score is missing:

$(printf '  %s\n' "${methods_only_opponent[@]}")

The two score tallies must mirror each other so the player and the field bank the
same rewards. Add each method above to Score.
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

field_count="$(echo "${player_fields}" | wc -l | tr -d ' ')"
method_count="$(echo "${player_methods}" | wc -l | tr -d ' ')"
echo "Checked Score and OpponentScore mirror ${field_count} tally fields and ${method_count} reward methods."
