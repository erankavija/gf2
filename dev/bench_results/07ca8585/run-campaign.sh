#!/usr/bin/env bash
# Min-sum update before/after, comparator and REQ-10 granularity campaigns (jit:07ca8585).
#
# Each campaign is evaluated under the protocol version its addendum names and
# its receipt pins and snapshots that version. The launcher adds no numeric
# setting: every one comes from the committed addendum and the shared protocol.
#
# Usage (from the worktree root):
#   run-campaign.sh FAMILY MODE RUN_ID ACTION
#     FAMILY  single-worker | multicore
#             | comparator-single-worker | comparator-multicore
#             | checknode | fixed-iteration
#     MODE    pilot | confirmation
#     ACTION  prepare   build nothing, write the plan; times nothing
#             run       bounded full-host sessions until the campaign completes
#             finalize  assemble the receipt and run independent acceptance
#             window    run, then finalize: the benchmark-window command
#
# `prepare` refuses an existing plan and checks that both arm executables are
# the ones the preparation build identity records. `run` prints the
# authoritative execution log before the first session, then invokes the shared
# runner inside one `dev/scripts/ccx1-bench-flock.sh --full-host` acquisition
# per bounded session. Exit 3 means the session checkpointed its cells and
# paused; the next session resumes under the same identity without repeating a
# completed cell. Any other nonzero exit stops the launcher with that code; the
# failed attempt stays in the stage and the family ledger. A repeated `window`
# resumes an unfinished campaign and skips a finalized one.
#
# The two gf2 generations are the same harness built from two trees, and the two
# isolated check-node arms are a third build in this issue's own arms workspace.
# Rebuilding any of them is outside this script, because a rebuilt executable is
# a different candidate identity: see the two preparation build identities for
# the commands and the digests a campaign requires.
#
# `checknode` is REQ-10's kernel granularity and `fixed-iteration` its
# full-iteration granularity. The first runs the arms of the kernel identity and
# reads no prepared quality; the second runs the pinned throughput arms against
# the fixed-stopping prepared quality corpus this issue committed, because the
# reused c077a88b corpus was produced under syndrome stopping and the arms refuse
# a corpus whose settings differ from theirs.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
FAMILY=${1:?single-worker, multicore, comparator-single-worker or comparator-multicore}
MODE=${2:?pilot or confirmation}
RUN_ID=${3:?run id, for example v4-r1}
ACTION=${4:?prepare, run, finalize or window}
case "$FAMILY" in
 single-worker|multicore|comparator-single-worker|comparator-multicore|checknode|fixed-iteration)
  FAMILY_ID=ldpc-update-$FAMILY-v1 ;;
 *) echo "unknown family $FAMILY" >&2; exit 2 ;;
esac
case "$MODE" in pilot|confirmation) ;; *) echo "unknown mode $MODE" >&2; exit 2 ;; esac
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
SURVEY=dev/active/07ca8585/survey
RESULTS=dev/bench_results/07ca8585
ADDENDUM=dev/active/07ca8585/addendum-${FAMILY_ID%-v1}
[[ "$MODE" != pilot ]] || ADDENDUM=$ADDENDUM-pilot
ADDENDUM=$ADDENDUM.json
AFTER=$repo/target/ldpc-throughput/release
BEFORE=$repo/target/ldpc-throughput-before/release
KERNEL=$repo/target/ldpc-update-arms/release
BUNDLES=$repo/target/ldpc-inputs
QUALITY=$repo/dev/bench_results/c077a88b/v3-preparation/quality
IDENTITY=$RESULTS/preparation/build-identity.json
KERNEL_IDENTITY=$RESULTS/preparation/kernel-build-identity.json
PRODUCING=dev/active/07ca8585/survey/producing-inputs.json
# The full-iteration cells decode against this issue's own corpus, and the
# kernel cells read none: an isolated check-node pass decodes no frame.
if [[ "$FAMILY" == fixed-iteration ]]; then
  QUALITY=$repo/$RESULTS/preparation/quality-fixed
fi
if [[ "$FAMILY" == checknode || "$FAMILY" == fixed-iteration ]]; then
  PRODUCING=dev/active/07ca8585/survey/producing-inputs-kernel.json
fi
STAGE=$repo/target/ldpc-update-campaigns/$RUN_ID-$FAMILY-$MODE
PLAN=$STAGE.plan.json
OUT=$RESULTS/$RUN_ID-07ca8585-ldpc-update-$FAMILY-$MODE
LAUNCH_LOG=$RESULTS/$RUN_ID-update-$FAMILY-$MODE-launcher.log
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance

# Every executable of the kernel identity, checked when a kernel arm runs.
check_kernel_arms() {
  while read -r arm; do
    recorded=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["kernel"]["executables"][sys.argv[2]])' \
      "$KERNEL_IDENTITY" "$arm")
    [[ "$(sha256sum "$KERNEL/$arm" | cut -d' ' -f1)" == "$recorded" ]] \
      || { echo "kernel/$arm differs from $KERNEL_IDENTITY" >&2; exit 2; }
  done < <(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["kernel"]["executables"]))' \
    "$KERNEL_IDENTITY")
}

# The fixed-stopping prepared quality corpus, checked when the arms read it.
check_fixed_quality() {
  while read -r path; do
    recorded=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["prepared_quality_fixed"]["files"][sys.argv[2]])' \
      "$KERNEL_IDENTITY" "$path")
    [[ "$(sha256sum "$repo/$path" | cut -d' ' -f1)" == "$recorded" ]] \
      || { echo "$path differs from $KERNEL_IDENTITY" >&2; exit 2; }
  done < <(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["prepared_quality_fixed"]["files"]))' \
    "$KERNEL_IDENTITY")
}

check_arms() {
  if [[ "$FAMILY" == checknode ]]; then check_kernel_arms; return; fi
  [[ "$FAMILY" != fixed-iteration ]] || check_fixed_quality
  for generation in before after; do
    if [[ "$generation" == before ]]; then dir=$BEFORE; else dir=$AFTER; fi
    while read -r arm; do
      recorded=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["generations"][sys.argv[2]]["executables"][sys.argv[3]])' \
        "$IDENTITY" "$generation" "$arm")
      [[ "$(sha256sum "$dir/$arm" | cut -d' ' -f1)" == "$recorded" ]] \
        || { echo "$generation/$arm differs from $IDENTITY" >&2; exit 2; }
    done < <(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["generations"][sys.argv[2]]["executables"]))' \
      "$IDENTITY" "$generation")
  done
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
    --addendum "$ADDENDUM" --before-dir "$BEFORE" --after-dir "$AFTER" \
    --bundles-dir "$BUNDLES" --quality-dir "$QUALITY" --kernel-dir "$KERNEL" \
    --producing-manifest "$PRODUCING" \
    --campaign-id "07ca8585-$RUN_ID-update-$FAMILY-$MODE" \
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
