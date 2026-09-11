#!/usr/bin/env bash
# Steady-state LDPC throughput campaigns under protocol v3 (jit:3be770d5).
#
# Usage (from the worktree root):
#   run-campaign.sh FAMILY MODE RUN_ID ACTION
#     FAMILY  single-worker | multicore | fastest-compatible
#     MODE    pilot | confirmation
#     ACTION  prepare   build the runner and write the plan; times nothing
#             run       bounded full-host sessions until the campaign completes
#             finalize  assemble the receipt and run independent acceptance
#             window    run, then finalize: the benchmark-window command
#
# `prepare` refuses an existing plan and checks that the arm executables are
# the ones the preparation build identity records. `run` prints the
# authoritative execution log before the first session, then invokes the
# shared runner inside one `dev/scripts/ccx1-bench-flock.sh --full-host`
# acquisition per bounded session. Exit 3 means the session checkpointed its
# cells and paused; the next session resumes under the same identity without
# repeating a completed cell. Any other nonzero exit stops the launcher with
# that code; the failed attempt stays in the stage and the family ledger. A
# repeated `window` resumes an unfinished campaign and skips a finalized one.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FAMILY=${1:?single-worker, multicore or fastest-compatible}
MODE=${2:?pilot or confirmation}
RUN_ID=${3:?run id, for example v3-r1}
ACTION=${4:?prepare, run, finalize or window}
case "$FAMILY" in
 single-worker|multicore) FAMILY_ID=ldpc-steady-matched-$FAMILY-v1 ;;
 fastest-compatible) FAMILY_ID=ldpc-steady-fastest-compatible-v1 ;;
 *) echo "unknown family $FAMILY" >&2; exit 2 ;;
esac
case "$MODE" in pilot|confirmation) ;; *) echo "unknown mode $MODE" >&2; exit 2 ;; esac
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
SURVEY=dev/active/3be770d5/survey
RESULTS=dev/bench_results/3be770d5
ADDENDUM=dev/active/3be770d5/addendum-${FAMILY_ID%-v1}
[[ "$MODE" != pilot ]] || ADDENDUM=$ADDENDUM-pilot
ADDENDUM=$ADDENDUM.json
ARMS=$repo/target/ldpc-throughput/release
BUNDLES=$repo/target/ldpc-inputs
QUALITY=$repo/dev/bench_results/c077a88b/v3-preparation/quality
IDENTITY=$RESULTS/preparation/build-identity.json
STAGE=$repo/target/ldpc-steady-campaigns/$RUN_ID-$FAMILY-$MODE
PLAN=$STAGE.plan.json
OUT=$RESULTS/$RUN_ID-3be770d5-ldpc-steady-$FAMILY-$MODE
LAUNCH_LOG=$RESULTS/$RUN_ID-steady-$FAMILY-$MODE-launcher.log
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance

check_arms() {
  for arm in gf2-throughput-arm aff3ct-throughput-arm; do
    recorded=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executables"][sys.argv[2]])' "$IDENTITY" "$arm")
    [[ "$(sha256sum "$ARMS/$arm" | cut -d' ' -f1)" == "$recorded" ]] \
      || { echo "$arm differs from $IDENTITY" >&2; exit 2; }
  done
}

run_sessions() {
  [[ -f "$PLAN" ]] || { echo 'prepare the plan and builds first' >&2; exit 2; }
  check_arms
  # Publication precedes confirmation; immutable snapshots bind measurement.
  if [[ "$MODE" == confirmation ]]; then
    git ls-files --error-unmatch "$ADDENDUM" >/dev/null
    git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
  fi
  echo "GF2_CAMPAIGN_EXECUTION_LOG=$STAGE/execution.log"
  while true; do
    printf 'start=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
    printf 'command=GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host %q run %q %q\n' "$RUNNER" "$STAGE" "$PLAN" >> "$LAUNCH_LOG"
    set +e
    CARGO_CI_NO_LOCK=1 GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
      "$RUNNER" run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
    code=${PIPESTATUS[0]}
    set -e
    printf 'terminal=%s end=%s\n' "$code" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
    [[ "$code" == 3 ]] || return "$code"
  done
}

finalize() {
  if [[ -f "$OUT/acceptance-summary.json" ]]; then
    echo "$OUT is finalized" >&2
    return 0
  fi
  "$RUNNER" finalize "$STAGE" "$OUT"
  cp "$LAUNCH_LOG" "$OUT/launcher.log"
  "$ACCEPTANCE" "$OUT"
}

case "$ACTION" in
 prepare)
  [[ ! -e "$PLAN" ]] || { echo "existing plan: $PLAN" >&2; exit 2; }
  check_arms
  mkdir -p "$(dirname "$PLAN")"
  ./scripts/cargo-budget.sh cargo +1.95 build --offline --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  python3 "$SURVEY/make-plan.py" --family "$FAMILY_ID" --label "$MODE" \
    --addendum "$ADDENDUM" --arms-dir "$ARMS" --bundles-dir "$BUNDLES" \
    --quality-dir "$QUALITY" --campaign-id "3be770d5-$RUN_ID-steady-$FAMILY-$MODE" \
    --max-cells-per-session 1 --output "$PLAN"
  ;;
 run) run_sessions ;;
 finalize) finalize ;;
 window)
  if [[ -f "$OUT/acceptance-summary.json" ]]; then
    echo "$OUT is finalized" >&2
    exit 0
  fi
  run_sessions
  finalize
  ;;
 *) echo 'action must be prepare, run, finalize or window' >&2; exit 2 ;;
esac
