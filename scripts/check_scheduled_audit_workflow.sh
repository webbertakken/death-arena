#!/usr/bin/env bash
set -euo pipefail

# Validate the standalone scheduled security-advisory workflow.
#
# The push/pull-request `audit` job in ci.yml only scans Cargo.lock when the code
# changes, yet RustSec advisories are disclosed continuously against crates that
# are already pinned. On a repository that can sit idle for weeks, a brand-new
# vulnerability against an unchanged lockfile would otherwise stay invisible until
# the next push. This guard keeps a dedicated cron-driven workflow in place so the
# same advisory scan runs on a timer, closing that between-pushes gap.

workflow=".github/workflows/scheduled-audit.yml"

if [[ ! -f "${workflow}" ]]; then
  cat >&2 <<ERROR
Missing scheduled security-advisory workflow: ${workflow}
A cron-driven audit must exist so a vulnerability disclosed against an unchanged
Cargo.lock is caught between pushes, not just when the code happens to change.
ERROR
  exit 1
fi

if grep -Fq "git reset --hard" "${workflow}"; then
  cat >&2 <<'ERROR'
Scheduled audit workflow must not use git reset --hard.
Checkout should leave the workspace ready without destructive cleanup.
ERROR
  exit 1
fi

if ! grep -Fq "schedule:" "${workflow}"; then
  cat >&2 <<'ERROR'
Scheduled audit workflow must run on a schedule, so the advisory scan fires even
when no push happens.
Add a schedule trigger with a cron entry.
ERROR
  exit 1
fi

if ! grep -Fq "cron:" "${workflow}"; then
  cat >&2 <<'ERROR'
Scheduled audit workflow schedule must declare a cron expression.
Add: schedule: [ { cron: "<expression>" } ]
ERROR
  exit 1
fi

if ! grep -Fq "workflow_dispatch:" "${workflow}"; then
  cat >&2 <<'ERROR'
Scheduled audit workflow must also allow manual runs, so the scan can be kicked
off on demand and survives GitHub disabling idle cron schedules.
Add a workflow_dispatch trigger.
ERROR
  exit 1
fi

if ! grep -Fq "bash scripts/check_security_advisories.sh" "${workflow}"; then
  cat >&2 <<'ERROR'
Scheduled audit workflow must run the shared advisory scan, so the timed run and
the push/pull-request audit stay a single source of truth.
Add a step that runs: bash scripts/check_security_advisories.sh
ERROR
  exit 1
fi

echo "Validated scheduled security-advisory workflow."
