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
    # Flat sampling: the release executables carry no frame pointers and no
    # DWARF unwind tables, so a requested call graph would be unusable rather
    # than absent, and a per-symbol share is what the attribution needs.
    perf record --quiet -F 4999 -o "${file}.perf.data" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}" \
        >"${file}.record.json"
    # The samples are kept as their rendered report: the raw sample file holds
    # absolute paths of this run's build tree and is many times its size.
    perf report --stdio --no-children --percent-limit 0.5 \
        -i "${file}.perf.data" >"${file}.report.txt"
    rm -f "${file}.perf.data"
done <"${RECORDED_FILE}"
