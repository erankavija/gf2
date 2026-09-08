#!/usr/bin/env bash
# Historical v1 launcher: reproduce with the receipt-pinned v1 producing inputs.
# Current protocol evidence uses run-smoke-v2.sh. V1 snapshots remain unchanged.
#
# Bounded release smoke of the Zen 3 benchmark protocol pipeline (jit:f547c394).
#
# Builds the runner, the acceptance tool and the synthetic arm under
# `--release`, then runs either the exploratory pilot or the separately frozen
# confirmation as two sessions under the canonical CCX1 lock wrapper. The
# pilot must be published before its digest is placed in the confirmation
# addendum. Both receipts prove the pipeline and make no gf2 performance claim.
#
# Usage: dev/bench_results/f547c394/run-smoke.sh pilot|confirmation [date-utc]
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=f547c394
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/f547c394/addendum-protocol-smoke-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-protocol-pilot"
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/f547c394/addendum-protocol-smoke.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-protocol-confirmation"
    ;;
  *)
    echo "usage: $0 pilot|confirmation [date-utc]" >&2
    exit 2
    ;;
esac
if [[ "$MODE" == confirmation ]] && \
   ! grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM"; then
  echo 'confirmation addendum does not identify a pilot receipt digest' >&2
  exit 2
fi
if [[ -e "$OUT" ]]; then
  echo "receipt directory $OUT already exists; remove it to re-run" >&2
  exit 2
fi
# Builds finish before timed work.
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance --bin ab-smoke-workload
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
WORKLOAD=$(realpath target/release/ab-smoke-workload)
CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN" "$CAMPAIGN" "$WORKLOAD" "$LOCK" "$MODE" "$LABEL" "$ADDENDUM" <<'PY_PLAN'
import json, sys
plan_path, campaign, workload, lock, mode, label, addendum = sys.argv[1:]
def arm(passes, description):
    return {"build": "conservative-portable", "description": description, "executable": workload,
            "arguments": [], "environment": {"GF2_SMOKE_PASSES": str(passes)}, "rustflags": None,
            "tuning_profile": None}
def cell(cell_id, baseline, words, seed, pilot=None):
    return {"cell_id": cell_id, "baseline_arm": baseline, "candidate_arm": "candidate",
            "case": {"words": words, "seed": seed}, "pilot_pairs": pilot}
cells = [cell("xor-fold-double-pass-1core", "baseline-two-pass", 65536, 1),
         cell("xor-fold-identical-1core", "baseline", 65536, 2)]
if mode == "confirmation":
    cells.extend([cell("xor-fold-streaming-pilot-6core", "baseline", 4194304, 3, pilot=6),
                  cell("xor-fold-identical-12core", "baseline", 65536, 4)])
plan = {
    "schema": "zen3-benchmark-plan-v1", "campaign_id": campaign, "issue": "f547c394",
    "label": label, "campaign_seed": 20260907,
    "addendum": addendum,
    "lock_path": lock, "wrapper": "dev/scripts/ccx1-bench-flock.sh", "timing_override": None,
    "arms": {"baseline-two-pass": arm(2, "xor-fold, two passes per call"),
             "baseline": arm(1, "xor-fold, one pass per call"),
             "candidate": arm(1, "xor-fold, one pass per call")},
    "cells": cells,
    "max_cells_per_session": 1 if mode == "pilot" else 2,
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
PY_PLAN
# The launcher log stays in the stage until finalization and is then copied
# beside the receipt.
LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"
# Session 1 obeys the plan's cell limit and pauses; session 2 resumes and completes.
# The wrapper's default mode pins CCX1 (taskset -c 6-11) and holds the mutex.
set +e
GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh "$RUNNER" run "$STAGE" "$PLAN" | tee -a "$LAUNCH_LOG"
first=${PIPESTATUS[0]}
set -e
echo "# session 1 exit: $first (3 = paused after max_cells_per_session)" >>"$LAUNCH_LOG"
if [[ $first -ne 3 ]]; then
  echo "session 1 exited $first, expected 3" >&2
  exit 1
fi
GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh "$RUNNER" run "$STAGE" "$PLAN" | tee -a "$LAUNCH_LOG"
echo "# session 2 exit: 0 (complete)" >>"$LAUNCH_LOG"
"$RUNNER" finalize "$STAGE" "$OUT" | tee -a "$LAUNCH_LOG"
cp "$LAUNCH_LOG" "$OUT/launcher.log"
LAUNCH_LOG="$OUT/launcher.log"
set +e
"$ACCEPTANCE" "$OUT" | tee -a "$LAUNCH_LOG"
verdict=${PIPESTATUS[0]}
set -e
{
  echo "# acceptance exit: $verdict"
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$LAUNCH_LOG"
echo "$MODE receipt: $OUT" >&2
exit "$verdict"
