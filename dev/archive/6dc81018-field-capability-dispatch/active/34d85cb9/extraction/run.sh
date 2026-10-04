#!/usr/bin/env bash
# Spike runner for JIT issue 34d85cb9.
#
# Usage (from the worktree root):  ./dev/active/34d85cb9/extraction/run.sh R1
#
# Each run id RN has a command file `RN.cmd` next to this script holding the
# exact Charon command line that was executed.  The runner executes that file
# verbatim and tees combined stdout+stderr to `../logs/RN.log`, so the command
# text quoted in `findings.md` is byte-identical to what ran.
set -uo pipefail

RUN="${1:?usage: run.sh <RUN_ID>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
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
