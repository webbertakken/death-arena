#!/usr/bin/env bash
set -euo pipefail

# Every per-match resource must be wiped when a fresh match begins.
#
# A Death Arena round keeps its state in Bevy resources: the capture and wreck
# tallies, the nitro/shield/sabotage timers, vehicle integrity, the wreck streak
# and stun/surge pools, the flag timers and the match clock. Each is declared
# once with init_resource in its plugin's build, and each must be reset to its
# default the instant a new match starts, on SystemSet::on_enter(AppState::InGame),
# so a round always begins from a clean slate. The plugins do exactly that today:
# every init_resource in the three match plugins has a matching `*x = X::default()`
# reset in an on-enter system.
#
# That reset is hand-maintained, and nothing stops a future change from declaring
# a new per-match resource with init_resource and forgetting to wipe it on enter.
# The result is a silent state-leak bug of the same family the parity guards were
# built to stop, here across the round boundary: a wreck streak, a banked tally or
# a half-spent nitro timer survives into the next match, so the second round opens
# mid-fight on stale numbers the player can never account for. The compiler cannot
# see it (an un-reset resource is still a perfectly valid resource), and no unit
# test catches it (each system passes its own suite either way); the leak only
# ever surfaces as a fresh round that does not feel fresh.
#
# This guard closes that gap. It derives the set of resources each match plugin
# declares with init_resource and the set it wipes with a `*x = X::default()`
# reset, then asserts every declared resource is reset, so a resource added to a
# match plugin and never wiped on enter fails the build instead of leaking state
# into the next round. It scopes to the three match plugins on purpose: a
# frame-refreshed tracker elsewhere (the virtual-player PlayerVelocity, rewritten
# every fixed frame) is self-correcting and deliberately needs no per-match reset,
# so it is not a match plugin and not in scope here.

files=(
  "src/gameplay/pickup/mod.rs"
  "src/gameplay/ctf/mod.rs"
  "src/gameplay/combat/mod.rs"
)

for required in "${files[@]}"; do
  if [[ ! -f "${required}" ]]; then
    cat >&2 <<ERROR
Missing ${required}: the match-resource reset parity cannot be verified without it.
ERROR
    exit 1
  fi
done

# The resources each match plugin declares: every init_resource::<Type>() across
# the files.
declared="$(
  git grep -hoE 'init_resource::<[A-Za-z0-9_]+>' -- "${files[@]}" |
    sed -E 's/init_resource::<//; s/>//' |
    sort -u || true
)"

# The resources each match plugin wipes on a fresh match: every `*ident =
# Type::default();` reset assignment across the files. This deref-assign is the
# reset idiom the on-enter systems use, so the set of types it names is exactly
# the resources a new match clears.
reset="$(
  git grep -hE '^[[:space:]]*\*[a-z_]+ = [A-Za-z0-9_]+::default\(\);' -- "${files[@]}" |
    grep -oE '[A-Za-z0-9_]+::default' |
    sed -E 's/::default//' |
    sort -u || true
)"

if [[ -z "${declared}" ]]; then
  cat >&2 <<'ERROR'
No init_resource declarations found in the match plugins.

This guard derives its work from the init_resource::<Type>() calls in the three
match plugins. Finding none means a plugin was renamed or now registers its
resources differently and this guard has gone blind: update it to match the new
convention rather than letting it pass vacuously.
ERROR
  exit 1
fi

if [[ -z "${reset}" ]]; then
  cat >&2 <<'ERROR'
No `*x = X::default()` reset assignments found in the match plugins.

This guard derives its work from the deref-assign resets in the on-enter systems.
Finding none means the reset idiom changed and this guard has gone blind: update
it to match the new convention rather than letting it pass vacuously.
ERROR
  exit 1
fi

readarray -t unreset < <(comm -23 <(echo "${declared}") <(echo "${reset}"))

if ((${#unreset[@]} > 0)); then
  cat >&2 <<ERROR
Per-match resource(s) declared with init_resource but never reset on a fresh match:

$(printf '  %s\n' "${unreset[@]}")

Each resource above is registered by a match plugin but no on-enter system wipes
it, so its state leaks from one round into the next. Reset each one to its default
in the plugin's SystemSet::on_enter(AppState::InGame) reset system:

  *resource = ResourceType::default();

If the resource is genuinely not per-match (a frame-refreshed tracker that needs
no reset), it does not belong in a match plugin; move it out so this guard stays
honest.
ERROR
  exit 1
fi

count="$(echo "${declared}" | wc -l | tr -d ' ')"
echo "Checked ${count} per-match resources are reset on a fresh match."
