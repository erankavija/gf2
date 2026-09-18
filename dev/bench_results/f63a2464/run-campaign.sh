#!/usr/bin/env bash
# QC-aware intra-frame candidate campaigns (jit:f63a2464).
#
# Each campaign is evaluated under the protocol version its addendum names and
# its receipt pins and snapshots that version. The launcher adds no numeric
# setting: every one comes from the committed addendum and the shared protocol.
#
# Usage (from the worktree root):
#   run-campaign.sh FAMILY MODE RUN_ID ACTION
#     FAMILY  intra-frame-single-worker | intra-frame-multicore
#             | comparator-single-worker
#     MODE    pilot | confirmation
#     ACTION  prepare   build nothing measured, write the plan; times nothing
#             run       bounded full-host sessions until the campaign completes
#             finalize  assemble the receipt and run independent acceptance
#             window    run, then finalize: the benchmark-window command
#
# `prepare` refuses an existing plan and checks that every arm executable is the
# one the preparation build identity records. `run` prints the authoritative
# execution log before the first session, then invokes the shared runner inside
# one `dev/scripts/ccx1-bench-flock.sh --full-host` acquisition per bounded
# session. Exit 3 means the session checkpointed its cells and paused; the next
# session resumes under the same identity without repeating a completed cell.
# Any other nonzero exit stops the launcher with that code; the failed attempt
# stays in the stage and the family ledger. A repeated `window` resumes an
# unfinished campaign and skips a finalized one.
#
# The arms are two builds: the `3be770d5` steady-state harness, which supplies
# the canonical gf2 arm and the pinned AFF3CT arm, and this issue's arms
# workspace, which supplies the candidate. Rebuilding either is outside this
# script, because a rebuilt executable is a different candidate identity: the
# preparation build identity records the commands and the digests a campaign
# requires.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FAMILY=${1:?intra-frame-single-worker, intra-frame-multicore or comparator-single-worker}
MODE=${2:?pilot or confirmation}
RUN_ID=${3:?run id, for example v4-r1}
ACTION=${4:?prepare, run, finalize or window}
case "$FAMILY" in
 intra-frame-single-worker|intra-frame-multicore|comparator-single-worker)
  FAMILY_ID=ldpc-qc-$FAMILY-v1 ;;
 *) echo "unknown family $FAMILY" >&2; exit 2 ;;
esac
case "$MODE" in pilot|confirmation) ;; *) echo "unknown mode $MODE" >&2; exit 2 ;; esac
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
SURVEY=dev/active/f63a2464/survey
RESULTS=dev/bench_results/f63a2464
ADDENDUM=dev/active/f63a2464/addendum-${FAMILY_ID%-v1}
[[ "$MODE" != pilot ]] || ADDENDUM=$ADDENDUM-pilot
ADDENDUM=$ADDENDUM.json
BASELINE=$repo/target/ldpc-qc-baseline/release
CANDIDATE=$repo/target/ldpc-qc-arms/release
BUNDLES=$repo/target/ldpc-inputs
QUALITY=$repo/dev/bench_results/c077a88b/v3-preparation/quality
IDENTITY=$RESULTS/preparation/build-identity.json
PRODUCING=dev/active/f63a2464/survey/producing-inputs.json
STAGE=$repo/target/ldpc-qc-campaigns/$RUN_ID-$FAMILY-$MODE
PLAN=$STAGE.plan.json
OUT=$RESULTS/$RUN_ID-f63a2464-ldpc-qc-$FAMILY-$MODE
LAUNCH_LOG=$RESULTS/$RUN_ID-qc-$FAMILY-$MODE-launcher.log
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance

# Every executable a campaign may launch, checked against the recorded digests.
check_arms() {
  while read -r generation arm; do
    dir=$BASELINE
    [[ "$generation" != candidate ]] || dir=$CANDIDATE
    recorded=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["executables"][sys.argv[2]]["sha256"])' \
      "$IDENTITY" "$arm")
    [[ "$(sha256sum "$dir/$arm" | cut -d' ' -f1)" == "$recorded" ]] \
      || { echo "$generation/$arm differs from $IDENTITY" >&2; exit 2; }
  done < <(python3 -c 'import json,sys
identity = json.load(open(sys.argv[1]))["executables"]
for name, entry in identity.items():
    print(entry["generation"], name)' "$IDENTITY")
}

run_sessions() {
  [[ -f "$PLAN" ]] || { echo 'prepare the plan first' >&2; exit 2; }
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
  # Family ledger genesis: created empty once, never rewritten. `noclobber`
  # makes a second prepare of the same family leave the existing chain alone.
  LEDGER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["family_wise"]["ledger_path"])' "$ADDENDUM")
  [[ -f "$LEDGER" ]] || (set -o noclobber; : > "$LEDGER")
  ./scripts/cargo-budget.sh cargo +1.95 build --offline --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  python3 "$SURVEY/make-plan.py" --family "$FAMILY_ID" --label "$MODE" \
    --addendum "$ADDENDUM" --baseline-dir "$BASELINE" --candidate-dir "$CANDIDATE" \
    --bundles-dir "$BUNDLES" --quality-dir "$QUALITY" \
    --producing-manifest "$PRODUCING" \
    --campaign-id "f63a2464-$RUN_ID-qc-$FAMILY-$MODE" \
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
