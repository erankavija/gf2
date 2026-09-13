#!/usr/bin/env bash
# Produce one 53c5a8c0 campaign receipt (jit:53c5a8c0).
#
# Usage:
#   ./run-crossover.sh crossover|polynomial pilot|confirmation [date-utc] [suffix]
#   ./run-crossover.sh build
#   ./run-crossover.sh smoke <family>
#
# The two families answer two questions and keep two independent ledgers:
#
#   crossover   where the batched carry-less paths overtake the per-element
#               paths a consumer can call instead, across the frozen size grid.
#               Both arms are one gf2 executable selecting its entry point from
#               GF2_CROSSOVER_PATH.
#   polynomial  how the current gf2 long product and the whole-consumer wide
#               field product compare with the pinned gf2x build, and where
#               that comparison changes direction. gf2 is the baseline and
#               gf2x the candidate, so a speedup below one means gf2 is faster.
#
# `pilot` measures every cell exploratorily and fixes the measurement
# resolution. `confirmation` refuses to run until
# dev/active/c7113c5a/survey/freeze-confirmation.py has derived the
# confirmation addendum from the committed pilot receipt, stamping that
# resolution and the receipt's digest: the confirmatory family's thresholds are
# frozen before its first trial, never after a result.
#
# `build` checks the correctness evidence and builds the runner, the acceptance
# tool and the arm executables, then stops: it proves the pre-timing pipeline
# runs to completion under a minimal environment without starting a campaign.
# `smoke` carries every cell of a family through the real runner on a throwaway
# plan under target/, at the protocol's minimum pilot pair count, so the wire
# contract is established by running it rather than by reading code; it writes
# nothing under dev/bench_results.
#
# The launcher builds everything before timed work, then runs bounded resumable
# sessions under the canonical exclusive wrapper. Re-invoking the same mode
# resumes the same campaign: the stage directory and plan are derived from the
# mode, so a paused or interrupted session continues under its own identity
# instead of starting a second campaign.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
# A minimal environment (a systemd unit with no login shell) carries no
# ~/.cargo/bin and no pinned toolchain; every other tool this launcher or its
# helpers call resolves from /usr/bin.
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
ISSUE=53c5a8c0
ACTIVE="dev/active/$ISSUE"
SURVEY="$ACTIVE/survey"
FAMILY=${1:-}
MODE=${2:-}
DATE_UTC=${3:-$(date -u +%Y-%m-%d)}
# A suffix separates receipts of the same family, label and date. A superseded
# receipt stays committed under its own directory rather than being replaced.
SUFFIX=${4:-}

case "$FAMILY" in
  build)
    MODE=build
    ;;
  smoke)
    FAMILY=${2:-}
    MODE=smoke
    ;;
  crossover|polynomial)
    ;;
  *)
    echo "usage: $0 crossover|polynomial pilot|confirmation [date-utc] [suffix] | build | smoke <family>" >&2
    exit 2
    ;;
esac

case "$FAMILY" in
  crossover)
    FAMILY_ID=gf2m-clmul-crossover
    VARIANT=conservative
    PILOT_SEED=20260913101
    CONFIRM_SEED=20260913201
    HOLDOUT="$SURVEY/holdout-cells.json"
    ;;
  polynomial)
    FAMILY_ID=wide-polynomial-competitiveness
    VARIANT=native
    PILOT_SEED=20260913301
    CONFIRM_SEED=20260913401
    HOLDOUT=""
    ;;
  build)
    ;;
  *)
    echo "family must be crossover or polynomial" >&2
    exit 2
    ;;
esac

case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM="$ACTIVE/addendum-v4-$FAMILY-pilot.json"
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-$FAMILY-pilot${SUFFIX}"
    SEED=$PILOT_SEED
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM="$ACTIVE/addendum-v4-$FAMILY-confirmation.json"
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-$FAMILY-confirmation${SUFFIX}"
    SEED=$CONFIRM_SEED
    ;;
  smoke)
    LABEL=pilot
    ADDENDUM="$ACTIVE/addendum-v4-$FAMILY-pilot.json"
    OUT="target/bench-smoke/$ISSUE-$FAMILY"
    SEED=$PILOT_SEED
    ;;
  build) ;;
  *)
    echo "mode must be pilot, confirmation, smoke or build" >&2
    exit 2
    ;;
esac

if [[ "$MODE" == confirmation ]]; then
  if [[ ! -f "$ADDENDUM" ]]; then
    echo "$ADDENDUM does not exist; derive it from the committed pilot receipt" >&2
    echo "with dev/active/c7113c5a/survey/freeze-confirmation.py, which reads" >&2
    echo "the pilot addendum and writes the confirmation one" >&2
    exit 2
  fi
  if ! grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM"; then
    echo "$ADDENDUM identifies no pilot receipt digest" >&2
    exit 2
  fi
  if ! grep -Eq '"measurement_resolution": [0-9]' "$ADDENDUM"; then
    echo "$ADDENDUM leaves the measurement resolution unresolved" >&2
    exit 2
  fi
fi
if [[ "$MODE" != build && "$MODE" != smoke && -e "$OUT" ]]; then
  echo "receipt directory $OUT already exists; remove it to re-run" >&2
  exit 2
fi
# Correctness evidence precedes timing.
if ! python3 -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1]))["passed"] else 1)' \
     "$SURVEY/validation.json"; then
  echo "run-validation.sh has not recorded a passing conformance run" >&2
  exit 2
fi

# Builds finish before timed work.
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
"$SURVEY/build-arms.sh" >/dev/null
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)

if [[ "$MODE" == build ]]; then
  echo "# build complete; no campaign staged" >&2
  echo "# runner: $RUNNER" >&2
  echo "# acceptance: $ACCEPTANCE" >&2
  exit 0
fi

ARM_DIR=$(realpath "target/53c5a8c0-arms-$VARIANT/release")
if [[ "$MODE" == smoke ]]; then
  STAGE=$(realpath -m "target/bench-smoke/$ISSUE-$FAMILY-stage")
  rm -rf "$STAGE" "$STAGE.plan.json" "$OUT"
else
  STAGE=$(realpath -m "target/bench-stage/$ISSUE-$FAMILY-$MODE")
fi
PLAN="$STAGE.plan.json"
mkdir -p "$(dirname "$STAGE")"
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"

# A resumed invocation keeps the plan it paused on, campaign identity included.
if [[ -f "$PLAN" ]]; then
  echo "# resuming the campaign already staged at $STAGE" >&2
else
  CAMPAIGN="$MODE-$FAMILY-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
  python3 "$SURVEY/make-plan.py" "$PLAN" "$CAMPAIGN" "$FAMILY_ID" "$LABEL" \
    "$ADDENDUM" "$SEED" "$ARM_DIR" "$LOCK" $HOLDOUT
fi

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# family: $FAMILY_ID"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# plan: $PLAN"
  echo "# stage: $STAGE"
  echo "# arms: $ARM_DIR"
  echo "# runner: $RUNNER"
  echo "# acceptance: $ACCEPTANCE"
  echo "# load_avg_start: $(uptime)"
} >>"$LAUNCH_LOG"

# Bounded resumable sessions: each holds the exclusive mutex for at most the
# plan's session budget so sibling workers are not starved.
session=0
while true; do
  session=$((session + 1))
  set +e
  GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN" | tee -a "$LAUNCH_LOG"
  status=${PIPESTATUS[0]}
  set -e
  echo "# session $session exit: $status (0 = complete, 3 = paused)" >>"$LAUNCH_LOG"
  case "$status" in
    0) break ;;
    3) continue ;;
    *) echo "session $session exited $status" >&2; exit "$status" ;;
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
echo "$FAMILY $MODE receipt: $OUT" >&2
exit "$verdict"
