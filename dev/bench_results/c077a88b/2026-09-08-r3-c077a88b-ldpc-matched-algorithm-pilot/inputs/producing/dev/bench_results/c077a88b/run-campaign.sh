#!/usr/bin/env bash
# One bounded session; rerun the identical command after exit 3 to resume.
# Build and prepare sources/quality before running. Never simulate BFER here.
# Usage: run-campaign.sh FAMILY pilot|confirmation [date] [prepare|run|finalize]
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FAMILY=${1:?matched-algorithm or quality-compatible}
MODE=${2:?pilot or confirmation}
DATE_UTC=${3:-$(date -u +%Y-%m-%d)}
ACTION=${4:-run}
case "$FAMILY:$MODE" in
  matched-algorithm:pilot|matched-algorithm:confirmation|quality-compatible:pilot|quality-compatible:confirmation) ;;
  *) exit 2 ;;
esac
SURVEY=dev/active/c077a88b/survey
EXT=${GF2_LDPC_EXT:-/tmp/c077a88b-ext}
ARMS=${GF2_LDPC_ARMS:-/tmp/c077a88b-build/release}
QUALITY=dev/bench_results/c077a88b/2026-09-08-c077a88b-preparation/quality
OUT=dev/bench_results/c077a88b/$DATE_UTC-c077a88b-ldpc-$FAMILY-$MODE
STAGE=/tmp/c077a88b-$DATE_UTC-$FAMILY-$MODE
PLAN=$STAGE.plan.json
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance
ADDENDUM=dev/active/c077a88b/addendum-ldpc-$FAMILY
[[ "$MODE" != pilot ]] || ADDENDUM=$ADDENDUM-pilot
ADDENDUM=$ADDENDUM.json
export GF2_BENCH_PRODUCING_MANIFEST=dev/active/c077a88b/survey/producing-inputs.json
export RAYON_NUM_THREADS=1
case "$ACTION" in
 prepare)
  [[ ! -e "$PLAN" ]] || { echo "existing plan: $PLAN" >&2; exit 2; }
  ./scripts/cargo-budget.sh cargo build --offline --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  python3 "$SURVEY/make-plan.py" --family "ldpc-$FAMILY" --label "$MODE" \
    --addendum "$ADDENDUM" --arms-dir "$ARMS" --bundles-dir "$EXT/inputs" \
    --quality-dir "$QUALITY" --campaign-id "c077a88b-$DATE_UTC-$FAMILY-$MODE" \
    --max-cells-per-session 1 --output "$PLAN"
  ;;
 run)
  [[ -f "$PLAN" ]] || { echo 'prepare the plan and builds first' >&2; exit 2; }
  # Commitment is an assignment publication step, not provenance validation.
  # The immutable snapshots in campaign-start determine evidence validity.
  if [[ "$MODE" == confirmation ]]; then
    git ls-files --error-unmatch "$ADDENDUM" >/dev/null
    git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
  fi
  echo "GF2_CAMPAIGN_EXECUTION_LOG=$STAGE/execution.log"
  CARGO_CI_NO_LOCK=1 GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN"
  ;;
 finalize)
  "$RUNNER" finalize "$STAGE" "$OUT"
  "$ACCEPTANCE" "$OUT"
  ;;
 *) echo 'action must be prepare, run, or finalize' >&2; exit 2 ;;
esac
