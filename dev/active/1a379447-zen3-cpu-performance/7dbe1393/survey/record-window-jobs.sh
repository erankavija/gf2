#!/usr/bin/env bash
# Record the benchmark-window log lines of this issue's jobs (jit:7dbe1393).
#
# Usage: record-window-jobs.sh <window.log> <job key>..., from the worktree
# root. The window runner writes one start line per job, with the host load
# averages it reads at that moment, and one exit line. The record holds those
# lines verbatim for the named keys.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
LOG="${1:?usage: record-window-jobs.sh <window.log> <job key>...}"
shift
[[ $# -gt 0 ]] || { echo 'name at least one job key' >&2; exit 2; }
RECORD="${HERE}/bench-window-jobs.txt"
{
    echo '# Benchmark-window log lines of the ragged-tail jobs (jit:7dbe1393)'
    echo '# start line fields: load=<1 min> <5 min> <15 min>'
    for key in "$@"; do
        grep -F -e " key=${key} " "${LOG}"
    done
} >"${RECORD}.tmp"
mv "${RECORD}.tmp" "${RECORD}"
cat "${RECORD}"
