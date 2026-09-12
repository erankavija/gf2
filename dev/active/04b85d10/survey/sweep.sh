#!/usr/bin/env bash
# One bounded timed session of the bit-storage consumer profile (jit:04b85d10).
#
# Runs inside the CCX1 lock wrapper, which run-profile.sh invokes once around
# this whole script per repetition. Holding the host mutex for one bounded
# session, rather than reacquiring it per measured process, is what the
# campaign discipline asks of a run that measures many short cells on a
# shared host.
#
# Usage: sweep.sh <output-dir> <bin-dir> <survey-dir> <cases.jsonl> yes|no
# The last argument says whether this session also records allocation-site
# backtraces, which are deterministic and so are recorded once per profile.
set -euo pipefail

USAGE="usage: sweep.sh <output-dir> <bin-dir> <survey-dir> <cases.jsonl> yes|no"
OUT="${1:?${USAGE}}"
BIN="${2:?${USAGE}}"
HERE="${3:?${USAGE}}"
CASES="${4:?${USAGE}}"
ALLOC_TRACE="${5:?${USAGE}}"
mkdir -p "${OUT}/perf-stat" "${OUT}/perf-report"
if [[ "${ALLOC_TRACE}" == yes ]]; then
    mkdir -p "${OUT}/alloc-trace"
fi

# --------------------------------------------------------------- correctness
# Correctness before timing: a timing comparison between two routes means
# nothing until the routes are known to agree.
"${BIN}/consumer-verify" >"${OUT}/verify.jsonl"

# --------------------------------------------------------------------- sweep
# The ladder is expanded once into a tab-separated table so the timed session
# spends its held mutex on measurement rather than on interpreter start-up.
python3 - "${CASES}" >"${OUT}/cases.tsv" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as handle:
    for line in handle:
        line = line.strip()
        if not line:
            continue
        entry = json.loads(line)
        env = " ".join(f"{k}={v}" for k, v in sorted(entry["env"].items()))
        print("\t".join([
            entry["family"],
            entry["path"],
            entry["cache_state"],
            str(entry["target_ms"]),
            json.dumps(entry["case"], sort_keys=True),
            env,
        ]))
PY

: >"${OUT}/profile.jsonl"
: >"${OUT}/sweep.log"
while IFS=$'\t' read -r family path state target body env_pairs; do
    echo "# ${family} ${path} ${body} env=[${env_pairs}]" >>"${OUT}/sweep.log"
    # shellcheck disable=SC2086
    if env RAYON_NUM_THREADS=1 ${env_pairs} GF2_BENCH=1 "${BIN}/consumer-profile" \
        --case "${body}" --path "${path}" --cache-state "${state}" \
        --target-ms "${target}" >>"${OUT}/profile.jsonl" 2>>"${OUT}/sweep.log"; then
        echo "# ok" >>"${OUT}/sweep.log"
    else
        echo "# FAILED: ${family} ${path} ${body}" >>"${OUT}/sweep.log"
    fi
done <"${OUT}/cases.tsv"

# ------------------------------------------------------------------ counters
# Two event groups of at most six counters each, so neither group multiplexes
# on this processor's six general-purpose counters. User-space events only:
# perf_event_paranoid is 2 on this host, which is recorded in host.txt.
GROUP_CORE=cycles:u,instructions:u,branches:u,branch-misses:u,stalled-cycles-frontend:u
GROUP_MEMORY=cycles:u,L1-dcache-loads:u,L1-dcache-load-misses:u,cache-references:u,cache-misses:u

while IFS='|' read -r label body path state target; do
    [[ -z "${label}" || "${label}" == \#* ]] && continue
    for group in core memory; do
        case "${group}" in
            core) events="${GROUP_CORE}" ;;
            memory) events="${GROUP_MEMORY}" ;;
        esac
        name="${OUT}/perf-stat/${label}.${group}.txt"
        {
            echo "# case: ${body}"
            echo "# path: ${path}  cache_state: ${state}  target_ms: ${target}"
            echo "# events: ${events}"
        } >"${name}"
        RAYON_NUM_THREADS=1 GF2_BENCH=1 perf stat -e "${events}" -- "${BIN}/consumer-profile" \
            --case "${body}" --path "${path}" --cache-state "${state}" \
            --target-ms "${target}" >>"${name}" 2>&1 || true
    done
done <"${HERE}/counter-cases.txt"

# ------------------------------------------------------------- call graphs
# A fixed sampling period, about 1 kHz at this processor's clock, makes every
# sample stand for the same number of cycles, so a symbol's share of the
# samples estimates its share of the cycles; a frequency target would start
# with short periods and over-weight preparation. `<label>.txt` is the call
# graph above 0.5% of the period; `<label>.samples.txt` lists the self
# samples of every symbol with the run's sample total, the counts from which
# a share and its interval are computed.
SAMPLE_PERIOD=4000037
while IFS='|' read -r label body path state target; do
    [[ -z "${label}" || "${label}" == \#* ]] && continue
    data="${OUT}/../perf-${label}.data"
    RAYON_NUM_THREADS=1 GF2_BENCH=1 perf record -q --call-graph dwarf,4096 -c "${SAMPLE_PERIOD}" -o "${data}" -- \
        "${BIN}/consumer-profile" --case "${body}" --path "${path}" \
        --cache-state "${state}" --target-ms "${target}" \
        >"${OUT}/perf-report/${label}.run.txt" 2>&1 || true
    {
        echo "# case: ${body}"
        echo "# path: ${path}  cache_state: ${state}  target_ms: ${target}"
        perf report -i "${data}" --stdio --no-children -n --percent-limit 0.5 2>/dev/null || true
    } >"${OUT}/perf-report/${label}.txt"
    stats="$(perf report -i "${data}" --stats 2>/dev/null || true)"
    {
        echo "# case: ${body}"
        echo "# path: ${path}  cache_state: ${state}  target_ms: ${target}"
        grep -m1 'SAMPLE events:' <<<"${stats}" | sed 's/^ */# /' || true
        perf report -i "${data}" --stdio --no-children -n -g none --percent-limit 0 2>/dev/null || true
    } >"${OUT}/perf-report/${label}.samples.txt"
    rm -f "${data}"
done <"${HERE}/report-cases.txt"

# -------------------------------------------------------- allocation sites
# The counting allocator says how many allocations a call makes; a backtrace of
# the first few says where. One short run per whole-consumer route that
# allocates inside its timed call.
[[ "${ALLOC_TRACE}" == yes ]] || exit 0
while IFS='|' read -r label body path state trace; do
    [[ -z "${label}" || "${label}" == \#* ]] && continue
    RAYON_NUM_THREADS=1 GF2_BENCH=1 GF2_PROFILE_ALLOC_TRACE="${trace}" \
        "${BIN}/consumer-profile" --case "${body}" --path "${path}" \
        --cache-state "${state}" --calls 2 \
        >"${OUT}/alloc-trace/${label}.json" 2>"${OUT}/alloc-trace/${label}.txt" || true
done <"${HERE}/alloc-trace-cases.txt"
