#!/usr/bin/env bash
set -euo pipefail

# Every guard script must be wired into the local aggregator and a CI workflow.
#
# The project leans on a growing fleet of scripts/check_*.sh guards, and two
# hand-maintained lists decide whether each one actually runs: the local
# aggregator scripts/check_local_quality.sh (the single command run before a
# commit, the shift-left entry point) invokes them one by one, and the GitHub
# workflows run them as steps so a bypassed local hook cannot land a regression.
# Both lists are edited by hand, so a freshly added guard only runs once someone
# remembers to add it in both places. Forget the aggregator line and the guard
# never runs locally; forget the workflow step and it never blocks a pull
# request. Either way the guard sits in the tree doing nothing, a silent gap
# exactly where the project relies hardest on these checks. scripts/
# check_ci_workflow.sh and scripts/check_precommit_hook.sh police specific known
# commands, but neither notices a brand-new guard that was never added anywhere.
# This check closes that gap: it derives the guard set from the filesystem and
# asserts every one is both invoked by the aggregator and run by a workflow, so
# an orphaned guard fails the build instead of going unnoticed.
#
# The aggregator itself is excluded from both checks: it orchestrates the guards
# rather than being one, so it neither invokes itself nor runs as its own CI
# step. A dist-only guard is allowed to live in the Pages workflow instead of the
# CI workflow, since the release artefact it inspects is only built there.

aggregator="scripts/check_local_quality.sh"
workflows=(.github/workflows/ci.yml .github/workflows/pages.yml)

for required in "${aggregator}" "${workflows[@]}"; do
  if [[ ! -f "${required}" ]]; then
    cat >&2 <<ERROR
Missing ${required}: the quality-guard wiring cannot be verified without it.
ERROR
    exit 1
  fi
done

readarray -d '' guard_scripts < <(git ls-files -z 'scripts/check_*.sh')

if ((${#guard_scripts[@]} == 0)); then
  echo "No guard scripts found."
  exit 0
fi

not_in_aggregator=()
not_in_workflow=()
for script in "${guard_scripts[@]}"; do
  if [[ "${script}" == "${aggregator}" ]]; then
    continue
  fi

  if ! grep -Fq "${script}" "${aggregator}"; then
    not_in_aggregator+=("${script}")
  fi

  if ! grep -Fq "${script}" "${workflows[@]}"; then
    not_in_workflow+=("${script}")
  fi
done

status=0

if ((${#not_in_aggregator[@]} > 0)); then
  cat >&2 <<ERROR
Guard script(s) not invoked by ${aggregator}:

$(printf '  %s\n' "${not_in_aggregator[@]}")

The aggregator is the single command run before a commit; a guard missing from it
never runs locally, so its regressions only surface later in CI and the
shift-left feedback the project relies on is lost. Add a line invoking each
script above to ${aggregator}.
ERROR
  status=1
fi

if ((${#not_in_workflow[@]} > 0)); then
  cat >&2 <<ERROR
Guard script(s) not run by any GitHub workflow (${workflows[*]}):

$(printf '  %s\n' "${not_in_workflow[@]}")

A guard that never runs in CI cannot block a pull request, so a bypassed local
hook lets exactly the regression it guards reach main. Add a step running each
script above to .github/workflows/ci.yml (or the Pages workflow for a dist-only
guard).
ERROR
  status=1
fi

if ((status != 0)); then
  exit "${status}"
fi

echo "Checked ${#guard_scripts[@]} guard scripts are wired into ${aggregator} and CI."
