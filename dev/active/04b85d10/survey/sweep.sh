#!/usr/bin/env bash
# One bounded timed session of the bit-storage consumer profile (jit:04b85d10).
#
# Runs inside the CCX1 lock wrapper, which run-profile.sh invokes once around
# this whole script. Holding the host mutex for one bounded session, rather
# than reacquiring it per measured process, is what the campaign discipline
# asks of a run that measures many short cells on a shared host.
#
# Usage: sweep.sh <output-dir> <bin-dir> <survey-dir>
set -euo pipefail

OUT="${1:?usage: sweep.sh <output-dir> <bin-dir> <survey-dir>}"
BIN="${2:?bin dir}"
HERE="${3:?survey dir}"

# --------------------------------------------------------------- correctness
# Correctness before timing: a timing comparison between two routes means
# nothing until the routes are known to agree.
"${BIN}/consumer-verify" >"${OUT}/verify.jsonl"

# --------------------------------------------------------------------- sweep
# The ladder is expanded once into a tab-separated table so the timed session
# spends its held mutex on measurement rather than on interpreter start-up.
python3 - "${OUT}/cases.jsonl" >"${OUT}/cases.tsv" <<'PY'
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
    if env ${env_pairs} GF2_BENCH=1 "${BIN}/consumer-profile" \
        --case "${body}" --path "${path}" --cache-state "${state}" \
        --target-ms "${target}" >>"${OUT}/profile.jsonl" 2>>"${OUT}/sweep.log"; then
        echo "# ok" >>"${OUT}/sweep.log"
    else
        echo "# FAILED: ${family} ${path} ${body}" >>"${OUT}/sweep.log"
    fi
done <"${OUT}/cases.tsv"

# ------------------------------------------------------------------ counters
# Two event groups of at most six counters each, so neither group multiplexes
# on this processor's six general-purpose counters.
GROUP_CORE=cycles,instructions,branches,branch-misses,stalled-cycles-frontend
GROUP_MEMORY=cycles,L1-dcache-loads,L1-dcache-load-misses,cache-references,cache-misses

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
        GF2_BENCH=1 perf stat -e "${events}" -- "${BIN}/consumer-profile" \
            --case "${body}" --path "${path}" --cache-state "${state}" \
            --target-ms "${target}" >>"${name}" 2>&1 || true
    done
done <"${HERE}/counter-cases.txt"

# ------------------------------------------------------------- call graphs
while IFS='|' read -r label body path state target; do
    [[ -z "${label}" || "${label}" == \#* ]] && continue
    data="${OUT}/../perf-${label}.data"
    GF2_BENCH=1 perf record -q --call-graph dwarf,4096 -F 999 -o "${data}" -- \
        "${BIN}/consumer-profile" --case "${body}" --path "${path}" \
        --cache-state "${state}" --target-ms "${target}" \
        >"${OUT}/perf-report/${label}.run.txt" 2>&1 || true
    {
        echo "# case: ${body}"
        echo "# path: ${path}  cache_state: ${state}  target_ms: ${target}"
        perf report -i "${data}" --stdio --no-children --percent-limit 0.5 2>/dev/null
    } >"${OUT}/perf-report/${label}.txt"
    rm -f "${data}"
done <"${HERE}/report-cases.txt"
