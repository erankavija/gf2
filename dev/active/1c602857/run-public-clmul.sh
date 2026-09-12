#!/usr/bin/env bash
# Produce the public wide carry-less product before/after receipt (jit:1c602857).
#
# Usage:
#   ./run-public-clmul.sh pilot|confirmation [date-utc] [suffix]
#
# Both modes measure the same six cells with the same two arms: the baseline
# runs `clmul_wide_slice_portable`, which is the public path this issue
# replaced, word for word; the candidate runs `clmul_wide` and
# `clmul_wide_slice` as the issue leaves them. One executable serves both and
# selects its entry point from GF2_CLMUL_PATH, so the arms share a build.
#
# `pilot` measures every cell exploratorily and fixes the measurement
# resolution. `confirmation` refuses to run until
# dev/active/c7113c5a/survey/freeze-confirmation.py has derived the
# confirmation addendum from the committed pilot receipt, stamping that
# resolution and the receipt's digest: the confirmatory family's thresholds are
# frozen before its first trial, never after a result. That script also refuses
# a margin the pilot's resolution invalidates, so a pilot resolution at or
# above five percent means the equivalence margin is replaced, with its own
# rationale, before the confirmation runs.
#
# The launcher builds everything before timed work, then runs bounded resumable
# sessions under the canonical exclusive wrapper. Re-invoking the same mode
# resumes the same campaign: the stage directory and plan are derived from the
# mode, so a paused or interrupted session continues under its own identity
# instead of starting a second campaign.
#
# `--full-host` is the wrapper form the protocol requires of a measurement run;
# the family declares single-core cells only and the runner narrows the
# affinity to each cell's resolved CPU itself.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=1c602857
ACTIVE="dev/active/$ISSUE"
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
# A suffix separates receipts of the same label and date. A superseded receipt
# stays committed under its own directory rather than being replaced.
SUFFIX=${3:-}

case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM="$ACTIVE/addendum-v4-public-clmul-pilot.json"
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-public-clmul-pilot${SUFFIX}"
    SEED=20260912101
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM="$ACTIVE/addendum-v4-public-clmul-confirmation.json"
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-public-clmul-confirmation${SUFFIX}"
    SEED=20260912201
    ;;
  *)
    echo "usage: $0 pilot|confirmation [date-utc] [suffix]" >&2
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
if [[ -e "$OUT" ]]; then
  echo "receipt directory $OUT already exists; remove it to re-run" >&2
  exit 2
fi
# Correctness evidence precedes timing.
if ! python3 -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1]))["passed"] else 1)' \
     "$ACTIVE/validation.json"; then
  echo "run-validation.sh has not recorded a passing conformance run" >&2
  exit 2
fi

# Builds finish before timed work.
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
(cd "$ACTIVE/arms" && "$repo/scripts/cargo-budget.sh" cargo build --release)
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
ARM=$(realpath "$ACTIVE/arms/target/release/clmul-wide-arm")

STAGE=$(realpath -m "target/bench-stage/$ISSUE-$MODE")
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
  CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
  python3 "$ACTIVE/make-plan.py" "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$SEED" "$ARM" "$LOCK"
fi

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# plan: $PLAN"
  echo "# stage: $STAGE"
  echo "# arm: $ARM"
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
echo "$MODE receipt: $OUT" >&2
exit "$verdict"
