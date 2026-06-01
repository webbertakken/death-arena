#!/usr/bin/env bash
set -euo pipefail

readarray -d '' shell_scripts < <(git ls-files -z '*.sh')

if ((${#shell_scripts[@]} == 0)); then
  echo "No shell scripts found."
  exit 0
fi

for script in "${shell_scripts[@]}"; do
  bash -n "${script}"

  if ! head -n 1 "${script}" | grep -Eq '^#!(/usr/bin/env bash|/bin/bash)$'; then
    cat >&2 <<ERROR
${script} must use a bash shebang.
Use: #!/usr/bin/env bash
ERROR
    exit 1
  fi

  if ! sed -n '1,10p' "${script}" | grep -Eq '^set -euo pipefail$'; then
    cat >&2 <<ERROR
${script} must enable bash strict mode near the top.
Use: set -euo pipefail
ERROR
    exit 1
  fi
done

echo "Checked ${#shell_scripts[@]} shell scripts."
