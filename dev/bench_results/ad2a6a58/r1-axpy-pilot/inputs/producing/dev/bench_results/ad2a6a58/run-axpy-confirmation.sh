#!/usr/bin/env bash
# Shipped GF(2^8) axpy lane campaigns (jit:ad2a6a58).
#
# Usage (from the worker worktree root):
#   dev/bench_results/ad2a6a58/run-axpy-confirmation.sh build
#   dev/bench_results/ad2a6a58/run-axpy-confirmation.sh window pilot|confirmation
#   dev/bench_results/ad2a6a58/run-axpy-confirmation.sh freeze
#   dev/bench_results/ad2a6a58/run-axpy-confirmation.sh tables
#
# `build` runs the correctness checks that precede timing — the two lanes of
# the measured executable agree on every byte coefficient at the word-boundary
# lengths, and the shipped crate's GF(2^8) table suites pass — and commits
# their evidence under dev/active/ad2a6a58/conformance/.
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
# `tables` regenerates the result tables from whatever is committed.
#
# Every numeric setting comes from the frozen addendum and the protocol's
# frozen shared settings; this script fixes only campaign identities, seeds,
# the pilot pair count and the session cell budget.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${HERE}/../../.." && pwd)"
[[ "$(pwd -P)" == "$(cd "${REPO}" && pwd -P)" ]] || {
    echo 'invoke from the worker worktree root' >&2
    exit 2
}

ISSUE=ad2a6a58
SURVEY=dev/active/${ISSUE}/survey
EVIDENCE=dev/active/${ISSUE}/conformance
RESULTS=dev/bench_results/${ISSUE}
LEDGER=${RESULTS}/axpy-family-ledger.jsonl
PRODUCING=${SURVEY}/producing-inputs.json
PLAN_TOOL=${SURVEY}/make-plan.py
LAUNCHER=${RESULTS}/run-axpy-confirmation.sh
PIN=dev/active/${ISSUE}/pinned-vector-confirmation.json
FREEZER=dev/active/c7113c5a/survey/freeze-confirmation.py
PILOT_ADDENDUM=dev/active/${ISSUE}/addendum-v4-axpy-pilot.json
CONFIRMATION_ADDENDUM=dev/active/${ISSUE}/addendum-v4-axpy-confirmation.json
DERIVATION=dev/active/${ISSUE}/confirmation-derivation-axpy.txt
ARM_TARGET="${REPO}/target/${ISSUE}-arm"
ARM="${ARM_TARGET}/release/gf256-axpy-arm"
VERIFY="${ARM_TARGET}/release/gf256-axpy-verify"
STAGES="${REPO}/target/${ISSUE}-campaigns"
RUNNER="${REPO}/target/release/benchmark-ab-runner"
ACCEPTANCE="${REPO}/target/release/benchmark-acceptance"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"

# The benchmark window starts this script with the login environment only.
export PATH="${HOME}/.cargo/bin:${PATH}"
# The family measures and builds with the repository's minimum supported
# toolchain; the arm, the runner and the acceptance tool all use it.
export RUSTUP_TOOLCHAIN=1.95

build_binaries() {
    CARGO_TARGET_DIR="${ARM_TARGET}" ./scripts/cargo-budget.sh cargo build --release --locked \
        --manifest-path "${SURVEY}/axpy-arm/Cargo.toml"
    ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
        --bin benchmark-ab-runner --bin benchmark-acceptance
}

ACTION=${1:?build, window, freeze or tables}

if [[ "${ACTION}" == build ]]; then
    mkdir -p "${EVIDENCE}"
    build_binaries
    # Correctness precedes timing: each check exits non-zero on a mismatch.
    "${VERIFY}" >"${EVIDENCE}/lane-equivalence.txt"
    {
        echo "# Shipped GF(2^8) table suites of the measured tree (jit:${ISSUE})"
        echo "# command: cargo nextest run -p gf2-core --all-features --cargo-profile ci-test"
        echo "#          --profile ci -E 'test(gf256) + binary_id(~gf256)'"
        ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features \
            --cargo-profile ci-test --profile ci -E 'test(gf256) + binary_id(~gf256)' 2>&1 |
            sed -E 's/\[[[:space:]]*[0-9]+\.[0-9]+s\]//g'
    } >"${EVIDENCE}/shipped-conformance.txt"
    python3 -B "${SURVEY}/make-producing-inputs.py"
    echo "correctness evidence: ${EVIDENCE}" >&2
    exit 0
fi

if [[ "${ACTION}" == tables ]]; then
    python3 -B "${SURVEY}/make-tables.py"
    exit 0
fi

if [[ "${ACTION}" == freeze ]]; then
    # The confirmation's resolution, margins and pilot pin come from the
    # committed pilot receipt, never from a reading taken here; the retained and
    # dropped cells follow the pilot addendum's frozen selection rule.
    for frozen in "${PILOT_ADDENDUM}" "${RESULTS}/r1-axpy-pilot/receipt.json"; do
        git ls-files --error-unmatch "${frozen}" >/dev/null
        git diff --quiet HEAD -- "${frozen}" || {
            echo "${frozen} differs from the committed bytes" >&2
            exit 2
        }
    done
    python3 -B "${FREEZER}" \
        --pilot-addendum "${PILOT_ADDENDUM}" \
        --pilot "${RESULTS}/r1-axpy-pilot" \
        --frozen-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --output "${CONFIRMATION_ADDENDUM}" \
        --record "${DERIVATION}" \
        --cell axpy-4k-element --cell axpy-128k-element --cell axpy-2m-stream-element \
        --cell axpy-4k-wide --cell axpy-128k-wide --cell axpy-2m-stream-wide \
        --selection-rationale \
        "The frozen selection rule of the pilot addendum: P-20's tail-support bound admits six \
confirmatory comparisons on this family's first attempt, so the confirmation keeps both element \
representations at an L1-resident size, an L2-resident size and the streaming size; it drops the \
8 MiB warm cells, whose memory-resident regime the streaming cells already carry, and the 1 KiB \
cold first-touch cells, whose one table-build window per candidate execution is the whole of the \
frozen max_flagged_fraction with no margin left, so a confirmatory slot spent there would measure \
the flagged-window policy rather than the lane. The dropped cells keep their pilot evidence." \
        --family-description \
        "Confirmatory stage of issue ad2a6a58, protocol version 4: every cell below is \
confirmatory and decides the shipped GF(2^8) product-table lane on fresh samples. Question, arms, \
representations and sizes are the pilot's; the resolution, the pilot pin and the retained cells \
are derived from the committed pilot receipt by the repository's canonical freezer, and the \
derivation record beside this addendum names them. Both arms remain one executable built from the \
shipped crate: the baseline arm holds every GF(2^8) call on the scalar element lane through the \
shipped process-global lane switch and the candidate arm leaves it clear, so a pair differs in \
the lane and in nothing else, and each execution reports the lane the shipped witness recorded \
for its measured calls. The frozen decision rule is the pilot's: the accelerated lane is retained \
when no confirmatory cell records fail and at least one records pass, and is removed otherwise, \
including when every confirmatory cell records not-material or inconclusive. Direction agreement \
is stated against the vector-family confirmation receipt of issue 19513245 pinned in \
dev/active/ad2a6a58/pinned-vector-confirmation.json, which is cited for its direction and not \
inherited as evidence."
    echo "confirmation addendum: ${CONFIRMATION_ADDENDUM}" >&2
    echo "derivation record: ${DERIVATION}" >&2
    exit 0
fi

[[ "${ACTION}" == window ]] || {
    echo 'action must be build, window, freeze or tables' >&2
    exit 2
}
[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo "timed runs of ${ISSUE} happen in the scheduled benchmark window; queue the command in \
dev/active/1a379447-zen3-cpu-performance/bench-window/queue.tsv" >&2
    exit 2
}

CAMPAIGN_NAME=${2:?pilot or confirmation}
case "${CAMPAIGN_NAME}" in
    pilot)
        ADDENDUM=${PILOT_ADDENDUM}
        LABEL=pilot SEED=2026091801 MAX_CELLS=5 PILOT_PAIRS=12 ;;
    confirmation)
        ADDENDUM=${CONFIRMATION_ADDENDUM}
        LABEL=confirmation SEED=2026091811 MAX_CELLS=3 PILOT_PAIRS= ;;
    *) echo "unknown campaign ${CAMPAIGN_NAME}" >&2; exit 2 ;;
esac
CAMPAIGN=${ISSUE}-r1-axpy-${CAMPAIGN_NAME}
STAGE="${STAGES}/${CAMPAIGN}"
PLAN="${STAGE}.plan.json"
OUT=${RESULTS}/r1-axpy-${CAMPAIGN_NAME}

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
# frozen addendum, the receipt pin, the family ledger and the whole
# producing-input closure the campaign snapshots. An uncommitted ledger
# reservation would leave the chain a receipt pins unreproducible from a fresh
# checkout.
mapfile -t FROZEN < <(
    printf '%s\n' "${ADDENDUM}" "${LEDGER}" "${PIN}" "${PRODUCING}"
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
    echo "# arm: ${ARM} sha256=$(sha256sum "${ARM}" | cut -d' ' -f1)"
    echo "# runner: ${RUNNER} sha256=$(sha256sum "${RUNNER}" | cut -d' ' -f1)"
    echo "# acceptance: ${ACCEPTANCE} sha256=$(sha256sum "${ACCEPTANCE}" | cut -d' ' -f1)"
    echo "# toolchain: $(rustc --version)"
    echo "# load_avg_start: $(cut -d' ' -f1-3 /proc/loadavg)"
} >>"${LAUNCH_LOG}"
echo "campaign execution log: ${STAGE}/execution.log" >&2

# A campaign is judged from its own journal, never from an exit code.
stage_complete() {
    [[ -f "${STAGE}/execution.log" ]] && python3 - "${STAGE}/execution.log" <<'PY'
import json, sys
terminal = [json.loads(line)["event"] for line in open(sys.argv[1])
            if json.loads(line)["event"] in ("complete", "failed", "paused", "budget-exhausted")]
raise SystemExit(0 if terminal and terminal[-1] == "complete" else 1)
PY
}

session=0
while ! stage_complete; do
    session=$((session + 1))
    echo "# session ${session} started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"${LAUNCH_LOG}"
    set +e
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
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
python3 - "${OUT}/execution.log" "${OUT}/receipt.json" "${PLAN}" <<'PY'
import collections, json, sys

log_path, receipt_path, plan_path = sys.argv[1:4]
events = [json.loads(line) for line in open(log_path)]
receipt = json.load(open(receipt_path))
plan = json.load(open(plan_path))

# Declared pairs per cell: an exploratory cell names its own count, a
# confirmatory one takes the protocol's frozen confirmatory pair count, which
# the receipt carries in its settings.
confirmatory = receipt["settings"]["confirmatory_pairs"]
declared = {cell["cell_id"]: cell["pilot_pairs"] or confirmatory for cell in plan["cells"]}

terminal = [event["event"] for event in events
            if event["event"] in ("complete", "failed", "paused", "budget-exhausted")]
if terminal != ["complete"]:
    raise SystemExit(f"the campaign ended {terminal} rather than one 'complete' record")

def cells_of(name):
    return [event for event in events
            if event["event"] == name and event.get("case") is not None]

starts = collections.Counter(event["case"]["cell_id"] for event in cells_of("cell-start"))
checkpoints = collections.Counter(event["case"]["cell_id"]
                                  for event in cells_of("checkpoint-accepted"))
completes = {event["case"]["cell_id"]: event["details"] for event in cells_of("cell-complete")}
abandoned = collections.Counter(event["case"]["cell_id"]
                                for event in cells_of("cell-abandoned"))

problems = []
for cell_id, pairs in sorted(declared.items()):
    # A restarted attempt is abandoned exactly once by a later session, so the
    # start count exceeds one only by the abandonments the journal records.
    if starts[cell_id] != 1 + abandoned[cell_id]:
        problems.append(f"{cell_id}: {starts[cell_id]} starts, {abandoned[cell_id]} abandoned")
    if checkpoints[cell_id] != 1:
        problems.append(f"{cell_id}: {checkpoints[cell_id]} checkpoints accepted")
    detail = completes.get(cell_id)
    if detail is None:
        problems.append(f"{cell_id}: never completed")
        continue
    if detail.get("status") != "measured":
        problems.append(f"{cell_id}: completed with status {detail.get('status')!r}")
    if detail.get("pairs") != pairs:
        problems.append(f"{cell_id}: {detail.get('pairs')} pairs, {pairs} declared")
extra = sorted(set(completes) - set(declared))
if extra:
    problems.append(f"the log completes undeclared cells: {', '.join(extra)}")
if problems:
    raise SystemExit("execution log: " + "; ".join(problems))
print(f"execution log: {len(declared)} declared cells, each started once, checkpointed once "
      f"and completed once at its declared pairs, terminal record 'complete'")
PY
echo "receipt: ${OUT}" >&2
exit "${verdict}"
