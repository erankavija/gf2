#!/usr/bin/env bash
# Byte-field vector and matrix comparison campaign (jit:6c6b09b1).
#
# Produces the CURRENT-CODE baseline receipts that compare gf2's byte-field
# consumer entry points against the pinned M4RIE, GF-Complete and ISA-L
# builds, under the frozen Zen 3 benchmark protocol.
#
# Order of work, which the measurement contract fixes: every build finishes
# and every adapter passes its conformance check before the first timed
# window opens. Timed sessions run under the exclusive CCX1 mutex in
# `--full-host` mode, so the runner resolves the core arm's CPU ids inside the
# whole processor rather than inside a pre-imposed taskset, and each session
# sets `CARGO_CI_NO_LOCK=1` because no cargo work may run under that mutex.
#
#   1. Build the runner, the acceptance tool, the C shim and both arms.
#   2. Validate the adapters against the independent scalar reference, run the
#      shared field-law suite for GF(2^8), and read the arithmetic backend
#      each pinned library selects on this host out of the loaded binaries.
#   3. Emit the plan and run the campaign in bounded sessions under the
#      exclusive CCX1 mutex, resuming from checkpoints between sessions.
#   4. Finalize the receipt and accept it with the independent tool.
#
# Usage: dev/bench_results/6c6b09b1/run-byte-field.sh pilot|confirmation [date-utc]
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=6c6b09b1
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
SURVEY=dev/active/$ISSUE/survey
# Extends the receipt's source provenance from the protocol tooling to this
# survey's own arms, so the arm sources enter the campaign's content identity
# rather than only the executables built from them.
PRODUCING=dev/active/$ISSUE/producing-inputs.json
PRIMARY="$(cd "$(dirname "$(git rev-parse --git-common-dir)")" && pwd)"
EXT="${GF2_SURVEY_EXT:-$PRIMARY/.agents/ext/$ISSUE}"
export GF2_SURVEY_EXT="$EXT"

case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/$ISSUE/addendum-byte-field-arms-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-byte-field-pilot"
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/$ISSUE/addendum-byte-field-arms.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-byte-field-confirmation"
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
if [[ ! -d "$EXT/prefix/lib" ]]; then
  echo "run $SURVEY/fetch-build.sh before this script" >&2
  exit 2
fi

# ---------------------------------------------------------------- 1. builds
# Builds finish before timed work. The externals are compiled with
# -O3 -march=native, so the gf2 arm is compiled with the matching
# -C target-cpu=native rather than the portable default; both sides then see
# the same host ISA and the comparison is not a codegen-target comparison.
GF2_ARM_RUSTFLAGS="-C target-cpu=native"
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
# The shared side of the CCX1 mutex is taken through cargo-budget.sh, which is
# its only supported shared acquirer: a direct `flock -s` skips the turnstile
# and can enter ahead of a queued measurement run.
./scripts/cargo-budget.sh make -C "$SURVEY" -j4
SURVEY_TARGET="$EXT/survey-target"
CARGO_TARGET_DIR="$SURVEY_TARGET" RUSTFLAGS="$GF2_ARM_RUSTFLAGS" \
  ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "$SURVEY/gf2-side/Cargo.toml"
CARGO_TARGET_DIR="$SURVEY_TARGET" \
  ./scripts/cargo-budget.sh cargo build --release \
    --manifest-path "$SURVEY/ext-side/Cargo.toml"

RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
GF2_ARM=$(realpath "$SURVEY_TARGET/release/gf2-arm")
EXT_ARM=$(realpath "$SURVEY_TARGET/release/ext-arm")

# ------------------------------------------------------------ 2. validation
# Correctness evidence precedes timing. Each check exits non-zero on the
# first mismatch, so a failure here stops the campaign before it starts.
CONFORMANCE="dev/active/$ISSUE/conformance"
mkdir -p "$CONFORMANCE"
./scripts/cargo-budget.sh \
  "$SURVEY/byte_field_conformance" >"$CONFORMANCE/externals.txt"
./scripts/cargo-budget.sh \
  "$SURVEY_TARGET/release/gf2-conformance" >"$CONFORMANCE/gf2-side.txt"
./scripts/cargo-budget.sh \
  "$EXT_ARM" --validate >"$CONFORMANCE/ext-wrapper.txt"
./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features \
  --cargo-profile ci-test --profile ci \
  -E 'test(field::axiom_tests::test_gf2_8_field_axioms)' \
  >"$CONFORMANCE/field-laws.txt" 2>&1
./scripts/cargo-budget.sh \
  "$SURVEY/arm-provenance.sh" >"dev/active/$ISSUE/arm-provenance.txt"

# ------------------------------------------------------------------ 3. plan
CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 "$SURVEY/make-plan.py" \
  --plan "$PLAN" --campaign "$CAMPAIGN" --issue "$ISSUE" --label "$LABEL" \
  --mode "$MODE" --addendum "$ADDENDUM" --lock "$LOCK" \
  --gf2-arm "$GF2_ARM" --ext-arm "$EXT_ARM" --rustflags "$GF2_ARM_RUSTFLAGS"

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# external staging: $EXT"
  echo "# gf2 arm rustflags: $GF2_ARM_RUSTFLAGS"
  echo "# external compiler flags: $(make -s -C "$SURVEY" identities | sed -n 's/^cflags=//p')"
  make -s -C "$SURVEY" identities | sed 's/^/# external identity: /'
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"

# ------------------------------------------------------------- 4. campaign
# Bounded sessions keep the exclusive mutex out of any one worker's hands for
# long: the runner pauses with status 3 after its per-session cell limit and
# the next session resumes from the checkpoint store without repeating a cell.
status=3
while [[ $status -eq 3 ]]; do
  set +e
  GF2_BENCH=1 CARGO_CI_NO_LOCK=1 \
    dev/scripts/ccx1-bench-flock.sh --full-host \
      "$RUNNER" run "$STAGE" "$PLAN" "$PRODUCING" \
    | tee -a "$LAUNCH_LOG"
  status=${PIPESTATUS[0]}
  set -e
  echo "# session exit: $status (3 = paused after the session cell limit)" >>"$LAUNCH_LOG"
done
if [[ $status -ne 0 ]]; then
  echo "campaign exited $status" >&2
  exit "$status"
fi

"$RUNNER" finalize "$STAGE" "$OUT" | tee -a "$LAUNCH_LOG"
cp "$LAUNCH_LOG" "$OUT/launcher.log"
cp "$CONFORMANCE/externals.txt" "$CONFORMANCE/gf2-side.txt" \
   "$CONFORMANCE/ext-wrapper.txt" "$CONFORMANCE/field-laws.txt" \
   "dev/active/$ISSUE/arm-provenance.txt" "$OUT/"
# The arm sources are pinned by the runner itself through
# dev/active/6c6b09b1/producing-inputs.json, so no private digest file is kept
# beside the receipt.
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
