#!/usr/bin/env bash
# One bounded profile session of the steady-state LDPC arms (jit:3be770d5).
#
# Runs inside one `dev/scripts/ccx1-bench-flock.sh --full-host` invocation,
# which run-profile.sh makes once per session. Every case runs `ldpc-profile`
# under `perf` started disabled (`-D -1`) with a control FIFO; the driver
# enables counting only around its profiled dispatches, so AList parsing,
# decoder construction and the warm pass stay outside every figure.
#
# stat cases: two `perf stat` counter groups of at most six user-space events
#   each, so neither group multiplexes on this processor's six general-purpose
#   counters (perf_event_paranoid 2 admits user-space events only).
# record cases: fixed-period user-cycle samples. A fixed period makes every
#   sample stand for the same number of cycles, so a symbol's or source line's
#   share of the samples estimates its share of the cycles. Reports list self
#   samples by DSO and symbol for every object, and by source line for the
#   harness executable, with the session's sample total.
# The first session also records the allocation census (deterministic counts)
# and one DWARF call-graph profile whose callers attribute the allocations.
#
# Usage: profile-session.sh <session-dir> <bin-dir> <expanded-cases> <first: yes|no>
set -euo pipefail

USAGE="usage: profile-session.sh <session-dir> <bin-dir> <expanded-cases> yes|no"
OUT="${1:?${USAGE}}"
BIN="${2:?${USAGE}}"
CASES="${3:?${USAGE}}"
FIRST="${4:?${USAGE}}"
mkdir -p "${OUT}/stat" "${OUT}/record"

GROUP_CORE=cycles:u,instructions:u,branches:u,branch-misses:u,stalled-cycles-frontend:u
GROUP_MEMORY=cycles:u,L1-dcache-loads:u,L1-dcache-load-misses:u,cache-references:u,cache-misses:u
# About 1 kHz at this processor's clock.
SAMPLE_PERIOD=4000037

# Runs `ldpc-profile` for one expanded case under the perf command given
# after `--`, with a fresh control FIFO pair.
profiled() {
    local stem="$1" arm="$2" bundle="$3" code="$4" core_arm="$5" batch="$6" passes="$7"
    local quality="$8" envpairs="$9"
    shift 9
    [[ "$1" == "--" ]] && shift
    local ctl="${OUT}/.ctl" ack="${OUT}/.ack"
    rm -f "${ctl}" "${ack}"
    mkfifo "${ctl}" "${ack}"
    local status=0
    # shellcheck disable=SC2086
    env RAYON_NUM_THREADS=1 GF2_LDPC_QUALITY="${quality}" ${envpairs} \
        "$@" -D -1 --control "fifo:${ctl},${ack}" -- \
        "${BIN}/ldpc-profile" --arm "${arm}" --bundle "${bundle}" --code "${code}" \
        --core-arm "${core_arm}" --batch "${batch}" --passes "${passes}" \
        --perf-control "${ctl},${ack}" >"${stem}.json" 2>"${stem}.err" || status=$?
    rm -f "${ctl}" "${ack}"
    echo "${status}" >"${stem}.status"
}

while IFS=$'\t' read -r kind label arm bundle code core_arm batch passes quality envpairs; do
    [[ -z "${kind}" || "${kind}" == \#* ]] && continue
    case "${kind}" in
        stat)
            for group in core memory; do
                case "${group}" in
                    core) events="${GROUP_CORE}" ;;
                    memory) events="${GROUP_MEMORY}" ;;
                esac
                stem="${OUT}/stat/${label}.${group}"
                profiled "${stem}" "${arm}" "${bundle}" "${code}" "${core_arm}" "${batch}" \
                    "${passes}" "${quality}" "${envpairs}" -- \
                    perf stat -x , -o "${stem}.csv" -e "${events}"
            done
            ;;
        record)
            stem="${OUT}/record/${label}"
            data="${OUT}/.perf.data"
            profiled "${stem}" "${arm}" "${bundle}" "${code}" "${core_arm}" "${batch}" \
                "${passes}" "${quality}" "${envpairs}" -- \
                perf record -q -e cycles:u -c "${SAMPLE_PERIOD}" -o "${data}"
            perf report -i "${data}" --stats 2>/dev/null | grep -m1 'SAMPLE events:' \
                >"${stem}.total.txt" || true
            perf report -i "${data}" --stdio --no-children -n -g none --percent-limit 0 \
                --sort dso,sym >"${stem}.symbols.txt" 2>/dev/null || true
            perf report -i "${data}" --stdio --no-children -n -g none --percent-limit 0 \
                --dsos ldpc-profile --sort srcline >"${stem}.srclines.txt" 2>/dev/null || true
            rm -f "${data}" "${data}.old"
            ;;
        *)
            echo "unknown case kind ${kind}" >&2
            exit 2
            ;;
    esac
done <"${CASES}"

[[ "${FIRST}" == yes ]] || exit 0

# ------------------------------------------------------------ first session
mkdir -p "${OUT}/callgraph"
while IFS=$'\t' read -r kind label arm bundle code core_arm batch passes quality envpairs; do
    [[ "${kind}" == record && "${arm}" == gf2 && "${core_arm}" == single-core ]] || continue
    RAYON_NUM_THREADS=1 "${BIN}/ldpc-alloc-census" --bundle "${bundle}" --code "${code}" \
        --batch "${batch}" >>"${OUT}/census.jsonl" 2>>"${OUT}/census.err"
    stem="${OUT}/callgraph/${label}"
    data="${OUT}/.perf.data"
    profiled "${stem}" "${arm}" "${bundle}" "${code}" "${core_arm}" "${batch}" \
        "${passes}" "${quality}" "${envpairs}" -- \
        perf record -q -e cycles:u -c "${SAMPLE_PERIOD}" --call-graph dwarf,16384 -o "${data}"
    perf report -i "${data}" --stdio --no-children -n -g caller --percent-limit 2 \
        --sort dso,sym >"${stem}.callers.txt" 2>/dev/null || true
    rm -f "${data}" "${data}.old"
done <"${CASES}"
