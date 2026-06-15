#!/usr/bin/env bash
set -euo pipefail

readarray -d '' shell_scripts < <(git ls-files -z '*.sh')

if ((${#shell_scripts[@]} == 0)); then
  echo "No shell scripts found."
  exit 0
fi

if ! command -v shellcheck >/dev/null 2>&1; then
  cat >&2 <<'ERROR'
shellcheck is required to lint shell scripts but was not found on PATH.
Install it (e.g. mise use -g shellcheck, or apt-get install shellcheck) and retry.
ERROR
  exit 1
fi

if ! command -v shfmt >/dev/null 2>&1; then
  cat >&2 <<'ERROR'
shfmt is required to check shell-script formatting but was not found on PATH.
Install it (e.g. mise use -g shfmt, or download a pinned release binary) and retry.
ERROR
  exit 1
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

# Static analysis: catch quoting, word-splitting, masked return codes, and other
# latent shell bugs that the bash -n parse and the convention greps above cannot
# see. Run at shellcheck's default (strictest) severity so even info- and
# style-level findings gate; the tree is clean at this level.
shellcheck -- "${shell_scripts[@]}"

# Formatting: enforce one canonical shell style across every tracked script, the
# shell-side mirror of `cargo fmt --all -- --check` for Rust. The static analysis
# above catches correctness bugs; shfmt catches drift in indentation,
# pipe-continuation and spacing so a review never argues style. The 2-space indent
# (-i 2) is pinned on the command line, not left to shfmt's tab default or an
# ambient .editorconfig, so the result is identical on every machine and in CI
# (which has no .editorconfig to read). The diff is printed so a failing run shows
# exactly what to fix.
if ! shfmt -i 2 -d -- "${shell_scripts[@]}"; then
  cat >&2 <<'ERROR'
Shell scripts are not shfmt-formatted (see the diff above).
Run: shfmt -i 2 -w $(git ls-files '*.sh')
ERROR
  exit 1
fi

echo "Checked ${#shell_scripts[@]} shell scripts."
