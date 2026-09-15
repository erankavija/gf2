#!/usr/bin/env bash
# Produce one count-optimization receipt (jit:5cbb6545).
#
# Usage:
#   ./run-count-campaign.sh <stage> [date-utc] [suffix]
#   ./run-count-campaign.sh build
#
# Stages, in the order the issue runs them:
#
#   smoke                         every arm identity once through the real
#                                 runner, into a throwaway receipt under
#                                 target/, so the wire contract is established
#                                 by an execution before a committed campaign.
#   popcount-sweep2, fused-sweep2 exploratory refinements that narrow the
#                                 crossover each first sweep bracketed.
#   popcount-sweep, fused-sweep   exploratory crossover searches. Every cell is
#                                 exploratory, so the stage decides nothing and
#                                 spends no confirmatory comparison; it fixes
#                                 the word counts the pilot and confirmation
#                                 then measure.
#   popcount-pilot, fused-pilot   exactly the cells the confirmation decides,
#                                 measured exploratorily; the committed receipt
#                                 is the resolution evidence the confirmation
#                                 addendum pins.
#   popcount-confirmation,        the frozen confirmatory stage. It refuses to
#   fused-confirmation            run until
#                                 dev/active/c7113c5a/survey/freeze-confirmation.py
#                                 has derived its addendum from the committed
#                                 pilot receipt, stamping that receipt's digest
#                                 and the measurement resolution: the family's
#                                 thresholds are frozen before its first trial,
#                                 never after a result.
#
# `build` checks the correctness evidence and builds the runner, the acceptance
# tool and the arm executable, then stops: it proves the launcher's pre-timing
# pipeline runs to completion under a minimal environment without starting a
# campaign or writing a receipt.
#
# The launcher builds everything before timed work, then runs bounded resumable
# sessions under the canonical exclusive wrapper. Re-invoking the same stage
# resumes the same campaign: the stage directory and plan are derived from the
# stage name, so a paused or interrupted session continues under its own
# identity instead of starting a second campaign.
#
# `--full-host` is the wrapper form the protocol requires of a measurement run;
# every family declares single-core cells only and the runner narrows the
# affinity to each cell's resolved CPU itself.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
# A minimal environment (a systemd unit with no login shell) carries no
# ~/.cargo/bin and no pinned toolchain; every other tool this launcher or its
# helpers call resolves from /usr/bin. Exported before any cargo or tool
# invocation, mirroring dev/active/1c602857/run-public-clmul.sh.
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
ISSUE=5cbb6545
ACTIVE="dev/active/$ISSUE"
STAGE_NAME=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
# A suffix separates receipts of the same stage and date. A superseded receipt
# stays committed under its own directory rather than being replaced.
SUFFIX=${3:-}

case "$STAGE_NAME" in
  popcount-sweep)       FAMILY=popcount; KIND=sweep;        LABEL=pilot;        SEED=20260913101 ;;
  fused-sweep)          FAMILY=fused;    KIND=sweep;        LABEL=pilot;        SEED=20260913102 ;;
  popcount-sweep2)      FAMILY=popcount; KIND=sweep2;       LABEL=pilot;        SEED=20260913111 ;;
  fused-sweep2)         FAMILY=fused;    KIND=sweep2;       LABEL=pilot;        SEED=20260913112 ;;
  popcount-pilot)       FAMILY=popcount; KIND=pilot;        LABEL=pilot;        SEED=20260913201 ;;
  fused-pilot)          FAMILY=fused;    KIND=pilot;        LABEL=pilot;        SEED=20260913202 ;;
  popcount-confirmation) FAMILY=popcount; KIND=confirmation; LABEL=confirmation; SEED=20260913301 ;;
  fused-confirmation)   FAMILY=fused;    KIND=confirmation; LABEL=confirmation; SEED=20260913302 ;;
  smoke)                FAMILY=smoke;    KIND=smoke;        LABEL=pilot;        SEED=20260913001 ;;
  build) ;;
  *)
    echo "usage: $0 <stage> [date-utc] [suffix] | build" >&2
    echo "stages: smoke popcount-sweep fused-sweep popcount-sweep2 fused-sweep2" >&2
    echo "        popcount-pilot fused-pilot" >&2
    echo "        popcount-confirmation fused-confirmation" >&2
    exit 2
    ;;
esac

if [[ "$STAGE_NAME" != build ]]; then
  ADDENDUM="$ACTIVE/addendum-$FAMILY-v4-$KIND.json"
  if [[ "$KIND" == smoke ]]; then
    # A smoke establishes the wire contract and makes no performance claim, so
    # its receipt is a throwaway under target/ rather than committed evidence.
    OUT="target/bench-smoke/$DATE_UTC-$ISSUE-$STAGE_NAME${SUFFIX}"
  else
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-$STAGE_NAME${SUFFIX}"
  fi
fi

if [[ "${KIND:-}" == confirmation ]]; then
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
if [[ "$STAGE_NAME" != build && -e "$OUT" ]]; then
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
./scripts/cargo-budget.sh cargo build --release \
  --manifest-path "$ACTIVE/survey/gf2-side/Cargo.toml"
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
ARM=$(realpath "$ACTIVE/survey/gf2-side/target/release/count-arm")
VERIFY=$(realpath "$ACTIVE/survey/gf2-side/target/release/count-verify")

if [[ "$STAGE_NAME" == build ]]; then
  echo "# build complete; no campaign staged" >&2
  echo "# runner: $RUNNER" >&2
  echo "# acceptance: $ACCEPTANCE" >&2
  echo "# arm: $ARM" >&2
  exit 0
fi

STAGE=$(realpath -m "target/bench-stage/$ISSUE-$STAGE_NAME")
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
  CAMPAIGN="$STAGE_NAME-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
  python3 "$ACTIVE/survey/make-plan.py" "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$SEED" "$ARM" "$LOCK"
fi

# Every arm of every planned cell answers its exact case before the mutex is
# taken: a plan whose arms disagree never reaches a timed window.
"$VERIFY" --plan "$PLAN"

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# stage: $STAGE_NAME"
  echo "# addendum: $ADDENDUM"
  echo "# plan: $PLAN"
  echo "# stage directory: $STAGE"
  echo "# arm: $ARM"
  echo "# runner: $RUNNER"
  echo "# acceptance: $ACCEPTANCE"
  echo "# external build: $("$ARM" --build-identity)"
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
echo "$STAGE_NAME receipt: $OUT" >&2
exit "$verdict"
