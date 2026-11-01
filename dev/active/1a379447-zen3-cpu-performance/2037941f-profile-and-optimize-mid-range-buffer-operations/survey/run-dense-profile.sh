#!/usr/bin/env bash
# Profile frozen dense-parity cells through the existing campaign arm.
# prepare is untimed; window requires the scheduled full-host benchmark window.
#
# Usage: run-dense-profile.sh prepare|window [--cases <file>]
#
# Without --cases the session is the frozen dense-parity baseline's. A case
# file names another session: one `out=<directory>` line, one `title=<text>`
# line and one `family|campaign addendum file name|cell|route|build` line per case. Its
# cells are profiled under `<cell>.<route>`, so one cell serves both its arms.
set -Eeuo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "${HERE}" rev-parse --show-toplevel)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
SURVEY=dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/survey
CAMPAIGNS=dev/active/1a379447-zen3-cpu-performance/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns
ARM=target/e1f9a78f-arms/release/dense-arm
SCALAR_ARM=target/e1f9a78f-scalar-arm/release/dense-arm
TOOL=target/e1f9a78f-arms/release/dense-campaign
FLOCK=dev/scripts/ccx1-bench-flock.sh
OUT=dev/bench_results/2037941f/dense-baseline-profile-r2
REPETITIONS=9
ISSUE_EVENTS=cycles,instructions,branches,branch-misses
MEMORY_EVENTS=L1-dcache-loads,L1-dcache-load-misses,cache-references,cache-misses
RECORD_EVENT=cycles:u
SAMPLE_FREQUENCY=4999
FRESH_CASE=child-v2
REPORT_PERCENT_LIMIT=0
ANNOTATE_PERCENT_LIMIT=1.0
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95

ISOLATED="${CAMPAIGNS}/dense-isolated-fused-parity.json"
ALLOCATED="${CAMPAIGNS}/dense-allocated-matvec.json"
# family|campaign addendum|cell|route|build|label
CASES=(
    "2037941f-dense-isolated-fused-parity|${ISOLATED}|and-popcnt-8w-warm|and-popcnt-a|simd|and-popcnt-8w-warm"
    "2037941f-dense-isolated-fused-parity|${ISOLATED}|and-popcnt-64w-warm|and-popcnt-a|simd|and-popcnt-64w-warm"
    "2037941f-dense-allocated-matvec|${ALLOCATED}|matvec-r1024-8w-warm|matvec-a|simd|matvec-r1024-8w-warm"
    "2037941f-dense-allocated-matvec|${ALLOCATED}|matvec-r1024-64w-warm|matvec-a|simd|matvec-r1024-64w-warm"
    "2037941f-dense-allocated-matvec|${ALLOCATED}|matvec-r1024-64w-streaming|matvec-a|simd|matvec-r1024-64w-streaming"
    "2037941f-dense-allocated-matvec|${ALLOCATED}|matvec-r1024-8w-scalar-reference-warm|matvec-scalar-reference|scalar|matvec-r1024-8w-scalar-reference-warm"
)
TITLE=''
CASE_FILE=''

mode="${1:-}"
[[ "${mode}" == prepare || "${mode}" == window || "${mode}" == --case ]] || {
    sed -n '2,10p' "${BASH_SOURCE[0]}" >&2
    exit 2
}
if [[ "${mode}" == --case ]]; then
    OUT="$2"
elif [[ "${2:-}" == --cases ]]; then
    CASE_FILE="${3:?--cases needs a file}"
    CASES=()
    OUT=''
    while IFS= read -r line; do
        case "${line}" in
            out=*) OUT="${line#out=}" ;;
            title=*) TITLE="${line#title=}" ;;
            ''|'#'*) ;;
            *) IFS='|' read -r family addendum cell route build <<<"${line}"
               addendum="$(python3 -B "${HERE}/repo_artifacts.py" tracked "${addendum}")"
               CASES+=("${family}|${addendum}|${cell}|${route}|${build}|${cell}.${route}") ;;
        esac
    done <"${CASE_FILE}"
    [[ -n "${OUT}" && ${#CASES[@]} -gt 0 ]] || {
        echo "${CASE_FILE} names no output directory or no case" >&2
        exit 2
    }
elif [[ $# -gt 1 ]]; then
    sed -n '2,10p' "${BASH_SOURCE[0]}" >&2
    exit 2
fi
if [[ "${mode}" == window ]]; then
    [[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
        echo 'profiling runs only in the scheduled benchmark window' >&2
        exit 2
    }
    # The window unit starts from a tree no session has built.
    "${SURVEY}/run-dense-harness.sh" arms
fi
[[ -x "${ARM}" && -x "${SCALAR_ARM}" && -x "${TOOL}" ]] || {
    echo 'release dense arms are absent; run run-dense-harness.sh build first' >&2
    exit 2
}
"${TOOL}" pins >/dev/null
ADDENDA=()
for item in "${CASES[@]}"; do
    IFS='|' read -r family addendum _ <<<"${item}"
    [[ " ${ADDENDA[*]} " == *" ${addendum} "* ]] && continue
    "${TOOL}" verify --family "${family}" --addendum "${addendum}" >/dev/null
    ADDENDA+=("${addendum}")
done

cpu="$(python3 -c 'import os; print(min(os.sched_getaffinity(0)))')"
request() {
    local family="$1" addendum="$2" cell="$3" arm="$4"
    "${TOOL}" profile-request --family "${family}" --addendum "${addendum}" \
        --cell "${cell}" --arm "${arm}" --cpu "${cpu}"
}
if [[ "${mode}" == prepare ]]; then
    for item in "${CASES[@]}"; do
        IFS='|' read -r family addendum cell route build label <<<"${item}"
        request "${family}" "${addendum}" "${cell}" "${route}" >/dev/null
        echo "${label}: ${route} (${build})"
    done
    echo 'non-timed profile preparation complete' >&2
    exit 0
fi

[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo 'a profile case runs only in the scheduled benchmark window' >&2
    exit 2
}
# The baseline session attributes an accepted baseline; a case file's session
# states its own precondition in its plan.
[[ -n "${CASE_FILE}" || "${mode}" == --case ]] ||
for family in 2037941f-dense-isolated-fused-parity 2037941f-dense-allocated-matvec; do
    receipt="dev/bench_results/2037941f/${family}/v4-r1-pilot"
    [[ -f "${receipt}/receipt.json" && -f "${receipt}/acceptance-summary.json" ]] || {
        echo "baseline receipt is incomplete: ${receipt}" >&2
        exit 2
    }
    python3 - "${receipt}/acceptance-summary.json" "${family}" <<'PY'
import json, sys
summary = json.load(open(sys.argv[1]))
if summary["verdict"] != "accepted" or summary["family"]["family_id"] != sys.argv[2]:
    raise SystemExit("baseline acceptance is unavailable for " + sys.argv[2])
PY
done

mkdir -p "${OUT}"
LOG="${OUT}/execution.log"
touch "${LOG}"
on_exit() {
    local code="$1"
    if ((code != 0)); then
        echo "failed $(date -u +%Y-%m-%dT%H:%M:%SZ) exit=${code}" >>"${LOG}"
    fi
}
trap 'on_exit "$?"' EXIT
# A case runs inside the session whose parent wrote and checked the identity.
if [[ "${mode}" != --case ]]; then
    identity_arguments=()
    if [[ -n "${CASE_FILE}" ]]; then
        identity_arguments+=(--table "cases=${CASE_FILE}" --table-drop title= --table-file-name-column '|' 1)
        for addendum in "${ADDENDA[@]}"; do
            identity_arguments+=(--json "addendum=${addendum}")
        done
    else
        identity_arguments+=(--json "isolated-addendum=${ISOLATED}" --json "allocated-addendum=${ALLOCATED}")
        for item in "${CASES[@]}"; do
            identity_arguments+=(--observed "case=${item}")
        done
    fi
    for setting in REPETITIONS ISSUE_EVENTS MEMORY_EVENTS; do
        identity_arguments+=(--setting "${setting}=${!setting}")
    done
    # Result rendering and source pointers do not decide measurement or resume.
    identity="$(python3 -B "${REPO}/dev/scripts/resume_identity.py" --log "${LOG}" \
        --executable "arm=${ARM}" --executable "scalar=${SCALAR_ARM}" --executable "tool=${TOOL}" \
        --script "script=${BASH_SOURCE[0]}" \
        --observed "cpu=${cpu}" "${identity_arguments[@]}")"
    grep -q '^identity ' "${LOG}" || echo "identity ${identity}" >>"${LOG}"
fi
echo "profile execution log: ${OUT}/execution.log"
if [[ ! -e "${OUT}/host.txt" ]]; then
    {
        echo "# Runtime host and toolchain observations"
        date -u +%Y-%m-%dT%H:%M:%SZ
        uname -a
        rustc --version --verbose
        perf --version
        lscpu
        cat /proc/sys/kernel/perf_event_paranoid
        cat /sys/devices/system/cpu/smt/control
        for policy in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
            printf '%s %s\n' "${policy}" "$(cat "${policy}")"
        done
        for index in /sys/devices/system/cpu/cpu0/cache/index*; do
            printf '%s level=%s type=%s size=%s shared=%s\n' \
                "${index}" "$(cat "${index}/level")" "$(cat "${index}/type")" \
                "$(cat "${index}/size")" "$(cat "${index}/shared_cpu_list")"
        done
    } >"${OUT}/host.txt"
fi
{
    echo "session start $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf 'command='
    printf '%q ' "$0" "$@"
    echo
    echo "cpu=${cpu} affinity=$(taskset -pc $$ 2>&1)"
    echo "host=$(hostname) kernel=$(uname -r) rustc=$(rustc --version) perf=$(perf --version)"
} >>"${LOG}"

profile_case() {
    local family="$1" addendum="$2" cell="$3" route="$4" build="$5" label="$6"
    local binary="${ARM}"
    [[ "${build}" == scalar ]] && binary="${SCALAR_ARM}"
    if rg -q -F "cell ${label} done " "${LOG}"; then
        return
    fi
    local attempt=1
    while [[ -e "${OUT}/${label}/attempt-${attempt}" ]]; do
        attempt=$((attempt + 1))
    done
    local dir="${OUT}/${label}/attempt-${attempt}"
    mkdir -p "${dir}"
    printf '%q profile-request --family %q --addendum %q --cell %q --arm %q --cpu %q > %q\n' \
        "${TOOL}" "${family}" "${addendum}" \
        "${cell}" "${route}" "${cpu}" "${dir}/request.json" >>"${dir}/commands.txt"
    request "${family}" "${addendum}" "${cell}" "${route}" >"${dir}/request.json"
    echo "cell ${label} start $(date -u +%Y-%m-%dT%H:%M:%SZ) attempt=${attempt} request=$(sha256sum "${dir}/request.json" | cut -d' ' -f1) arm=$(sha256sum "${binary}" | cut -d' ' -f1)" >>"${LOG}"
    printf 'GF2_DENSE_ROUTE=%q GF2_TUNING_FRESH_CASE=%q taskset -c %q %q < %q\n' \
        "${route}" "${FRESH_CASE}" "${cpu}" "${binary}" "${dir}/request.json" >>"${dir}/commands.txt"
    local rep events kind
    for rep in $(seq 1 "${REPETITIONS}"); do
        for kind in issue memory; do
            if [[ "${kind}" == issue ]]; then events="${ISSUE_EVENTS}"; else events="${MEMORY_EVENTS}"; fi
            printf 'perf stat -x, -e %q -o %q -- taskset -c %q env GF2_DENSE_ROUTE=%q GF2_TUNING_FRESH_CASE=%q %q < %q > %q\n' \
                "${events}" "${dir}/${kind}-${rep}.csv" "${cpu}" "${route}" \
                "${FRESH_CASE}" "${binary}" "${dir}/request.json" "${dir}/${kind}-${rep}.result" >>"${dir}/commands.txt"
            perf stat -x, -e "${events}" -o "${dir}/${kind}-${rep}.csv" -- \
                taskset -c "${cpu}" env GF2_DENSE_ROUTE="${route}" GF2_TUNING_FRESH_CASE="${FRESH_CASE}" \
                "${binary}" <"${dir}/request.json" >"${dir}/${kind}-${rep}.result"
        done
        printf 'perf record --quiet -e %q -F %q -o %q -- taskset -c %q env GF2_DENSE_ROUTE=%q GF2_TUNING_FRESH_CASE=%q %q < %q > %q\n' \
            "${RECORD_EVENT}" "${SAMPLE_FREQUENCY}" "${dir}/cycles-${rep}.data" "${cpu}" "${route}" "${FRESH_CASE}" "${binary}" "${dir}/request.json" "${dir}/record-${rep}.result" >>"${dir}/commands.txt"
        perf record --quiet -e "${RECORD_EVENT}" -F "${SAMPLE_FREQUENCY}" -o "${dir}/cycles-${rep}.data" -- \
            taskset -c "${cpu}" env GF2_DENSE_ROUTE="${route}" GF2_TUNING_FRESH_CASE="${FRESH_CASE}" \
            "${binary}" <"${dir}/request.json" >"${dir}/record-${rep}.result"
        printf 'perf report --stdio --no-children --percent-limit %q -F overhead,sample,dso,symbol -i %q > %q\n' \
            "${REPORT_PERCENT_LIMIT}" "${dir}/cycles-${rep}.data" "${dir}/cycles-report-${rep}.txt" >>"${dir}/commands.txt"
        perf report --stdio --no-children --percent-limit "${REPORT_PERCENT_LIMIT}" -F overhead,sample,dso,symbol \
            -i "${dir}/cycles-${rep}.data" >"${dir}/cycles-report-${rep}.txt"
        printf 'perf annotate --stdio --percent-limit %q -i %q > %q\n' \
            "${ANNOTATE_PERCENT_LIMIT}" "${dir}/cycles-${rep}.data" "${dir}/cycles-annotate-${rep}.txt" >>"${dir}/commands.txt"
        perf annotate --stdio --percent-limit "${ANNOTATE_PERCENT_LIMIT}" -i "${dir}/cycles-${rep}.data" \
            >"${dir}/cycles-annotate-${rep}.txt"
        rm "${dir}/cycles-${rep}.data"
    done
    echo "cell ${label} done $(date -u +%Y-%m-%dT%H:%M:%SZ) attempt=${attempt}" >>"${LOG}"
}

if [[ "${mode}" == --case ]]; then
    profile_case "$3" "$4" "$5" "$6" "$7" "$8"
    exit 0
fi
for item in "${CASES[@]}"; do
    IFS='|' read -r family addendum cell route build label <<<"${item}"
    {
        printf 'wrapper command='
        printf '%q ' "${FLOCK}" --full-host "$0" --case "${OUT}" \
            "${family}" "${addendum}" "${cell}" "${route}" "${build}" "${label}"
        echo
    } >>"${LOG}"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host "$0" --case "${OUT}" \
        "${family}" "${addendum}" "${cell}" "${route}" "${build}" "${label}"
done
python3 -B "${SURVEY}/summarize-dense-profile.py" "${OUT}" ${TITLE:+"${TITLE}"} \
    >"${OUT}/profile-summary.md"
if ! rg -q '^complete ' "${LOG}"; then
    echo "complete $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"${LOG}"
fi
