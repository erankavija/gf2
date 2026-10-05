#!/usr/bin/env bash
# Entry-check cost campaigns (jit:bfc2ceab).
#
# Usage (from the worker worktree root):
#   run-entry-check.sh build
#   run-entry-check.sh window pilot|confirmation
#   run-entry-check.sh freeze
#
# `build` builds the two arms, the runner and the acceptance tool and runs the
# arm package's tests in both builds: the baseline with no flag and the
# candidate under `--cfg gf2_entry_checked`.
#
# `window` is the only timed action and the only command a queue line carries.
# It refuses outside a benchmark window, refuses a campaign input that is not
# committed and unmodified, prints the canonical execution log path before the
# first bounded run, resumes an interrupted campaign under its own identity,
# and finalizes and evaluates a complete campaign once. Re-running it on a
# finalized campaign re-evaluates and measures nothing.
#
# `freeze` derives the confirmation addendum from the committed pilot receipt
# with the repository's canonical freezer, reading no result of its own.
#
# Every numeric setting comes from the frozen addendum and the protocol's
# frozen shared settings; this script fixes only campaign identities, seeds,
# the pilot pair count and the session cell budget.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO=$(git -C "${HERE}" rev-parse --show-toplevel)
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

# The one live file called $1, repository-relative: receipt input snapshots hold
# byte copies under an `inputs` directory, and the live file lies outside them.
live() {
    local found
    mapfile -t found < <(git ls-files -- ":(glob)**/$1" | grep -v '/inputs/')
    [[ ${#found[@]} -eq 1 ]] || {
        echo "${#found[@]} live files are called $1; exactly one must be" >&2
        exit 2
    }
    printf '%s\n' "${found[0]}"
}

RESULTS=$(realpath --relative-to="${REPO}" "${HERE}")
ISSUE=$(basename "${RESULTS}")
FAMILY=$(dirname "$(live addendum-v4-entry-check-pilot.json)")
SURVEY=${FAMILY}/survey
LEDGER=${RESULTS}/entry-check-family-ledger.jsonl
PRODUCING=${SURVEY}/producing-inputs.json
PLAN_TOOL=${SURVEY}/make-plan.py
MANIFEST=${SURVEY}/entry-check-arm/Cargo.toml
LOG_CHECKER=$(live verify-campaign-log.py)
LOCK_WRAPPER=$(live ccx1-bench-flock.sh)
PILOT_ADDENDUM=${FAMILY}/addendum-v4-entry-check-pilot.json
CONFIRMATION_ADDENDUM=${FAMILY}/addendum-v4-entry-check-confirmation.json
DERIVATION=${FAMILY}/confirmation-derivation-entry-check.txt
ARM_TARGET="${REPO}/target/${ISSUE}-arm"
CHECKED_TARGET="${ARM_TARGET}-checked"
CHECKED_FLAGS='--cfg gf2_entry_checked'
ARM="${ARM_TARGET}/release/entry-check-arm"
STAGES="${REPO}/target/${ISSUE}-campaigns"
RUNNER="${REPO}/target/release/benchmark-ab-runner"
ACCEPTANCE="${REPO}/target/release/benchmark-acceptance"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"

# The benchmark window starts this script with the login environment only.
export PATH="${HOME}/.cargo/bin:${PATH}"
# The family measures and builds with the repository's minimum supported
# toolchain; the arms, the runner and the acceptance tool all use it.
export RUSTUP_TOOLCHAIN=1.95

# The two arms are one package and one command line; the candidate adds the
# cfg and takes the sibling target directory `make-plan.py` names.
arm_cargo() {
    local target=$1 flags=$2
    shift 2
    CARGO_TARGET_DIR="${target}" RUSTFLAGS="${flags}" ./scripts/cargo-budget.sh "$@" \
        --locked --manifest-path "${MANIFEST}"
}

build_binaries() {
    arm_cargo "${ARM_TARGET}" '' cargo build --release
    arm_cargo "${CHECKED_TARGET}" "${CHECKED_FLAGS}" cargo build --release
    ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
        --bin benchmark-ab-runner --bin benchmark-acceptance
}

ACTION=${1:?build, window or freeze}

if [[ "${ACTION}" == build ]]; then
    # Correctness precedes timing: each build's workloads reach their kernels
    # and publish the entry-point form its configuration names.
    arm_cargo "${ARM_TARGET}" '' --test cargo test --release
    arm_cargo "${CHECKED_TARGET}" "${CHECKED_FLAGS}" --test cargo test --release
    build_binaries
    exit 0
fi

if [[ "${ACTION}" == freeze ]]; then
    # The confirmation's resolution and pilot pin come from the committed pilot
    # receipt, never from a reading taken here; the retained cells are the
    # pilot addendum's frozen selection.
    for frozen in "${PILOT_ADDENDUM}" "${RESULTS}/r1-entry-check-pilot/receipt.json"; do
        git ls-files --error-unmatch "${frozen}" >/dev/null
        git diff --quiet HEAD -- "${frozen}" || {
            echo "${frozen} differs from the committed bytes" >&2
            exit 2
        }
    done
    # The canonical freezer, by its name and opening among the same-named scripts.
    FREEZER=$(python3 -B "$(live repository_files.py)" document freeze-confirmation.py \
        $'#!/usr/bin/env python3\n"""Freeze a confirmation addendum from its accepted pilot.')
    python3 -B "${FREEZER}" \
        --pilot-addendum "${PILOT_ADDENDUM}" \
        --pilot "${RESULTS}/r1-entry-check-pilot" \
        --frozen-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --output "${CONFIRMATION_ADDENDUM}" \
        --record "${DERIVATION}" \
        --cell spmm-small-n16 --cell spmm-small-n1024 --cell spmm-medium-n16 \
        --cell permanent-f3-n14 --cell ple-small-n128 --cell gemm-small-n32 \
        --selection-rationale \
        "The pilot addendum freezes this selection: P-20's tail-support bound admits six \
confirmatory comparisons on the family's first attempt; spmm-medium-n1024 is dropped because \
spmm-small-n1024 carries its amortized regime, and ple-medium-n128 because its panel check has \
the form of ple-small-n128. The dropped cells keep their pilot evidence." \
        --family-description \
        "Confirmatory stage of issue ${ISSUE}, protocol version 4: every cell below is \
confirmatory and decides, on fresh samples, whether the checked form of a caller-trusted kernel \
entry point is recorded as an accepted cost for its consumer. Question, arms, consumers and sizes \
are the pilot's; the resolution, the pilot pin and the retained cells are derived from the \
committed pilot receipt by the repository's canonical freezer, and the derivation record beside \
this addendum names them. Both arms remain the one arm package built twice from one tree: the \
baseline with no flag and the candidate with RUSTFLAGS \`${CHECKED_FLAGS}\`, so a pair differs in \
that cfg alone, and each execution reports the entry-point form its build publishes. The frozen \
decision rule is the pilot's: the checked form of a cost class is recorded as an accepted measured \
cost when every confirmatory cell of the class records pass, and a class with a cell that records \
fail or inconclusive goes back to the invoker with the measured ratio."
    echo "confirmation addendum: ${CONFIRMATION_ADDENDUM}" >&2
    echo "derivation record: ${DERIVATION}" >&2
    exit 0
fi

[[ "${ACTION}" == window ]] || {
    echo 'action must be build, window or freeze' >&2
    exit 2
}
[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo "timed runs of ${ISSUE} happen in the scheduled benchmark window; queue the command in \
the benchmark-window queue" >&2
    exit 2
}

CAMPAIGN_NAME=${2:?pilot or confirmation}
case "${CAMPAIGN_NAME}" in
    pilot)
        ADDENDUM=${PILOT_ADDENDUM}
        LABEL=pilot SEED=2026100501 MAX_CELLS=4 PILOT_PAIRS=12 ;;
    confirmation)
        ADDENDUM=${CONFIRMATION_ADDENDUM}
        LABEL=confirmation SEED=2026100511 MAX_CELLS=3 PILOT_PAIRS= ;;
    *) echo "unknown campaign ${CAMPAIGN_NAME}" >&2; exit 2 ;;
esac
CAMPAIGN=${ISSUE}-r1-entry-check-${CAMPAIGN_NAME}
STAGE="${STAGES}/${CAMPAIGN}"
PLAN="${STAGE}.plan.json"
OUT=${RESULTS}/r1-entry-check-${CAMPAIGN_NAME}

evaluate() {
    set +e
    "${ACCEPTANCE}" "${OUT}" | tee -a "${LAUNCH_LOG}"
    verdict=${PIPESTATUS[0]}
    set -e
}

if [[ -f "${OUT}/acceptance-summary.json" ]]; then
    LAUNCH_LOG="${OUT}/launcher.log"
    printf '# re-evaluation command: %s %s\n' "$0" "$*" >>"${LAUNCH_LOG}"
    evaluate
    printf '# acceptance exit: %s\n' "${verdict}" >>"${LAUNCH_LOG}"
    exit "${verdict}"
fi

# Publication precedes measurement, so the receipt pins committed bytes: the
# frozen addendum, the family ledger and the whole producing-input closure the
# campaign snapshots.
mapfile -t FROZEN < <(
    printf '%s\n' "${ADDENDUM}" "${LEDGER}" "${PRODUCING}"
    python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["build_inputs"]))' \
        "${PRODUCING}"
)
git ls-files --error-unmatch -- "${FROZEN[@]}" >/dev/null
git diff --quiet HEAD -- "${FROZEN[@]}" || {
    echo "a campaign input differs from its committed bytes:" >&2
    git diff --name-only HEAD -- "${FROZEN[@]}" >&2
    exit 2
}

build_binaries
mkdir -p "${STAGES}" "$(dirname "${OUT}")"
touch "${LOCK}"

python3 -B "${PLAN_TOOL}" \
    --addendum "${ADDENDUM}" \
    --label "${LABEL}" \
    --campaign-id "${CAMPAIGN}" \
    --campaign-seed "${SEED}" \
    --lock "$(realpath "${LOCK}")" \
    --executable "${ARM}" \
    --producing-manifest "${PRODUCING}" \
    --max-cells-per-session "${MAX_CELLS}" \
    ${PILOT_PAIRS:+--pilot-pairs "${PILOT_PAIRS}"} \
    --output "${PLAN}.projected"
"${RUNNER}" check "${PLAN}.projected"

# A resumed campaign keeps its own identity: the projection must reproduce the
# plan the first session measured, byte for byte, or the resume is refused.
if [[ -e "${PLAN}" ]]; then
    cmp -s "${PLAN}.projected" "${PLAN}" || {
        echo "${PLAN} differs from the current projection; resume is refused" >&2
        exit 2
    }
    rm "${PLAN}.projected"
    INVOCATION=resume
else
    mv "${PLAN}.projected" "${PLAN}"
    INVOCATION=new
fi

CHECKED_ARM="${CHECKED_TARGET}/release/entry-check-arm"
LAUNCH_LOG="${STAGE}.launcher.log"
{
    echo "# command: $0 $*"
    echo "# invocation: ${INVOCATION}"
    echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git rev-parse HEAD)"
    echo "# addendum: ${ADDENDUM} sha256=$(sha256sum "${ADDENDUM}" | cut -d' ' -f1)"
    echo "# producing manifest: ${PRODUCING} sha256=$(sha256sum "${PRODUCING}" | cut -d' ' -f1)"
    echo "# plan: ${PLAN} sha256=$(sha256sum "${PLAN}" | cut -d' ' -f1)"
    echo "# ledger: ${LEDGER} sha256=$(sha256sum "${LEDGER}" | cut -d' ' -f1)"
    echo "# baseline arm: ${ARM} sha256=$(sha256sum "${ARM}" | cut -d' ' -f1) RUSTFLAGS=''"
    echo "# candidate arm: ${CHECKED_ARM} sha256=$(sha256sum "${CHECKED_ARM}" | cut -d' ' -f1)" \
        "RUSTFLAGS='${CHECKED_FLAGS}'"
    echo "# runner: ${RUNNER} sha256=$(sha256sum "${RUNNER}" | cut -d' ' -f1)"
    echo "# acceptance: ${ACCEPTANCE} sha256=$(sha256sum "${ACCEPTANCE}" | cut -d' ' -f1)"
    echo "# toolchain: $(rustc --version)"
    echo "# load_avg_start: $(cut -d' ' -f1-3 /proc/loadavg)"
} >>"${LAUNCH_LOG}"
echo "campaign execution log: ${STAGE}/execution.log" >&2

# A campaign is judged from its own journal, never from an exit code: the shared
# checker answers whether the stage has reached its terminal `complete` record.
session=0
while ! python3 -B "${LOG_CHECKER}" --log "${STAGE}/execution.log" --stage-complete; do
    session=$((session + 1))
    echo "# session ${session} started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"${LAUNCH_LOG}"
    set +e
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 "${LOCK_WRAPPER}" --full-host \
        "${RUNNER}" run "${STAGE}" "${PLAN}" 2>&1 | tee -a "${LAUNCH_LOG}"
    rc=${PIPESTATUS[0]}
    set -e
    echo "# session ${session} exit: ${rc}" >>"${LAUNCH_LOG}"
    # Exit 3 pauses at the session cell budget and releases the mutex so a
    # queued sibling gets the host; exit 0 completes the campaign.
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
    echo "# load_avg_end: $(cut -d' ' -f1-3 /proc/loadavg)"
    echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"${LAUNCH_LOG}"

# A campaign whose every cell failed can still exit cleanly, so the campaign is
# verified from its own execution log against the plan it measured.
python3 -B "${LOG_CHECKER}" --log "${OUT}/execution.log" \
    --receipt "${OUT}/receipt.json" --plan "${PLAN}"
echo "receipt: ${OUT}" >&2
exit "${verdict}"
