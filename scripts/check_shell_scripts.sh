#!/usr/bin/env bash
set -euo pipefail

readarray -d '' shell_scripts < <(git ls-files -z '*.sh')

if ((${#shell_scripts[@]} == 0)); then
  echo "No shell scripts found."
  exit 0
fi

for script in "${shell_scripts[@]}"; do
  bash -n "${script}"
done

echo "Checked ${#shell_scripts[@]} shell scripts."
