#!/usr/bin/env bash
# Profile-family campaigns (jit:63bad95d).
#
# Usage (from the worker worktree root):
#   run-profile-families.sh build
#   run-profile-families.sh smoke <family> pilot|confirmation
#   run-profile-families.sh window <family> pilot|confirmation
#   run-profile-families.sh freeze <family> [freezer margin options]
#
# `build` builds the harness executable once per build level, runs the harness
# contract in each level and builds the runner and the acceptance tool.
#
# `smoke` projects the plan the window measures, under a throwaway identity and
# an unused lock, and drives every arm of every cell once through
# `benchmark-ab-runner smoke`. Nothing in it is timed.
#
# `window` is the only timed action and the only command a queue line carries.
# It refuses outside a benchmark window, refuses a campaign input that is not
# committed and unmodified, rebuilds every executable it launches, prints the
# canonical execution log path before the first bounded run, resumes an
# interrupted campaign under its own identity, and finalizes and evaluates a
# complete campaign once. Re-running it on a finalized campaign re-evaluates
# and measures nothing.
#
# `freeze` derives the confirmation addendum from the committed pilot receipt
# with the repository's canonical freezer.
#
# Every numeric setting comes from the frozen addendum, the plan record and the
# protocol's shared settings; this script fixes campaign identities, seeds and
# the session cell budget.
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

ISSUE=63bad95d
RESULTS=$(realpath --relative-to="${REPO}" "${HERE}")
SURVEY=$(dirname "$(live make-family-plan.py)")
ACTIVE=$(dirname "${SURVEY}")
RECORD=${SURVEY}/planned-families.json
PRODUCING=${SURVEY}/profile-producing-inputs.json
MANIFEST=${SURVEY}/profile-arms/Cargo.toml
LOG_CHECKER=$(live verify-campaign-log.py)
LOCK_WRAPPER=$(live ccx1-bench-flock.sh)
STAGES="${REPO}/target/${ISSUE}-campaigns"
RUNNER="${REPO}/target/release/benchmark-ab-runner"
ACCEPTANCE="${REPO}/target/release/benchmark-acceptance"
LOCK="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
LEVELS=(conservative-portable tuned-portable native)

# The benchmark window starts this script without a login shell.
export PATH="${HOME}/.cargo/bin:${PATH}"
export RUSTUP_TOOLCHAIN=1.95
export CARGO_CI_NO_SCCACHE=1

arm_target() { printf '%s/target/%s-arm-%s' "${REPO}" "${ISSUE}" "$1"; }
arm_executable() { printf '%s/release/profile-arm' "$(arm_target "$1")"; }

# RUSTFLAGS of one build level, from the plan record's build table; empty for a
# level that declares none.
level_rustflags() {
    python3 -B -c 'import json, sys
print(json.load(open(sys.argv[1]))["builds"][sys.argv[2]]["rustflags"] or "")' "${RECORD}" "$1"
}

# Cargo for one build level: its own target directory and exactly the level's
# RUSTFLAGS, whatever the caller's environment holds.
level_cargo() {
    local level=$1 flags
    shift
    flags=$(level_rustflags "${level}")
    if [[ -n "${flags}" ]]; then
        env CARGO_TARGET_DIR="$(arm_target "${level}")" RUSTFLAGS="${flags}" "$@"
    else
        env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
            CARGO_TARGET_DIR="$(arm_target "${level}")" "$@"
    fi
}

build_binaries() {
    local level
    for level in "${LEVELS[@]}"; do
        level_cargo "${level}" ./scripts/cargo-budget.sh cargo build --release --locked \
            --manifest-path "${MANIFEST}"
    done
    ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
        --bin benchmark-ab-runner --bin benchmark-acceptance
}

arm_arguments() {
    local level
    for level in "${LEVELS[@]}"; do
        printf -- '--arm\n%s=%s\n' "${level}" "$(arm_executable "${level}")"
    done
}

ACTION=${1:?build, smoke, window or freeze}

if [[ "${ACTION}" == build ]]; then
    for level in "${LEVELS[@]}"; do
        level_cargo "${level}" ./scripts/cargo-budget.sh --test cargo test --release --locked \
            --manifest-path "${MANIFEST}"
    done
    build_binaries
    for level in "${LEVELS[@]}"; do
        printf '%s  profile-arm (%s)\n' \
            "$(sha256sum "$(arm_executable "${level}")" | cut -d' ' -f1)" "${level}"
    done
    exit 0
fi

FAMILY=${2:?family identifier}
python3 -B -c 'import json, sys
if sys.argv[2] not in [family["id"] for family in json.load(open(sys.argv[1]))["families"]]:
    raise SystemExit(f"{sys.argv[2]} is not a planned family")' "${RECORD}" "${FAMILY}"
SHORT=${FAMILY#"${ISSUE}"-}
LEDGER=${RESULTS}/${FAMILY}-ledger.jsonl
PILOT_ADDENDUM=${ACTIVE}/campaigns/${FAMILY}-pilot.json
CONFIRMATION_ADDENDUM=${ACTIVE}/campaigns/${FAMILY}-confirmation.json
DERIVATION=${ACTIVE}/campaigns/${FAMILY}-confirmation-derivation.txt

if [[ "${ACTION}" == freeze ]]; then
    shift 2
    for frozen in "${PILOT_ADDENDUM}" "${RESULTS}/${FAMILY}/r1-pilot/receipt.json"; do
        git ls-files --error-unmatch "${frozen}" >/dev/null
        git diff --quiet HEAD -- "${frozen}" || {
            echo "${frozen} differs from the committed bytes" >&2
            exit 2
        }
    done
    # The canonical freezer, by its name and opening among the same-named scripts.
    FREEZER=$(python3 -B "$(live repository_files.py)" document freeze-confirmation.py \
        $'#!/usr/bin/env python3\n"""Freeze a confirmation addendum from its accepted pilot.')
    HOLDOUT=()
    DECLARATION=$(python3 -B -c 'import json, sys
for family in json.load(open(sys.argv[1]))["families"]:
    if family["id"] == sys.argv[2]:
        print(family.get("holdout_declaration", ""))' "${RECORD}" "${FAMILY}")
    PURPOSE=$(python3 -B -c 'import json, sys
for family in json.load(open(sys.argv[1]))["families"]:
    if family["id"] == sys.argv[2]:
        print(family["purpose"])' "${RECORD}" "${FAMILY}")
    [[ -z "${DECLARATION}" ]] || HOLDOUT=(--holdout-cells "${SURVEY}/${DECLARATION}")
    python3 -B "${FREEZER}" \
        --pilot-addendum "${PILOT_ADDENDUM}" \
        --pilot "${RESULTS}/${FAMILY}/r1-pilot" \
        --frozen-utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --output "${CONFIRMATION_ADDENDUM}" \
        --record "${DERIVATION}" \
        --purpose "${PURPOSE}" \
        "${HOLDOUT[@]}" \
        "$@"
    exit 0
fi

STAGE_NAME=${3:?pilot or confirmation}
case "${STAGE_NAME}" in
    pilot) ADDENDUM=${PILOT_ADDENDUM} SEED_BASE=2026100710 ;;
    confirmation) ADDENDUM=${CONFIRMATION_ADDENDUM} SEED_BASE=2026100720 ;;
    *) echo "stage must be pilot or confirmation" >&2; exit 2 ;;
esac
# One seed per family and stage: the base plus the family's position in the
# plan record.
SEED=$((SEED_BASE + $(python3 -B -c 'import json, sys
print([family["id"] for family in json.load(open(sys.argv[1]))["families"]].index(sys.argv[2]))' \
    "${RECORD}" "${FAMILY}")))
MAX_CELLS=3

project_plan() {
    local campaign=$1 seed=$2 lock=$3 output=$4
    local arms
    mapfile -t arms < <(arm_arguments)
    python3 -B "${SURVEY}/make-family-plan.py" \
        --addendum "${ADDENDUM}" \
        --label "${STAGE_NAME}" \
        --campaign-id "${campaign}" \
        --campaign-seed "${seed}" \
        --lock "${lock}" \
        "${arms[@]}" \
        --producing-manifest "${PRODUCING}" \
        --max-cells-per-session "${MAX_CELLS}" \
        --output "${output}"
    "${RUNNER}" check "${output}"
}

if [[ "${ACTION}" == smoke ]]; then
    build_binaries
    SCRATCH=target/${ISSUE}-campaigns/smoke-${SHORT}-${STAGE_NAME}
    rm -rf "${SCRATCH}"
    mkdir -p "${SCRATCH}"
    # A throwaway file nothing locks: the smoke spawns no timed execution.
    touch "${SCRATCH}/unused.lock"
    project_plan "${ISSUE}-${SHORT}-${STAGE_NAME}-arms-smoke" "$((SEED + 100))" \
        "$(realpath "${SCRATCH}/unused.lock")" "${SCRATCH}/plan.json"
    "${RUNNER}" smoke "${SCRATCH}/plan.json" \
        --record "${SURVEY}/${SHORT}-${STAGE_NAME}-smoke.json"
    echo "smoke record: ${SURVEY}/${SHORT}-${STAGE_NAME}-smoke.json" >&2
    exit 0
fi

[[ "${ACTION}" == window ]] || {
    echo 'action must be build, smoke, window or freeze' >&2
    exit 2
}
[[ "${GF2_BENCH_WINDOW:-0}" == 1 ]] || {
    echo "timed runs of ${ISSUE} happen in the scheduled benchmark window; queue the command in \
the benchmark-window queue" >&2
    exit 2
}

CAMPAIGN=${ISSUE}-r1-${SHORT}-${STAGE_NAME}
STAGE="${STAGES}/${CAMPAIGN}"
PLAN="${STAGE}.plan.json"
OUT=${RESULTS}/${FAMILY}/r1-${STAGE_NAME}

evaluate() {
    set +e
    "${ACCEPTANCE}" "${OUT}" | tee -a "${LAUNCH_LOG}"
    verdict=${PIPESTATUS[0]}
    set -e
}

if [[ -f "${OUT}/acceptance-summary.json" ]]; then
    ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
        --bin benchmark-acceptance
    LAUNCH_LOG="${OUT}/launcher.log"
    printf '# re-evaluation command: %s %s\n' "$0" "$*" >>"${LAUNCH_LOG}"
    evaluate
    printf '# acceptance exit: %s\n' "${verdict}" >>"${LAUNCH_LOG}"
    exit "${verdict}"
fi

# Publication precedes measurement, so the receipt pins committed bytes: the
# frozen addendum, the family ledger and the whole producing-input closure.
python3 -B "${SURVEY}/make-profile-producing-inputs.py" --check
mapfile -t FROZEN < <(
    printf '%s\n' "${ADDENDUM}" "${LEDGER}" "${PRODUCING}"
    python3 -B -c 'import json, sys
print("\n".join(json.load(open(sys.argv[1]))["build_inputs"]))' "${PRODUCING}"
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
project_plan "${CAMPAIGN}" "${SEED}" "$(realpath "${LOCK}")" "${PLAN}.projected"

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
    for level in "${LEVELS[@]}"; do
        echo "# arm ${level}: $(arm_executable "${level}")" \
            "sha256=$(sha256sum "$(arm_executable "${level}")" | cut -d' ' -f1)" \
            "rustflags=$(level_rustflags "${level}")"
    done
    echo "# runner: ${RUNNER} sha256=$(sha256sum "${RUNNER}" | cut -d' ' -f1)"
    echo "# acceptance: ${ACCEPTANCE} sha256=$(sha256sum "${ACCEPTANCE}" | cut -d' ' -f1)"
    echo "# toolchain: $(rustc --version)"
    echo "# load_avg_start: $(cut -d' ' -f1-3 /proc/loadavg)"
} >>"${LAUNCH_LOG}"
echo "campaign execution log: ${STAGE}/execution.log" >&2

# A campaign is judged from its own journal, never from an exit code.
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
    # Exit 3 pauses at the session cell budget; exit 0 completes the campaign.
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

python3 -B "${LOG_CHECKER}" --log "${OUT}/execution.log" \
    --receipt "${OUT}/receipt.json" --plan "${PLAN}"
echo "receipt: ${OUT}" >&2
exit "${verdict}"
