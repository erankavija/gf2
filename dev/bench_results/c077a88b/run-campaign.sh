#!/usr/bin/env bash
# One checkpointed full-host session. Repeat after exit 3; never overlap timings.
# Usage: run-campaign.sh FAMILY pilot|confirmation [run-id] prepare|run|finalize
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FAMILY=${1:?matched-algorithm or quality-compatible}
MODE=${2:?pilot or confirmation}
RUN_ID=${3:-v3-r1}
ACTION=${4:-run}
case "$FAMILY:$MODE" in
 matched-algorithm:pilot|matched-algorithm:confirmation|quality-compatible:pilot|quality-compatible:confirmation) ;;
 *) exit 2 ;;
esac
SURVEY=dev/active/c077a88b/survey
EXT=${GF2_LDPC_EXT:-/tmp/c077a88b-ext}
ARMS=$repo/target/ldpc-survey/release
QUALITY=dev/bench_results/c077a88b/v3-preparation/quality
OUT=dev/bench_results/c077a88b/$RUN_ID-c077a88b-ldpc-$FAMILY-$MODE
STAGE=$repo/target/ldpc-campaigns/$RUN_ID-$FAMILY-$MODE
PLAN=$STAGE.plan.json
LAUNCH_LOG=dev/bench_results/c077a88b/$RUN_ID-$FAMILY-$MODE-launcher.log
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance
ADDENDUM=dev/active/c077a88b/addendum-ldpc-$FAMILY
[[ "$MODE" != pilot ]] || ADDENDUM=$ADDENDUM-pilot
ADDENDUM=$ADDENDUM.json
export RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
case "$ACTION" in
 prepare)
  [[ ! -e "$PLAN" ]] || { echo "existing plan: $PLAN" >&2; exit 2; }
  mkdir -p "$(dirname "$PLAN")"
  ./scripts/cargo-budget.sh cargo +1.95 build --offline --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  python3 "$SURVEY/make-plan.py" --family "ldpc-$FAMILY" --label "$MODE" \
    --addendum "$ADDENDUM" --arms-dir "$ARMS" --bundles-dir "$EXT/inputs" \
    --quality-dir "$QUALITY" --campaign-id "c077a88b-$RUN_ID-$FAMILY-$MODE" \
    --max-cells-per-session 1 --output "$PLAN"
  ;;
 run)
  [[ -f "$PLAN" ]] || { echo 'prepare the plan and builds first' >&2; exit 2; }
  # Publication precedes confirmation; immutable snapshots bind measurement.
  if [[ "$MODE" == confirmation ]]; then
    git ls-files --error-unmatch "$ADDENDUM" >/dev/null
    git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
  fi
  printf 'start=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
  printf 'command=GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host %q run %q %q\n' "$RUNNER" "$STAGE" "$PLAN" >> "$LAUNCH_LOG"
  echo "GF2_CAMPAIGN_EXECUTION_LOG=$STAGE/execution.log"
  set +e
  CARGO_CI_NO_LOCK=1 GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
  code=${PIPESTATUS[0]}
  set -e
  printf 'terminal=%s end=%s\n' "$code" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
  exit "$code"
  ;;
 finalize)
  "$RUNNER" finalize "$STAGE" "$OUT"
  cp "$LAUNCH_LOG" "$OUT/launcher.log"
  "$ACCEPTANCE" "$OUT"
  ;;
 *) echo 'action must be prepare, run, or finalize' >&2; exit 2 ;;
esac
