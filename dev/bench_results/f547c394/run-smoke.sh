#!/usr/bin/env bash
# Bounded release smoke of the Zen 3 benchmark protocol pipeline (jit:f547c394).
#
# Builds the runner, the acceptance tool and the synthetic arm under
# `--release`, then runs the frozen smoke family (dev/active/f547c394/
# addendum-protocol-smoke.json) as two sessions under the canonical CCX1 lock
# wrapper so the receipt exercises checkpoint/resume, finalizes the receipt
# directory and evaluates it. The receipt is labelled `smoke`: it proves the
# pipeline and claims no performance result.
#
# Usage (from the repository root, clean tree): dev/bench_results/f547c394/run-smoke.sh [date-utc]
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=f547c394
DATE_UTC=${1:-$(date -u +%Y-%m-%d)}
OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-smoke"
if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
  echo 'the smoke requires a clean source tree' >&2
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
CAMPAIGN="smoke-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN" "$CAMPAIGN" "$WORKLOAD" "$LOCK" <<'PY_PLAN'
import json, sys
plan_path, campaign, workload, lock = sys.argv[1:]
def arm(passes, description):
    return {"build": "conservative-portable", "description": description, "executable": workload,
            "arguments": [], "environment": {"GF2_SMOKE_PASSES": str(passes)}, "rustflags": None,
            "tuning_profile": None}
def cell(cell_id, baseline, words, seed, pilot=None):
    return {"cell_id": cell_id, "baseline_arm": baseline, "candidate_arm": "candidate",
            "case": {"words": words, "seed": seed}, "pilot_pairs": pilot}
plan = {
    "schema": "zen3-benchmark-plan-v1", "campaign_id": campaign, "issue": "f547c394",
    "label": "smoke", "campaign_seed": 20260906,
    "addendum": "dev/active/f547c394/addendum-protocol-smoke.json",
    "lock_path": lock, "wrapper": "dev/scripts/ccx1-bench-flock.sh", "timing_override": None,
    "arms": {"baseline-two-pass": arm(2, "xor-fold, two passes per call"),
             "baseline": arm(1, "xor-fold, one pass per call"),
             "candidate": arm(1, "xor-fold, one pass per call")},
    "cells": [cell("xor-fold-double-pass-1core", "baseline-two-pass", 65536, 1),
              cell("xor-fold-identical-1core", "baseline", 65536, 2),
              cell("xor-fold-streaming-pilot-6core", "baseline", 4194304, 3, pilot=6),
              cell("xor-fold-identical-12core", "baseline", 65536, 4)],
    "max_cells_per_session": 2,
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
PY_PLAN
# The launcher log stays outside the repository while sessions run, because the
# runner refuses a dirty source tree; it is copied beside the receipt afterwards.
LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision: $(git rev-parse HEAD)"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"
# Session 1 measures two cells and pauses; session 2 resumes and completes.
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
"$ACCEPTANCE" "$OUT" --repo "$repo" | tee -a "$LAUNCH_LOG"
verdict=${PIPESTATUS[0]}
set -e
{
  echo "# acceptance exit: $verdict"
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$LAUNCH_LOG"
echo "smoke receipt: $OUT" >&2
exit "$verdict"
