#!/usr/bin/env bash
# One bounded repetition of the logical-buffer profile (jit:18a87159).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${HERE}/record-invocation.sh"

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
    issue_command=(perf stat -x, -e "${GROUP_ISSUE}" -o "${file}.issue.csv" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}")
    record_invocation "${REP}/invocations.log" perf-stat-issue "${issue_command[@]}"
    "${issue_command[@]}" >"${file}.issue.json"

    memory_command=(perf stat -x, -e "${GROUP_MEMORY}" -o "${file}.memory.csv" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}")
    record_invocation "${REP}/invocations.log" perf-stat-memory "${memory_command[@]}"
    "${memory_command[@]}" >"${file}.memory.json"
done <"${CASES_FILE}"

while read -r case; do
    [[ -z "${case}" ]] && continue
    file="${REP}/${case}"
    # Flat sampling: the release executables carry no frame pointers and no
    # DWARF unwind tables, so a requested call graph would be unusable rather
    # than absent, and a per-symbol share is what the attribution needs.
    record_command=(perf record --quiet -F 4999 -o "${file}.perf.data" \
        -- "${DRIVER}" run --case "${case}" --seconds "${SECONDS_PER_PASS}")
    record_invocation "${REP}/invocations.log" perf-record "${record_command[@]}"
    "${record_command[@]}" >"${file}.record.json"
    # The samples are kept as their rendered report: the raw sample file holds
    # absolute paths of this run's build tree and is many times its size.
    report_command=(perf report --stdio --no-children --percent-limit 0.5 \
        -i "${file}.perf.data")
    record_invocation "${REP}/invocations.log" perf-report "${report_command[@]}"
    "${report_command[@]}" >"${file}.report.txt"
    rm -f "${file}.perf.data"
done <"${RECORDED_FILE}"
