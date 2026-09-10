#!/usr/bin/env bash
# Protocol-v3 polynomial-multiplication baseline campaigns (jit:c7113c5a).
#
# Usage (from the repository root):
#   dev/bench_results/c7113c5a/run-polynomial-v3.sh build
#   dev/bench_results/c7113c5a/run-polynomial-v3.sh plan|run|finalize CAMPAIGN
#
# CAMPAIGN is one of baselines-pilot, host-targeting-pilot,
# baselines-confirmation or host-targeting-confirmation.
#
# `build` fetches and builds gf2x and the arm executables, runs the
# correctness validation and records the build evidence, then builds the
# protocol runner and acceptance tool; every step runs under the CPU budget
# and finishes before any timed work. `plan` projects the saved runner plan
# from the frozen family addendum once. `run` measures the plan as bounded
# checkpointed sessions under the canonical CCX1 mutex and resumes an
# interrupted campaign under the same identity without repeating completed
# cells. `finalize` assembles the receipt directory and evaluates it.
#
# Every numeric setting comes from the addendum and the protocol's frozen
# shared settings; this script fixes only campaign identities, seeds and the
# session budget.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=c7113c5a
SURVEY=dev/active/$ISSUE/survey
EXT=${GF2_SURVEY_EXT:-$repo/target/$ISSUE-ext}
STAGES=$repo/target/$ISSUE-campaigns
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
export RUSTUP_TOOLCHAIN=1.95 RAYON_NUM_THREADS=1

ACTION=${1:?build, plan, run or finalize}
if [[ "$ACTION" == build ]]; then
  "$SURVEY/fetch-build.sh" "$EXT"
  "$SURVEY/run-validation.sh" "$SURVEY/validation-v3.json" "$EXT"
  "$SURVEY/record-build-evidence.sh" "$SURVEY/gf2x-build-evidence-v3.txt" "$EXT"
  ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  exit 0
fi

CAMPAIGN=${2:?campaign}
case "$CAMPAIGN" in
  baselines-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-baselines-pilot.json
    LABEL=pilot SEED=20260910301 MAX_CELLS=4 PILOT_PAIRS=12 ;;
  host-targeting-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-host-targeting-pilot.json
    LABEL=pilot SEED=20260910311 MAX_CELLS=4 PILOT_PAIRS=12 ;;
  baselines-confirmation)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-baselines-confirmation.json
    LABEL=confirmation SEED=20260910401 MAX_CELLS=3 PILOT_PAIRS= ;;
  host-targeting-confirmation)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-host-targeting-confirmation.json
    LABEL=confirmation SEED=20260910411 MAX_CELLS=2 PILOT_PAIRS= ;;
  *) echo "unknown campaign $CAMPAIGN" >&2; exit 2 ;;
esac
ID=$ISSUE-v3-r1-$CAMPAIGN
STAGE=$STAGES/$ID
PLAN=$STAGE.plan.json
LAUNCH_LOG=$STAGE.launcher.log
OUT=dev/bench_results/$ISSUE/v3-r1-$CAMPAIGN

case "$ACTION" in
  plan)
    [[ ! -e "$PLAN" ]] || { echo "existing plan: $PLAN" >&2; exit 2; }
    # Publication precedes measurement: the frozen addendum is committed and
    # unmodified, so the receipt's addendum pin names committed bytes.
    git ls-files --error-unmatch "$ADDENDUM" >/dev/null
    git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
    mkdir -p "$STAGES"
    touch "$LOCK"
    python3 "$SURVEY/make-plan.py" --addendum "$ADDENDUM" --label "$LABEL" \
      --campaign-id "$ID" --campaign-seed "$SEED" --ext "$EXT" \
      --lock "$(realpath "$LOCK")" --max-cells-per-session "$MAX_CELLS" \
      ${PILOT_PAIRS:+--pilot-pairs "$PILOT_PAIRS"} --output "$PLAN"
    ;;
  run)
    [[ -f "$PLAN" ]] || { echo "plan the campaign first: $PLAN" >&2; exit 2; }
    {
      echo "# command: $0 $*"
      echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
      echo "# gf2 revision (informational): $(git rev-parse HEAD)"
      echo "# addendum: $ADDENDUM sha256 $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
      echo "# plan: $PLAN sha256 $(sha256sum "$PLAN" | cut -d' ' -f1)"
      echo "# toolchain: $(rustc --version)"
      echo "# load_avg_start: $(cut -d' ' -f1-3 /proc/loadavg)"
    } >>"$LAUNCH_LOG"
    # Exit 3 pauses at the session cell budget and releases the mutex so a
    # queued sibling gets the host; exit 0 completes the campaign.
    session=0
    while :; do
      session=$((session + 1))
      echo "# session $session command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host $RUNNER run $STAGE $PLAN" >>"$LAUNCH_LOG"
      set +e
      GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
        "$RUNNER" run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
      code=${PIPESTATUS[0]}
      set -e
      echo "# session $session exit: $code at $(date -u +%Y-%m-%dT%H:%M:%SZ) load $(cut -d' ' -f1-3 /proc/loadavg)" >>"$LAUNCH_LOG"
      case "$code" in
        0) break ;;
        3) ;;
        *) exit "$code" ;;
      esac
    done
    ;;
  finalize)
    "$RUNNER" finalize "$STAGE" "$OUT" | tee -a "$LAUNCH_LOG"
    cp "$LAUNCH_LOG" "$OUT/launcher.log"
    "$ACCEPTANCE" "$OUT"
    ;;
  *) echo 'action must be build, plan, run or finalize' >&2; exit 2 ;;
esac
