#!/usr/bin/env bash
# Zen 3 protocol receipts for the bit-storage consumer profile (jit:04b85d10).
#
# Builds the runner, the acceptance tool and the consumer arm under --release,
# then measures either the exploratory pilot or the separately frozen
# confirmation as a sequence of bounded sessions under the canonical CCX1 lock
# wrapper. The pilot is published before its digest enters the confirmatory
# addendum.
#
# The wrapper runs in its --full-host mode because this family declares
# twelve-physical-core and twenty-four-logical-CPU arms, which the six-core pin
# cannot resolve. It holds the same exclusive mutex, so the run stays
# serialized against sibling benchmark work.
#
# Usage: dev/bench_results/04b85d10/run-consumer-profile.sh pilot|confirmation [date-utc]
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=04b85d10
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/$ISSUE/addendum-bit-storage-consumers-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-consumers-pilot"
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/$ISSUE/addendum-bit-storage-consumers.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-consumers-confirmation"
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

# Builds finish before timed work. The harness keeps its artefacts in the
# git-excluded staging area so the committed tree carries no build output.
HARNESS_MANIFEST=dev/active/$ISSUE/survey/gf2-side/Cargo.toml
HARNESS_TARGET="$repo/target/consumer-profile-$ISSUE"
CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
CARGO_TARGET_DIR="$HARNESS_TARGET" CARGO_CI_NO_SCCACHE=1 flock -s /tmp/gf2-ccx1.lock \
  ./scripts/cargo-budget.sh cargo build --release --manifest-path "$HARNESS_MANIFEST"

RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
ARM=$(realpath "$HARNESS_TARGET/release/consumer-arm")
VERIFY=$(realpath "$HARNESS_TARGET/release/consumer-verify")

CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"

python3 dev/active/$ISSUE/survey/build-plan.py \
  "$PLAN" "$CAMPAIGN" "$ARM" "$LOCK" "$MODE" "$LABEL" "$ADDENDUM"

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# arm sha256: $(sha256sum "$ARM" | cut -d' ' -f1)"
  echo "# rustc: $(rustc --version)"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"

# Correctness before timing: the routes a cell compares must agree first.
{
  echo "# consumer-verify:"
  "$VERIFY"
  echo "# consumer-verify exit: 0"
} >>"$LAUNCH_LOG"

# Bounded sessions: the runner pauses with exit 3 after the plan's cell limit
# and resumes from its checkpoints without repeating a completed cell.
session=0
while true; do
  session=$((session + 1))
  echo "# session $session start: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$LAUNCH_LOG"
  set +e
  GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN" | tee -a "$LAUNCH_LOG"
  code=${PIPESTATUS[0]}
  set -e
  echo "# session $session exit: $code" >>"$LAUNCH_LOG"
  case "$code" in
    0) break ;;
    3) continue ;;
    *) echo "session $session exited $code" >&2; exit 1 ;;
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
