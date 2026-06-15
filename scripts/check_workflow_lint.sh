#!/usr/bin/env bash
set -euo pipefail

# Static analysis for GitHub Actions workflows.
#
# The per-workflow content guards (check_ci_workflow.sh, check_pages_workflow.sh,
# check_scheduled_audit_workflow.sh) only assert that each workflow still wires in
# the required steps; none validate the workflow definitions themselves. A typo'd
# `${{ }}` expression, a deprecated action input, an undefined `needs:` reference,
# or a shell bug inside a `run:` step all parse as valid YAML yet break the
# pipeline only once it runs on a push. actionlint catches these statically, and
# (because shellcheck sits on PATH alongside it) lints the bash embedded in every
# `run:` step too, so the same shell rules the standalone scripts are held to also
# cover the inline workflow steps.

readarray -d '' workflows < <(
  git ls-files -z '.github/workflows/*.yml' '.github/workflows/*.yaml'
)

if ((${#workflows[@]} == 0)); then
  echo "No GitHub Actions workflows found."
  exit 0
fi

if ! command -v actionlint >/dev/null 2>&1; then
  cat >&2 <<'ERROR'
actionlint is required to lint GitHub Actions workflows but was not found on PATH.
Install it (e.g. mise use -g actionlint, or download a release binary from
https://github.com/rhysd/actionlint/releases) and retry.
ERROR
  exit 1
fi

# Run at actionlint's default severity so every finding gates. shellcheck on PATH
# is picked up automatically to lint the shell inside each `run:` step.
actionlint -- "${workflows[@]}"

echo "Checked ${#workflows[@]} GitHub Actions workflows."
