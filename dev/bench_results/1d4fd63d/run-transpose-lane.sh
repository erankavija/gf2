#!/usr/bin/env bash
# Transpose-lane campaigns (jit:1d4fd63d).
#
# Usage:
#   dev/bench_results/1d4fd63d/run-transpose-lane.sh smoke|pilot|selected|confirmation [date-utc] [suffix]
#   dev/bench_results/1d4fd63d/run-transpose-lane.sh external-pilot|external-confirmation [date-utc] [suffix]
#   dev/bench_results/1d4fd63d/run-transpose-lane.sh build
#
# Both timed modes measure the same arms: the baseline runs the block kernel
# `gf2_kernels_simd::transpose::detect` publishes, the candidate runs one lane
# named through `transpose::lane`. One executable serves every arm and selects
# its lane from `GF2_TRANSPOSE_LANE`, so a measured ratio attributes to the
# lane rather than to two builds.
#
# `smoke` runs the smallest case of every workload kind and every arm wiring
# through the real runner and the real acceptance tool, so the child protocol
# and the plan derivation are proved on the wire before a timed campaign is
# queued. Its family and ledger are its own and its receipt is labelled
# `smoke`, which is never a performance result about gf2.
#
# `pilot` is the ranking stage: every cell is exploratory and its recorded
# outcomes select the lane a confirmation can reach. `selected` repeats that
# lane's cells and the identity control at the confirmatory sample, so the
# measurement resolution the confirmation freezes against is the one a
# confirmatory sample has; its cells are exploratory too. `confirmation`
# refuses to run until
# `dev/active/c7113c5a/survey/freeze-confirmation.py` has derived the
# confirmation addendum from the committed pilot receipt, stamping that
# resolution, the receipt's digest and the cell selection, so the confirmatory
# family's thresholds are frozen before its first trial and never after a
# result.
#
# `external-pilot` and `external-confirmation` measure the second family, which
# puts each gf2 lane against M4RI and Bitshuffle at the kernel geometry
# 6fb89a3c froze. Their gf2 side is this issue's own arm; their external arms
# are 6fb89a3c's pinned executables, rebuilt by its committed `fetch-build.sh`
# and checked byte for byte against its committed `build-evidence.json` here
# before any timed run, with the bit mapping of every arm checked against naive
# bit arithmetic by `survey/verify-bit-mapping.py`.
#
# `build` checks the conformance record and builds the runner, the acceptance
# tool and the arm executable, then stops: it proves this launcher's
# pre-timing pipeline runs to completion under a minimal environment without
# starting a campaign or writing a receipt.
#
# Every numeric setting comes from the frozen addendum and the protocol's
# frozen shared settings, except the exploratory pair count `PILOT_PAIRS`,
# which the protocol leaves to the launcher between `pilot_min_pairs` and
# `pilot_max_pairs`; it is declared here with its reason and passed to the
# plan derivation.
#
# `--full-host` is the wrapper form the protocol requires of a measurement
# run; every cell declares one worker and the runner narrows the affinity to
# each cell's resolved CPUs itself.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
# A minimal environment (a systemd unit with no login shell) carries no
# ~/.cargo/bin and no pinned toolchain; every other tool this launcher calls
# resolves from /usr/bin.
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
ISSUE=1d4fd63d
ACTIVE="dev/active/$ISSUE"
RESULTS="dev/bench_results/$ISSUE"

# Exploratory pair count. Twelve pairs are six counterbalanced blocks: half the
# confirmatory sample, which keeps the ranking stage inside one benchmark
# window at nineteen cells while still estimating the family's resolution from
# more than the protocol's minimum block.
PILOT_PAIRS=12

MODE=${1:-}
# Which family this mode measures: `selection` compares gf2 lanes against each
# other, `external` compares them against M4RI and Bitshuffle. Each has its own
# addenda, ledger, arms, plan derivation and producing manifest.
FAMILY=selection
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
# A suffix separates receipts of the same label and date. A superseded receipt
# stays committed under its own directory rather than being replaced.
SUFFIX=${3:-}

case "$MODE" in
  smoke)
    # Pipeline proof, never a performance result: the smallest case of every
    # workload kind and every arm wiring, through the real runner.
    LABEL=smoke
    ADDENDUM="$ACTIVE/addendum-v4-transpose-lane-smoke.json"
    OUT="$RESULTS/$DATE_UTC-$ISSUE-transpose-lane-smoke${SUFFIX}"
    SEED=20260913001
    ;;
  pilot)
    LABEL=pilot
    ADDENDUM="$ACTIVE/addendum-v4-transpose-lane-pilot.json"
    OUT="$RESULTS/$DATE_UTC-$ISSUE-transpose-lane-pilot${SUFFIX}"
    SEED=20260913101
    ;;
  selected)
    LABEL=pilot
    ADDENDUM="$ACTIVE/addendum-v4-transpose-lane-selected.json"
    OUT="$RESULTS/$DATE_UTC-$ISSUE-transpose-lane-selected${SUFFIX}"
    SEED=20260913151
    # The stage that sizes the resolution runs at the confirmatory sample, so
    # its cells take the frozen `confirmatory_pairs` rather than a pilot count.
    PILOT_PAIRS=24
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM="$ACTIVE/addendum-v4-transpose-lane-confirmation.json"
    OUT="$RESULTS/$DATE_UTC-$ISSUE-transpose-lane-confirmation${SUFFIX}"
    SEED=20260913201
    ;;
  external-pilot)
    LABEL=pilot
    FAMILY=external
    ADDENDUM="$ACTIVE/addendum-v4-external-pilot.json"
    OUT="$RESULTS/$DATE_UTC-$ISSUE-external-pilot${SUFFIX}"
    SEED=20260913301
    PILOT_PAIRS=24
    ;;
  external-confirmation)
    LABEL=confirmation
    FAMILY=external
    ADDENDUM="$ACTIVE/addendum-v4-external-confirmation.json"
    OUT="$RESULTS/$DATE_UTC-$ISSUE-external-confirmation${SUFFIX}"
    SEED=20260913401
    ;;
  build) ;;
  *)
    echo "usage: $0 smoke|pilot|selected|confirmation|external-pilot|external-confirmation [date-utc] [suffix] | build" >&2
    exit 2
    ;;
esac

if [[ "$MODE" == confirmation || "$MODE" == external-confirmation ]]; then
  if [[ ! -f "$ADDENDUM" ]]; then
    echo "$ADDENDUM does not exist; derive it from the committed pilot receipt" >&2
    echo "with dev/active/c7113c5a/survey/freeze-confirmation.py" >&2
    exit 2
  fi
  grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM" ||
    { echo "$ADDENDUM identifies no pilot receipt digest" >&2; exit 2; }
  grep -Eq '"measurement_resolution": [0-9]' "$ADDENDUM" ||
    { echo "$ADDENDUM leaves the measurement resolution unresolved" >&2; exit 2; }
fi
if [[ "$MODE" != build ]]; then
  if [[ -e "$OUT" ]]; then
    echo "receipt directory $OUT already exists; remove it to re-run" >&2
    exit 2
  fi
  # Publication precedes measurement: the addendum is committed bytes.
  git ls-files --error-unmatch "$ADDENDUM" >/dev/null
  git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
  LEDGER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["family_wise"]["ledger_path"])' "$ADDENDUM")
  [[ -f "$LEDGER" ]] || { echo "family ledger $LEDGER must exist (genesis) before the first campaign" >&2; exit 2; }
  git ls-files --error-unmatch "$LEDGER" >/dev/null
fi

# Correctness evidence precedes timing.
python3 -c 'import json,sys; sys.exit(0 if json.load(open(sys.argv[1]))["passed"] else 1)' \
  "$ACTIVE/validation.json" ||
  { echo "run-validation.sh has not recorded a passing conformance run" >&2; exit 2; }

# Builds finish before timed work.
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
if [[ "$FAMILY" == external ]]; then
  (cd "$ACTIVE/external-arms" && "$repo/scripts/cargo-budget.sh" cargo build --release)
  ARM=$(realpath "$ACTIVE/external-arms/target/release/transpose-lane-external-arm")
  # The external arms are 6fb89a3c's, rebuilt from its committed sources and
  # accepted only when byte-identical to the digests it pinned; the bit mapping
  # of every arm of every cell is checked against naive bit arithmetic before
  # any timing.
  SURVEY="$repo/dev/active/6fb89a3c/survey"
  GF2_SURVEY_EXT=${GF2_SURVEY_EXT:-$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/.agents/ext/6fb89a3c}
  export GF2_SURVEY_EXT
  "$SURVEY/fetch-build.sh" "$GF2_SURVEY_EXT"
  python3 "$ACTIVE/survey/check-external-arms.py" --survey "$SURVEY"
  python3 "$ACTIVE/survey/verify-bit-mapping.py" --arm "$ARM" --survey "$SURVEY"
else
  (cd "$ACTIVE/arms" && "$repo/scripts/cargo-budget.sh" cargo build --release)
  ARM=$(realpath "$ACTIVE/arms/target/release/transpose-lane-arm")
fi

if [[ "$MODE" == build ]]; then
  echo "# build complete; no campaign staged" >&2
  echo "# runner: $RUNNER" >&2
  echo "# acceptance: $ACCEPTANCE" >&2
  echo "# arm: $ARM" >&2
  exit 0
fi

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
  if [[ "$FAMILY" == external ]]; then
    python3 "$ACTIVE/make-external-plan.py" "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" \
      "$SEED" "$ARM" "$SURVEY" "$LOCK" "$PILOT_PAIRS"
  else
    python3 "$ACTIVE/make-plan.py" "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$SEED" "$ARM" "$LOCK" "$PILOT_PAIRS"
  fi
fi

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE  label: $LABEL"
  echo "# addendum: $ADDENDUM  sha256: $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
  echo "# ledger: $LEDGER  sha256: $(sha256sum "$LEDGER" | cut -d' ' -f1)"
  echo "# plan: $PLAN  sha256: $(sha256sum "$PLAN" | cut -d' ' -f1)"
  echo "# exploratory pairs per cell: $PILOT_PAIRS"
  echo "# stage: $STAGE"
  echo "# arm: $(sha256sum "$ARM")"
  echo "# runner: $RUNNER"
  echo "# acceptance: $ACCEPTANCE"
  echo "# toolchain: $(rustc --version)"
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
  echo "# sessions this invocation: $session"
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$LAUNCH_LOG"
echo "$MODE receipt: $OUT" >&2
exit "$verdict"
