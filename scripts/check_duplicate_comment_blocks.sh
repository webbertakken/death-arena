#!/usr/bin/env bash
set -euo pipefail

# A comment block must never be duplicated back-to-back in the crate source.
#
# This codebase treats its prose as first-class: clippy, rustfmt and the
# rustdoc gate (scripts/check_rust_docs.sh, run with -D warnings) all police the
# code and the doc links, but none of them notices when a stretch of comment
# text is accidentally pasted twice in a row. A doubled paragraph is a real
# defect: it bloats the source, and a later edit to one copy and not the other
# leaves the two silently contradicting each other, exactly the kind of stale,
# misleading comment the heavy documentation culture here is meant to avoid.
# Such a duplication slips past every existing gate; this guard closes that gap,
# failing the build the moment a comment block is repeated immediately after
# itself.
#
# Only the crate source under src/ is scanned, matching the other content
# guards. A block is flagged only when a run of consecutive comment lines is
# immediately followed by an identical run carrying real prose (more than a
# handful of characters once slashes and whitespace are stripped), so a pair of
# bare `//` separators or a short repeated marker never trips the gate; only
# genuinely duplicated wording does. None appears anywhere in the source today;
# this keeps it that way.

readarray -d '' rust_sources < <(git ls-files -z 'src/*.rs')

if ((${#rust_sources[@]} == 0)); then
  echo "No Rust source files found."
  exit 0
fi

# For each file, scan for a contiguous comment block (window size 2..=8 lines)
# that is immediately repeated. A line counts as a comment when its first
# non-blank characters are `//` (so `//` and `///` both qualify); the repeated
# halves must be byte-identical and together carry more than 30 characters of
# real text once the leading slashes and all whitespace are stripped.
findings=""
for src in "${rust_sources[@]}"; do
  match="$(
    awk '
      { line[NR] = $0 }
      END {
        for (k = 2; k <= 8; k++) {
          for (i = 1; i + 2 * k - 1 <= NR; i++) {
            ok = 1
            content = ""
            for (j = 0; j < k; j++) {
              first = line[i + j]
              second = line[i + k + j]
              trimmed = first
              sub(/^[ \t]+/, "", trimmed)
              if (trimmed !~ /^\/\//) { ok = 0; break }
              if (first != second) { ok = 0; break }
              text = trimmed
              sub(/^\/+[ \t]*/, "", text)
              gsub(/[ \t]/, "", text)
              content = content text
            }
            if (ok && length(content) > 30) {
              print FILENAME ":" i "-" (i + 2 * k - 1)
              i = i + 2 * k - 1
            }
          }
        }
      }
    ' "${src}"
  )"
  if [[ -n "${match}" ]]; then
    findings+="${match}"$'\n'
  fi
done

if [[ -n "${findings}" ]]; then
  cat >&2 <<ERROR
Duplicated comment blocks are not allowed in the crate source.

A stretch of comment text is repeated immediately after itself. Delete the
duplicate copy: a doubled paragraph bloats the source and rots into two copies
that silently disagree the moment one is edited and the other is not.

${findings}
ERROR
  exit 1
fi

echo "Checked ${#rust_sources[@]} Rust source file(s) for duplicated comment blocks."
