#!/usr/bin/env bash
# Resumable protocol-v4 exploratory DVB-T2 interleaver campaign (jit:9fb40c83).
# Usage: run-dvb-campaign.sh [run-id]
set -euo pipefail

[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo 'timed DVB profiling runs only in the scheduled benchmark window' >&2
    exit 2
}

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}
RUN_ID="${1:-v4-r2}"
ADDENDUM=dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-addendum.json
PRODUCING=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/dvb-producing-inputs.json
LEDGER=dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-trial-ledger.jsonl
PLAN_TOOL=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/make-dvb-plan.py
LAUNCHER=dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/run-dvb-campaign.sh
TARGET="${REPO}/target/9fb40c83-arms-native"
ARM="${TARGET}/release/dvb-profile-arm"
CAMPAIGN="${RUN_ID}-9fb40c83-dvb-interleave-profile"
STAGE="${REPO}/target/9fb40c83-campaigns/${CAMPAIGN}"
PLAN="${STAGE}.plan.json"
OUT="${REPO}/dev/bench_results/c04dd4ac/dvb-interleave-profile/${RUN_ID}-pilot"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"

export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1
mkdir -p "$(dirname "${STAGE}")" "$(dirname "${OUT}")"
touch "${LOCK}"
LOCK="$(realpath "${LOCK}")"

for path in "${ADDENDUM}" "${PRODUCING}" "${LEDGER}" "${PLAN_TOOL}" "${LAUNCHER}"; do
    git ls-files --error-unmatch "${path}" >/dev/null
    git diff --quiet HEAD -- "${path}" || {
        echo "${path} differs from the committed campaign input" >&2
        exit 2
    }
done
[[ -x "${ARM}" ]] || {
    echo "${ARM} is absent; run ${HERE#"${REPO}/"}/build-dvb-harness.sh before the campaign" >&2
    exit 2
}

./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER="$(realpath target/release/benchmark-ab-runner)"
ACCEPTANCE="$(realpath target/release/benchmark-acceptance)"

evaluate() {
    set +e
    "${ACCEPTANCE}" "${OUT}" | tee -a "${LAUNCH_LOG}"
    verdict=${PIPESTATUS[0]}
    set -e
}

if [[ -f "${OUT}/receipt.json" ]]; then
    LAUNCH_LOG="${OUT}/launcher.log"
    printf '# re-evaluation_utc: %s command: %s %s\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$0" "$*" >>"${LAUNCH_LOG}"
    evaluate
    printf '# acceptance exit: %s\n' "${verdict}" >>"${LAUNCH_LOG}"
    exit "${verdict}"
fi

python3 -B "${PLAN_TOOL}" \
    --addendum "${ADDENDUM}" \
    --campaign-id "${CAMPAIGN}" \
    --campaign-seed 20260915 \
    --lock "${LOCK}" \
    --executable "${ARM}" \
    --producing-manifest "${PRODUCING}" \
    --output "${PLAN}.projected" \
    --max-cells-per-session 2 \
    --pilot-pairs 6
if [[ -e "${PLAN}" ]]; then
    cmp -s "${PLAN}.projected" "${PLAN}" || {
        echo "${PLAN} differs from the current projection; resume is refused" >&2
        exit 2
    }
    rm "${PLAN}.projected"
    INVOCATION=resume
else
    git diff --quiet HEAD -- "${LEDGER}" || {
        echo "${LEDGER} differs from HEAD before a new campaign" >&2
        exit 2
    }
    mv "${PLAN}.projected" "${PLAN}"
    INVOCATION=new
fi

LAUNCH_LOG="${STAGE}.launcher.log"
{
    echo "# command: $0 $*"
    echo "# invocation: ${INVOCATION}"
    echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git rev-parse HEAD)"
    echo "# xdsopl revision: 32357d8ad55a6a302c34e093759f0454e45cca56"
    echo "# addendum: ${ADDENDUM} sha256=$(sha256sum "${ADDENDUM}" | cut -d' ' -f1)"
    echo "# producing manifest: ${PRODUCING} sha256=$(sha256sum "${PRODUCING}" | cut -d' ' -f1)"
    echo "# executable: ${ARM} sha256=$(sha256sum "${ARM}" | cut -d' ' -f1)"
    echo "# stage: ${STAGE}"
    echo "# plan: ${PLAN} sha256=$(sha256sum "${PLAN}" | cut -d' ' -f1)"
    echo "# toolchain: $(rustc --version --verbose | tr '\n' ';')"
    echo "# load_avg_start: $(uptime)"
} >>"${LAUNCH_LOG}"
echo "campaign execution log: ${STAGE}/execution.log" >&2

stage_complete() {
    [[ -f "${STAGE}/execution.log" ]] && python3 - "${STAGE}/execution.log" <<'PY'
import json, sys
events = [json.loads(line)["event"] for line in open(sys.argv[1])]
terminal = [event for event in events if event in ("complete", "failed", "paused", "budget-exhausted")]
raise SystemExit(0 if terminal and terminal[-1] == "complete" else 1)
PY
}

session=0
while ! stage_complete; do
    session=$((session + 1))
    set +e
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
        "${RUNNER}" run "${STAGE}" "${PLAN}" | tee -a "${LAUNCH_LOG}"
    rc=${PIPESTATUS[0]}
    set -e
    echo "# session ${session} exit: ${rc}" >>"${LAUNCH_LOG}"
    case "${rc}" in
        0) break ;;
        3) ;;
        *) echo "session ${session} failed with ${rc}" >&2; exit "${rc}" ;;
    esac
done

"${RUNNER}" finalize "${STAGE}" "${OUT}" | tee -a "${LAUNCH_LOG}"
cp "${LAUNCH_LOG}" "${OUT}/launcher.log"
LAUNCH_LOG="${OUT}/launcher.log"
evaluate
{
    echo "# acceptance exit: ${verdict}"
    echo "# sessions this invocation: ${session}"
    echo "# load_avg_end: $(uptime)"
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"${LAUNCH_LOG}"
echo "exploratory receipt: ${OUT}" >&2
exit "${verdict}"
