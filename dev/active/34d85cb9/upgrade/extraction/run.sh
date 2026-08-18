#!/usr/bin/env bash
# Runner for the newest-upstream leg of JIT issue 34d85cb9.
#
# Same contract as dev/active/34d85cb9/extraction/run.sh: execute `<RUN>.cmd`
# verbatim from the repository root and tee to `../logs/<RUN>.log`, so the
# command text quoted in findings.md is byte-identical to what ran.
#
# The only difference from the baseline runner is the path depth (this script
# lives one directory deeper) and that the .cmd files here invoke the newly
# built Charon/Aeneas by absolute path instead of the ones on PATH.
set -uo pipefail

RUN="${1:?usage: run.sh <RUN_ID>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../../.." && pwd)"
CMD="$HERE/$RUN.cmd"
LOG="$HERE/../logs/$RUN.log"

[ -f "$CMD" ] || { echo "no such command file: $CMD" >&2; exit 2; }
mkdir -p "$(dirname "$LOG")"

cd "$ROOT"
echo "=== $RUN === $(date -Is)" | tee "$LOG"
echo "--- command ---" | tee -a "$LOG"
cat "$CMD" | tee -a "$LOG"
echo "--- output ---" | tee -a "$LOG"
START=$(date +%s)
bash "$CMD" >>"$LOG" 2>&1
RC=$?
END=$(date +%s)
echo "--- exit=$RC elapsed=$((END - START))s ---" | tee -a "$LOG"
exit $RC
