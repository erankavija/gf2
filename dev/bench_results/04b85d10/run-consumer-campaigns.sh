#!/usr/bin/env bash
# Protocol-v3 receipts for the bit-storage consumer families (jit:04b85d10).
#
# Usage: dev/bench_results/04b85d10/run-consumer-campaigns.sh <family> pilot|confirmation [date-utc]
#   family: logical | count | layout
#
# Builds the protocol runner, the acceptance tool and the consumer harness
# under `--release` with the repository MSRV toolchain, proves the compared
# routes agree, projects a `zen3-benchmark-plan-v1` from the frozen family
# addendum, then measures it as a sequence of bounded sessions under the
# canonical CCX1 mutex. Each session takes the lock through the turnstile,
# measures at most `max_cells_per_session` cells, checkpoints and releases,
# so a queued sibling gets the host between sessions and a killed session
# resumes without repeating a completed cell.
#
# The wrapper runs in its --full-host mode because the layout family declares
# a six-physical-core arm the six-core pin cannot resolve; it holds the same
# exclusive mutex, so the run stays serialized against sibling benchmark work.
# Builds take the shared side of that mutex through `scripts/cargo-budget.sh`,
# the only supported shared acquirer, and finish before any timed work.
#
# Every numeric setting comes from the addendum and from the protocol's frozen
# shared settings; this script adds none. The pilot must be published, and its
# digest placed in the confirmatory addendum, before the confirmation runs.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=04b85d10
FAMILY=${1:-}
MODE=${2:-}
DATE_UTC=${3:-$(date -u +%Y-%m-%d)}
case "$FAMILY" in
  logical|count|layout) ;;
  *) echo "usage: $0 logical|count|layout pilot|confirmation [date-utc]" >&2; exit 2 ;;
esac
case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/$ISSUE/addendum-bit-storage-$FAMILY-v3-pilot.json
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/$ISSUE/addendum-bit-storage-$FAMILY-v3-confirmation.json
    ;;
  *) echo "usage: $0 logical|count|layout pilot|confirmation [date-utc]" >&2; exit 2 ;;
esac
OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-$FAMILY-v3-$MODE"
LEDGER="dev/bench_results/$ISSUE/v3-bit-storage-$FAMILY-consumers-family-ledger.jsonl"
MAX_CELLS=3

if [[ "$MODE" == confirmation ]]; then
  # Publication precedes confirmation: the frozen addendum names a committed
  # pilot digest and is itself committed unchanged.
  grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM" ||
    { echo 'confirmation addendum does not identify a pilot receipt digest' >&2; exit 2; }
  git ls-files --error-unmatch "$ADDENDUM" >/dev/null
  git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
fi
[[ -f "$LEDGER" ]] || { echo "family ledger $LEDGER must exist before the first campaign" >&2; exit 2; }
[[ ! -e "$OUT" ]] || { echo "receipt directory $OUT already exists; remove it to re-run" >&2; exit 2; }

# Builds finish before timed work. The harness keeps its artefacts in the
# git-excluded target tree so the committed tree carries no build output.
export RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
HARNESS_MANIFEST=dev/active/$ISSUE/survey/gf2-side/Cargo.toml
HARNESS_TARGET="$repo/target/consumer-profile-$ISSUE"
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
CARGO_TARGET_DIR="$HARNESS_TARGET" ./scripts/cargo-budget.sh cargo build --release \
  --manifest-path "$HARNESS_MANIFEST"

RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
ARM=$(realpath "$HARNESS_TARGET/release/consumer-arm")
VERIFY=$(realpath "$HARNESS_TARGET/release/consumer-verify")

CAMPAIGN="$MODE-v3-$FAMILY-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE="$repo/target/consumer-campaigns/$CAMPAIGN"
PLAN="$STAGE.plan.json"
mkdir -p "$(dirname "$STAGE")"
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"

python3 dev/active/$ISSUE/survey/build-plan.py \
  "$PLAN" "$CAMPAIGN" "$ARM" "$LOCK" "$LABEL" "$ADDENDUM" "$MAX_CELLS"

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# family: $FAMILY  mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# addendum sha256: $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
  echo "# ledger: $LEDGER ($(grep -c . "$LEDGER") lines before this campaign)"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# toolchain: $(rustc --version)"
  echo "# RUSTFLAGS: ${RUSTFLAGS:-<unset>}"
  echo "# arm sha256: $(sha256sum "$ARM" | cut -d' ' -f1)"
  echo "# verify sha256: $(sha256sum "$VERIFY" | cut -d' ' -f1)"
  echo "# runner sha256: $(sha256sum "$RUNNER" | cut -d' ' -f1)"
  echo "# acceptance sha256: $(sha256sum "$ACCEPTANCE" | cut -d' ' -f1)"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"

# Correctness before timing: the routes a cell compares must agree first.
{
  echo "# consumer-verify:"
  "$VERIFY"
  echo "# consumer-verify exit: 0"
} >>"$LAUNCH_LOG"

# Bounded checkpointed sessions: exit 3 means the session paused at the cell
# budget and the campaign resumes; exit 0 means the campaign is complete.
# `CARGO_CI_NO_LOCK=1` is set for anything the session might shell out to,
# because this process holds the exclusive side of the same mutex that
# `scripts/cargo-budget.sh` takes shared.
session=0
while :; do
  session=$((session + 1))
  echo "# session $session start: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >>"$LAUNCH_LOG"
  set +e
  GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN" | tee -a "$LAUNCH_LOG"
  rc=${PIPESTATUS[0]}
  set -e
  echo "# session $session exit: $rc" >>"$LAUNCH_LOG"
  case "$rc" in
    0) break ;;
    3) ;;
    *) echo "session $session failed with $rc" >&2; exit "$rc" ;;
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
  echo "# sessions: $session"
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$LAUNCH_LOG"
echo "$FAMILY $MODE receipt: $OUT" >&2
exit "$verdict"
