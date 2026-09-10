#!/usr/bin/env bash
# Bounded release A/B of the YMM raw-batch dispatch repair (jit:1d0da41f).
#
# Builds the protocol runner, the acceptance tool and both arms under
# `--release`, then runs the exploratory pilot or the separately frozen
# confirmation as bounded sessions under the canonical CCX1 lock wrapper. The
# pilot is published before its digest enters the confirmation addendum.
#
# The baseline arm links a pinned snapshot of the pre-change crates and the
# candidate arm links this checkout, so the receipt compares the dispatch
# repair against the code it replaces on the same host in the same session.
#
# Usage: dev/bench_results/1d0da41f/run-clmul-ab.sh pilot|confirmation [date-utc]
#
# Environment:
#   GF2_BASELINE_TREE  required; the pre-change source tree the baseline links.
#   GF2_ARM_STAGE      arm staging root (default: ${TMPDIR:-/tmp}/gf2-1d0da41f-arms).
set -euo pipefail
export RUSTUP_TOOLCHAIN=1.95.0
export CARGO_CI_NO_SCCACHE=1
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=1d0da41f
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/$ISSUE/addendum-ymm-clmul-dispatch-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-clmul-dispatch-pilot"
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/$ISSUE/addendum-ymm-clmul-dispatch.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-clmul-dispatch-confirmation"
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
  --bin benchmark-ab-runner --bin benchmark-acceptance
STAGE_ARMS=${GF2_ARM_STAGE:-${TMPDIR:-/tmp}/gf2-1d0da41f-arms}
GF2_ARM_STAGE="$STAGE_ARMS" ./dev/active/$ISSUE/survey/build-arms.sh
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
CANDIDATE=$(realpath "$STAGE_ARMS/target-candidate/release/ymm-clmul-arm")
BASELINE=$(realpath "$STAGE_ARMS/target-baseline/release/ymm-clmul-arm")

CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN" "$CAMPAIGN" "$BASELINE" "$CANDIDATE" "$LOCK" "$MODE" "$LABEL" "$ADDENDUM" <<'PY_PLAN'
import json, sys
plan_path, campaign, baseline, candidate, lock, mode, label, addendum = sys.argv[1:]
def arm(executable, description):
    return {"build": "conservative-portable", "description": description,
            "executable": executable, "arguments": [], "environment": {},
            "rustflags": None, "tuning_profile": None}
def cell(cell_id, workload, elements, degree, seed, pilot=None):
    return {"cell_id": cell_id, "baseline_arm": "baseline", "candidate_arm": "candidate",
            "case": {"degree": degree, "elements": elements, "seed": seed, "workload": workload},
            "pilot_pairs": pilot}
pilot_pairs = 6 if mode == "pilot" else None
cells = [
    cell("raw-batch-small-8-1core", "raw-batch", 8, 64, 101, pilot_pairs),
    cell("raw-batch-odd-tail-65-1core", "raw-batch", 65, 64, 102, pilot_pairs),
    cell("raw-batch-l1-512-1core", "raw-batch", 512, 64, 103, pilot_pairs),
    cell("fieldvec-dot-1024-1core", "fieldvec-dot", 1024, 8, 104, pilot_pairs),
]
if mode == "confirmation":
    cells.append(cell("raw-batch-l1-512-streaming-6core", "raw-batch", 512, 64, 105))
plan = {
    "schema": "zen3-benchmark-plan-v1", "campaign_id": campaign, "issue": "1d0da41f",
    "label": label, "campaign_seed": 20260907,
    "addendum": addendum,
    "lock_path": lock, "wrapper": "dev/scripts/ccx1-bench-flock.sh", "timing_override": None,
    "arms": {
        "baseline": arm(baseline, "pre-change crates: raw batch dispatch requires AVX512VL"),
        "candidate": arm(candidate, "repaired crates: raw batch dispatch requires AVX2+VPCLMULQDQ"),
    },
    "cells": cells,
    "max_cells_per_session": 2,
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
PY_PLAN
LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# baseline arm: $BASELINE"
  echo "# candidate arm: $CANDIDATE"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"
# Bounded sessions: the runner pauses with exit 3 after max_cells_per_session
# and resumes from its checkpoints, so the CCX1 mutex is released between them.
session=0
while true; do
  session=$((session + 1))
  set +e
  GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host "$RUNNER" run "$STAGE" "$PLAN" dev/active/1d0da41f/producing-inputs.json | tee -a "$LAUNCH_LOG"
  status=${PIPESTATUS[0]}
  set -e
  echo "# session $session exit: $status (3 = paused after max_cells_per_session)" >>"$LAUNCH_LOG"
  case "$status" in
    0) break ;;
    3) continue ;;
    *) echo "session $session exited $status" >&2; exit 1 ;;
  esac
done
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
