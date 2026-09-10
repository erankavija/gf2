#!/usr/bin/env bash
# Protocol-v3 campaigns of jit:26465e6c.
#
# Usage: dev/bench_results/26465e6c/run-campaign.sh <popcount|and-popcnt> <pilot|confirmation> <prepare|run|finalize>
#
# prepare   builds the runner, the acceptance tool and the survey binaries
#           (release, Rust 1.95, offline), records their identities, runs the
#           equivalence validation over the generic matrix and over every
#           planned cell, records the emitted instructions of the arm binary,
#           and projects and checks the plan. It takes no timing lock.
# run       measures the plan as bounded sessions under the canonical CCX1
#           mutex until the campaign completes. A session exits 3 when it
#           pauses at the plan's cell budget; the loop then releases the mutex,
#           yields briefly and resumes. Re-invoking `run` after an interruption
#           resumes the same campaign identity without repeating a cell.
# finalize  assembles the receipt directory, copies the launcher log and the
#           instruction record into it, and evaluates acceptance.
#
# Every numeric setting comes from the frozen addendum and the protocol's
# shared settings. Builds finish before timed work and take the shared side of
# the mutex through scripts/cargo-budget.sh; timed work holds it exclusively
# with CARGO_CI_NO_LOCK=1 for anything cargo inside it.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FAMILY=${1:-}
MODE=${2:-}
ACTION=${3:-}
case "$FAMILY:$MODE" in
  popcount:pilot|popcount:confirmation|and-popcnt:pilot|and-popcnt:confirmation) ;;
  *) echo "usage: $0 <popcount|and-popcnt> <pilot|confirmation> <prepare|run|finalize>" >&2; exit 2 ;;
esac
ISSUE=26465e6c
SURVEY=dev/active/$ISSUE/survey
ADDENDUM=dev/active/$ISSUE/addendum-$FAMILY-v3-$MODE.json
LEDGER=dev/bench_results/$ISSUE/v3-$FAMILY-family-ledger.jsonl
OUT=dev/bench_results/$ISSUE/v3-$FAMILY-$MODE
CAMPAIGN=$ISSUE-v3-$FAMILY-$MODE
CAMPAIGNS=$repo/target/$ISSUE-campaigns
STAGE=$CAMPAIGNS/$CAMPAIGN
PLAN=$STAGE.plan.json
LAUNCH_LOG=$STAGE.launcher.log
OBSERVATION=$STAGE.instruction-observation.txt
HARNESS_TARGET=$repo/target/popcount-survey-$ISSUE
ARM=$HARNESS_TARGET/release/popcount-arm
VERIFY=$HARNESS_TARGET/release/popcount-verify
CHECK_PLAN=$HARNESS_TARGET/release/check-plan
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance
case "$FAMILY" in popcount) MAX_CELLS=8 ;; and-popcnt) MAX_CELLS=3 ;; esac
export RUSTUP_TOOLCHAIN=1.95 RAYON_NUM_THREADS=1 CARGO_CI_NO_SCCACHE=1
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"

log() { printf '%s\n' "$*" >> "$LAUNCH_LOG"; }

frozen_confirmation() {
  # Publication precedes confirmation: the addendum names a committed pilot
  # digest and is itself committed unchanged.
  [[ "$MODE" != confirmation ]] && return 0
  grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM" ||
    { echo 'confirmation addendum does not name a pilot receipt digest' >&2; exit 2; }
  git ls-files --error-unmatch "$ADDENDUM" >/dev/null
  git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
}

case "$ACTION" in
  prepare)
    [[ -f "$LEDGER" ]] || { echo "family ledger $LEDGER must exist before the first campaign" >&2; exit 2; }
    [[ ! -e "$STAGE" && ! -e "$PLAN" ]] || { echo "campaign $CAMPAIGN is already prepared" >&2; exit 2; }
    [[ ! -e "$OUT" ]] || { echo "receipt directory $OUT already exists" >&2; exit 2; }
    frozen_confirmation
    mkdir -p "$CAMPAIGNS"
    ./scripts/cargo-budget.sh cargo build --release --offline -p tuning-campaign-support \
      --bin benchmark-ab-runner --bin benchmark-acceptance
    CARGO_TARGET_DIR="$HARNESS_TARGET" ./scripts/cargo-budget.sh cargo build --release --offline \
      --manifest-path "$SURVEY/gf2-side/Cargo.toml"
    {
      echo "# command: $0 $*"
      echo "# prepared_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
      echo "# gf2 revision (informational): $(git rev-parse HEAD)"
      echo "# campaign: $CAMPAIGN"
      echo "# addendum: $ADDENDUM sha256=$(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
      echo "# ledger: $LEDGER ($(grep -c . "$LEDGER" || true) lines before this campaign)"
      echo "# stage: $STAGE"
      echo "# toolchain: $(rustc --version)"
      echo "# RUSTFLAGS: ${RUSTFLAGS:-<unset>}"
      echo "# arm sha256: $(sha256sum "$ARM" | cut -d' ' -f1)"
      echo "# runner sha256: $(sha256sum "$RUNNER" | cut -d' ' -f1)"
      echo "# acceptance sha256: $(sha256sum "$ACCEPTANCE" | cut -d' ' -f1)"
      echo "# external build: $("$ARM" --build-identity)"
    } > "$LAUNCH_LOG"
    python3 "$SURVEY/build-plan.py" "$FAMILY" "$ADDENDUM" "$CAMPAIGN" "$MODE" "$ARM" "$LOCK" \
      "$MAX_CELLS" "$PLAN" >> "$LAUNCH_LOG"
    "$CHECK_PLAN" "$PLAN" >> "$LAUNCH_LOG"
    log "# popcount-verify (generic matrix):"
    "$VERIFY" >> "$LAUNCH_LOG"
    log "# popcount-verify (planned cells):"
    "$VERIFY" --plan "$PLAN" >> "$LAUNCH_LOG"
    python3 "$SURVEY/observe-instructions.py" "$ARM" > "$OBSERVATION"
    log "# instruction record: $OBSERVATION sha256=$(sha256sum "$OBSERVATION" | cut -d' ' -f1)"
    cat "$LAUNCH_LOG"
    ;;
  run)
    [[ -f "$PLAN" ]] || { echo 'prepare the campaign first' >&2; exit 2; }
    frozen_confirmation
    [[ "$(sha256sum "$ARM" | cut -d' ' -f1)" == "$(grep -o 'arm sha256: [0-9a-f]*' "$LAUNCH_LOG" | cut -d' ' -f3)" ]] ||
      { echo 'the arm binary differs from the prepared one' >&2; exit 2; }
    session=0
    while :; do
      session=$((session + 1))
      log "# session start: $(date -u +%Y-%m-%dT%H:%M:%SZ) load: $(cut -d' ' -f1-3 /proc/loadavg)"
      log "# command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host $RUNNER run $STAGE $PLAN"
      echo "GF2_CAMPAIGN_EXECUTION_LOG=$STAGE/execution.log"
      set +e
      GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
        "$RUNNER" run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
      code=${PIPESTATUS[0]}
      set -e
      log "# session exit: $code at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
      case "$code" in
        0) break ;;
        3) sleep 5 ;;
        *) echo "session failed with $code; re-invoke run to resume" >&2; exit "$code" ;;
      esac
    done
    ;;
  finalize)
    [[ ! -e "$OUT" ]] || { echo "receipt directory $OUT already exists" >&2; exit 2; }
    "$RUNNER" finalize "$STAGE" "$OUT" | tee -a "$LAUNCH_LOG"
    cp "$OBSERVATION" "$OUT/instruction-observation.txt"
    cp "$LAUNCH_LOG" "$OUT/launcher.log"
    set +e
    "$ACCEPTANCE" "$OUT" | tee -a "$OUT/launcher.log"
    verdict=${PIPESTATUS[0]}
    set -e
    echo "# acceptance exit: $verdict" >> "$OUT/launcher.log"
    exit "$verdict"
    ;;
  *) echo "usage: $0 <popcount|and-popcnt> <pilot|confirmation> <prepare|run|finalize>" >&2; exit 2 ;;
esac
