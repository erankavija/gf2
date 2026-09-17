#!/usr/bin/env bash
# One repetition of the logical-buffer profile sweep (jit:18a87159).
#
# Invoked by run-logical-profile.sh inside the full-host lock, once per
# repetition, so the lock is released between repetitions.
#
# Usage: sweep-logical-profile.sh <rep-dir> <driver> <seconds> <issue-events> <memory-events> <cases-file> <recorded-file>
set -euo pipefail

REP="${1:?rep directory}"
DRIVER="${2:?driver}"
SECONDS_PER_PASS="${3:?seconds}"
GROUP_ISSUE="${4:?issue events}"
GROUP_MEMORY="${5:?memory events}"
CASES_FILE="${6:?cases file}"
RECORDED_FILE="${7:?recorded cases file}"

while read -r case; do
    [[ -z "${case}" ]] && continue
    file="${REP}/${case}"
    perf stat -x, -e "${GROUP_ISSUE}" -o "${file}.issue.csv" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}" \
        >"${file}.issue.json"
    perf stat -x, -e "${GROUP_MEMORY}" -o "${file}.memory.csv" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}" \
        >"${file}.memory.json"
done <"${CASES_FILE}"

while read -r case; do
    [[ -z "${case}" ]] && continue
    file="${REP}/${case}"
    perf record --quiet -g --call-graph dwarf -o "${file}.perf.data" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}" \
        >"${file}.record.json"
    # The call graph is kept as its rendered report: the raw sample file holds
    # absolute paths of this run's build tree and is many times its size.
    perf report --stdio --no-children --percent-limit 0.5 \
        -i "${file}.perf.data" >"${file}.report.txt"
    rm -f "${file}.perf.data"
done <"${RECORDED_FILE}"
