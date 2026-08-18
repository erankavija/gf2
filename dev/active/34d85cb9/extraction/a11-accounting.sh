#!/usr/bin/env bash
# Re-derive excerpts/A11-accounting.txt: every Aeneas [Error] class in the A11
# (pinned leg) run with its occurrence count, and the sorry tally of the Lean it
# produced, kept separate because the two do not map onto each other 1:1.
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
LOG="$HERE/../logs-trimmed/A11.log"
FUNS="$HERE/A11_lean/Funs.lean"
OUT="$HERE/excerpts/A11-accounting.txt"
mkdir -p "$HERE/excerpts"

{
  echo "# A11 (pinned leg) error/sorry accounting."
  echo "# Sources: dev/active/34d85cb9/logs-trimmed/A11.log"
  echo "#          dev/active/34d85cb9/extraction/A11_lean/Funs.lean"
  echo "# Reproduce: dev/active/34d85cb9/extraction/a11-accounting.sh"
  echo
  echo "## [Error] lines in the log, by class"
  grep -oE "\[Error\] .*" "$LOG" | sed 's/[[:space:]]*$//' | sort | uniq -c | sort -rn | sed 's/^/  /'
  echo "  ---"
  printf "  %6d TOTAL [Error] lines\n" "$(grep -c '\[Error\]' "$LOG")"
  echo
  echo "## sorry occurrences in the generated Funs.lean"
  printf "  %6d carrying a 'Could not find: type_var_id' comment\n" \
    "$(grep -c 'Could not find: type_var_id' "$FUNS")"
  printf "  %6d plain (no attributing comment)\n" \
    "$(grep 'sorry' "$FUNS" | grep -vc 'Could not find: type_var_id')"
  echo "  ---"
  printf "  %6d TOTAL sorry occurrences\n" "$(grep -c sorry "$FUNS")"
  echo
  echo "## Mapping"
  echo "  The 40 type_var_id sorrys map 1:1 onto the 40 type_var_id [Error] lines"
  echo "  (each sorry embeds the error text verbatim)."
  echo "  The remaining 13 plain sorrys do NOT map 1:1 onto the remaining 18"
  echo "  [Error] lines, and the log does not carry enough information to derive"
  echo "  such a mapping: two of those errors are explicitly cascades ('probably"
  echo "  because of an error which happened before'), and a single failed body"
  echo "  yields one sorry regardless of how many errors were reported for it."
} >"$OUT"

echo "wrote $OUT"
