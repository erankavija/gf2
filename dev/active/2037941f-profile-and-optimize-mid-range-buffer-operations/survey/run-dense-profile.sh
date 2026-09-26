#!/usr/bin/env bash
# Profile the frozen dense-parity baseline through its existing campaign arm.
# prepare is untimed; window requires the scheduled full-host benchmark window.
set -Eeuo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
SURVEY=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/survey
CAMPAIGNS=dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns
ARM=target/e1f9a78f-arms/release/dense-arm
SCALAR_ARM=target/e1f9a78f-scalar-arm/release/dense-arm
TOOL=target/e1f9a78f-arms/release/dense-campaign
FLOCK=dev/scripts/ccx1-bench-flock.sh
OUT=dev/bench_results/2037941f/dense-baseline-profile
REPETITIONS=9
ISSUE_EVENTS=cycles,instructions,branches,branch-misses
MEMORY_EVENTS=L1-dcache-loads,L1-dcache-load-misses,cache-references,cache-misses
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95

CASES=(
    '2037941f-dense-isolated-fused-parity|and-popcnt-8w-warm|and-popcnt-a|simd'
    '2037941f-dense-isolated-fused-parity|and-popcnt-64w-warm|and-popcnt-a|simd'
    '2037941f-dense-allocated-matvec|matvec-r1024-8w-warm|matvec-a|simd'
    '2037941f-dense-allocated-matvec|matvec-r1024-64w-warm|matvec-a|simd'
    '2037941f-dense-allocated-matvec|matvec-r1024-64w-streaming|matvec-a|simd'
    '2037941f-dense-allocated-matvec|matvec-r1024-8w-scalar-reference-warm|matvec-scalar-reference|scalar'
)

mode="${1:-}"
[[ "${mode}" == prepare || "${mode}" == window || "${mode}" == --case ]] || {
    echo "usage: $0 prepare|window" >&2
    exit 2
}
[[ -x "${ARM}" && -x "${SCALAR_ARM}" && -x "${TOOL}" ]] || {
    echo 'release dense arms are absent; run run-dense-harness.sh build first' >&2
    exit 2
}
"${TOOL}" pins >/dev/null
for family in 2037941f-dense-isolated-fused-parity 2037941f-dense-allocated-matvec; do
    "${TOOL}" verify --family "${family}" --addendum "${CAMPAIGNS}/${family#2037941f-}.json" >/dev/null
done

cpu="$(python3 -c 'import os; print(min(os.sched_getaffinity(0)))')"
request() {
    local family="$1" cell="$2" arm="$3"
    "${TOOL}" profile-request --family "${family}" \
        --addendum "${CAMPAIGNS}/${family#2037941f-}.json" \
        --cell "${cell}" --arm "${arm}" --cpu "${cpu}"
}
if [[ "${mode}" == prepare ]]; then
    for item in "${CASES[@]}"; do
        IFS='|' read -r family cell route build <<<"${item}"
        request "${family}" "${cell}" "${route}" >/dev/null
        echo "${cell}: ${route} (${build})"
    done
    echo 'non-timed profile preparation complete' >&2
    exit 0
fi

[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo 'profiling runs only in the scheduled benchmark window' >&2
    exit 2
}
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
identity="arm=$(sha256sum "${ARM}" | cut -d' ' -f1) scalar=$(sha256sum "${SCALAR_ARM}" | cut -d' ' -f1) tool=$(sha256sum "${TOOL}" | cut -d' ' -f1) script=$(sha256sum "$0" | cut -d' ' -f1)"
identity+=" summary=$(sha256sum "${SURVEY}/summarize-dense-profile.py" | cut -d' ' -f1)"
identity+=" closure=$(sha256sum "${SURVEY}/dense-producing-inputs.json" | cut -d' ' -f1)"
identity+=" isolated-addendum=$(sha256sum "${CAMPAIGNS}/dense-isolated-fused-parity.json" | cut -d' ' -f1)"
identity+=" allocated-addendum=$(sha256sum "${CAMPAIGNS}/dense-allocated-matvec.json" | cut -d' ' -f1)"
first="$(sed -n 's/^identity //p' "${LOG}" | head -n 1)"
[[ -z "${first}" || "${first}" == "${identity}" ]] || {
    echo 'profile producing identity changed; resume refused' >&2
    exit 2
}
[[ -n "${first}" ]] || echo "identity ${identity}" >>"${LOG}"
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
    local family="$1" cell="$2" route="$3" build="$4"
    local binary="${ARM}"
    [[ "${build}" == scalar ]] && binary="${SCALAR_ARM}"
    if rg -q "^cell ${cell} done " "${LOG}"; then
        return
    fi
    local attempt=1
    while [[ -e "${OUT}/${cell}/attempt-${attempt}" ]]; do
        attempt=$((attempt + 1))
    done
    local dir="${OUT}/${cell}/attempt-${attempt}"
    mkdir -p "${dir}"
    printf '%q profile-request --family %q --addendum %q --cell %q --arm %q --cpu %q > %q\n' \
        "${TOOL}" "${family}" "${CAMPAIGNS}/${family#2037941f-}.json" \
        "${cell}" "${route}" "${cpu}" "${dir}/request.json" >>"${dir}/commands.txt"
    request "${family}" "${cell}" "${route}" >"${dir}/request.json"
    echo "cell ${cell} start $(date -u +%Y-%m-%dT%H:%M:%SZ) attempt=${attempt} request=$(sha256sum "${dir}/request.json" | cut -d' ' -f1) arm=$(sha256sum "${binary}" | cut -d' ' -f1)" >>"${LOG}"
    printf 'GF2_DENSE_ROUTE=%q GF2_TUNING_FRESH_CASE=child-v2 taskset -c %q %q < %q\n' \
        "${route}" "${cpu}" "${binary}" "${dir}/request.json" >>"${dir}/commands.txt"
    local rep events kind
    for rep in $(seq 1 "${REPETITIONS}"); do
        for kind in issue memory; do
            if [[ "${kind}" == issue ]]; then events="${ISSUE_EVENTS}"; else events="${MEMORY_EVENTS}"; fi
            printf 'perf stat -x, -e %q -o %q -- taskset -c %q env GF2_DENSE_ROUTE=%q GF2_TUNING_FRESH_CASE=child-v2 %q < %q > %q\n' \
                "${events}" "${dir}/${kind}-${rep}.csv" "${cpu}" "${route}" \
                "${binary}" "${dir}/request.json" "${dir}/${kind}-${rep}.result" >>"${dir}/commands.txt"
            perf stat -x, -e "${events}" -o "${dir}/${kind}-${rep}.csv" -- \
                taskset -c "${cpu}" env GF2_DENSE_ROUTE="${route}" GF2_TUNING_FRESH_CASE=child-v2 \
                "${binary}" <"${dir}/request.json" >"${dir}/${kind}-${rep}.result"
        done
        printf 'perf record --quiet -e cycles:u -F 4999 -o %q -- taskset -c %q env GF2_DENSE_ROUTE=%q GF2_TUNING_FRESH_CASE=child-v2 %q < %q > %q\n' \
            "${dir}/cycles-${rep}.data" "${cpu}" "${route}" "${binary}" "${dir}/request.json" "${dir}/record-${rep}.result" >>"${dir}/commands.txt"
        perf record --quiet -e cycles:u -F 4999 -o "${dir}/cycles-${rep}.data" -- \
            taskset -c "${cpu}" env GF2_DENSE_ROUTE="${route}" GF2_TUNING_FRESH_CASE=child-v2 \
            "${binary}" <"${dir}/request.json" >"${dir}/record-${rep}.result"
        printf 'perf report --stdio --no-children --percent-limit 0 -F overhead,sample,dso,symbol -i %q > %q\n' \
            "${dir}/cycles-${rep}.data" "${dir}/cycles-report-${rep}.txt" >>"${dir}/commands.txt"
        perf report --stdio --no-children --percent-limit 0 -F overhead,sample,dso,symbol \
            -i "${dir}/cycles-${rep}.data" >"${dir}/cycles-report-${rep}.txt"
        printf 'perf annotate --stdio --percent-limit 1.0 -i %q > %q\n' \
            "${dir}/cycles-${rep}.data" "${dir}/cycles-annotate-${rep}.txt" >>"${dir}/commands.txt"
        perf annotate --stdio --percent-limit 1.0 -i "${dir}/cycles-${rep}.data" \
            >"${dir}/cycles-annotate-${rep}.txt"
        rm "${dir}/cycles-${rep}.data"
    done
    echo "cell ${cell} done $(date -u +%Y-%m-%dT%H:%M:%SZ) attempt=${attempt}" >>"${LOG}"
}

if [[ "${mode}" == --case ]]; then
    profile_case "$3" "$4" "$5" "$6"
    exit 0
fi
for item in "${CASES[@]}"; do
    IFS='|' read -r family cell route build <<<"${item}"
    {
        printf 'wrapper command='
        printf '%q ' "${FLOCK}" --full-host "$0" --case "${OUT}" \
            "${family}" "${cell}" "${route}" "${build}"
        echo
    } >>"${LOG}"
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${FLOCK}" --full-host "$0" --case "${OUT}" \
        "${family}" "${cell}" "${route}" "${build}"
done
python3 -B "${SURVEY}/summarize-dense-profile.py" "${OUT}" >"${OUT}/profile-summary.md"
if ! rg -q '^complete ' "${LOG}"; then
    echo "complete $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"${LOG}"
fi
