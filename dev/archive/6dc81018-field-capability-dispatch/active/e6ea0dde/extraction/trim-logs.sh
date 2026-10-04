#!/usr/bin/env bash
# Trim the raw run logs of dev/active/e6ea0dde/logs/ into committed evidence.
#
# Strips ANSI escapes, terminal progress-bar redraws, and the rustc dead-code
# warning blocks that dominate every `charon cargo` run, keeping the command
# header, Charon/Aeneas diagnostics, exceptions and the exit/elapsed footer.
# Same filter set as dev/active/34d85cb9/extraction/trim-logs.sh.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="${1:-$HERE/../logs}"
DST="${2:-$HERE/../logs-trimmed}"
mkdir -p "$DST"

for f in "$SRC"/*.log; do
  base="$(basename "$f")"
  tr '\r' '\n' <"$f" \
    | sed -e 's/\x1b\[[0-9;]*[A-Za-z]//g' -e 's/\x1b\[?25[lh]//g' \
    | grep -vE '^(Translated|Applied prepasses|Post-processed|Extracted)' \
    | grep -vE '^warning: ' \
    | grep -vE '^[0-9]+ \|' \
    | grep -vE '^\.\.\.$' \
    | grep -vE '^[[:space:]]+(\||-->|=|\^)' \
    | grep -vE '^\s+\^+' \
    | grep -vE '^\s+--> ' \
    | grep -vE '^\s*Constraints in scope:' \
    | sed -e 's/[[:space:]]*$//' \
    | cat -s \
    >"$DST/$base"
done

echo "trimmed $(ls -1 "$DST" | wc -l) logs into $DST"
