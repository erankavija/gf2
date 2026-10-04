#!/usr/bin/env bash
# Runner for JIT issue 7d7c647c, following the e6ea0dde convention.
#
# Usage (from the repository root):
#   ./dev/active/7d7c647c/probes/run.sh S1
#
# Each run id holds its exact command line in `<ID>.cmd` next to this script.
# The runner prints that file into the log and then executes it verbatim, so
# every command quoted in `design.md` is byte-identical to what ran. Combined
# stdout+stderr goes to `logs/<ID>.log`; the log closes with the exit code and
# the measured wall time.
set -uo pipefail

RUN="${1:?usage: run.sh <RUN_ID>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
CMD="$HERE/$RUN.cmd"
LOG="$HERE/logs/$RUN.log"

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
