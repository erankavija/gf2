#!/usr/bin/env bash
# One profile session inside the held CCX1 mutex (jit:19513245).
#
# Usage: sweep.sh <session-dir> <bin-dir> <survey-dir> <counters: yes|no>
#
# Measures every case of the ladder once and, when asked, records hardware
# counters for the cases `counter-cases.txt` names. Run only by
# `run-profile.sh`, which holds the mutex around it.
set -euo pipefail
SESSION="${1:?session dir}"
BIN="${2:?bin dir}"
SURVEY="${3:?survey dir}"
COUNTERS="${4:?yes or no}"

"${BIN}/consumer-profile" session "${SESSION}/cases.json"

if [[ "${COUNTERS}" == yes ]]; then
    mkdir -p "${SESSION}/counters"
    grep -v '^#' "${SURVEY}/counter-cases.txt" | while read -r case; do
        [[ -n "${case}" ]] || continue
        perf stat -x, -e cycles,instructions,branches,branch-misses,cache-references,cache-misses \
            -o "${SESSION}/counters/${case}.csv" \
            -- "${BIN}/consumer-profile" counters "${case}" \
            >"${SESSION}/counters/${case}.calls" 2>&1
    done
fi
