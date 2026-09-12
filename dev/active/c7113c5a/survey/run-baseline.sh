#!/usr/bin/env bash
# Produce a polynomial-multiplication baseline receipt (jit:c7113c5a).
#
# Usage:
#   ./run-baseline.sh pilot|confirmation|confirmation-2 [date-utc] [suffix]
#
# `confirmation-2` runs the superseding addendum: the ten cells of
# `confirmation` unchanged plus a six-cell build ladder that gives the
# conservative, tuned and native builds of both sides the same two operand
# sizes. It is one campaign, so the family pays one Bonferroni correction over
# its sixteen cells and one prior confirmatory trial. It refuses to run while
# that addendum declares protocol version 1: re-measuring the ten carried cells
# is a second confirmatory attempt, which the protocol permits only under a new
# protocol version.
#
# Builds the protocol runner and the acceptance tool under `--release`, asserts
# that the arm binaries and the pinned gf2x builds already exist and that the
# correctness validation passed, then runs the frozen family as bounded resumable
# sessions under the canonical CCX1 lock wrapper and finalizes the receipt.
#
# The pilot must be committed before its digest is placed in the confirmation
# addendum: the confirmatory family's measurement resolution is pinned to that
# receipt by content digest, which is what makes the confirmation's thresholds
# frozen before its first trial.
#
# `--full-host` is required rather than the default CCX1 pinning because the
# family declares six-core, twelve-core and twenty-four-logical-CPU arms; the
# runner narrows the affinity to each cell's resolved CPU set itself, so the
# single-core cells are still measured on one CPU.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=c7113c5a
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
# A suffix separates receipts of the same label and date. Superseded receipts
# stay committed under their own directory rather than being replaced.
SUFFIX=${3:-}
EXT="${GF2_SURVEY_EXT:-$repo/.agents/ext/$ISSUE}"

case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/$ISSUE/addendum-polynomial-baselines-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-polynomial-pilot${SUFFIX}"
    SEED=20260907101
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/$ISSUE/addendum-polynomial-baselines.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-polynomial-confirmation${SUFFIX}"
    SEED=20260907201
    ;;
  confirmation-2)
    LABEL=confirmation
    ADDENDUM=dev/active/$ISSUE/addendum-polynomial-baselines-2.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-polynomial-confirmation-2${SUFFIX}"
    SEED=20260908201
    ;;
  *)
    echo "usage: $0 pilot|confirmation|confirmation-2 [date-utc]" >&2
    exit 2
    ;;
esac

if [[ "$MODE" == confirmation* ]] && \
   ! grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM"; then
  echo 'confirmation addendum does not identify a pilot receipt digest' >&2
  exit 2
fi
if [[ -e "$OUT" ]]; then
  echo "receipt directory $OUT already exists; remove it to re-run" >&2
  exit 2
fi
# `confirmation-2` re-measures the ten cells of `confirmation`, so it is a second
# confirmatory attempt on the same candidate identity while
# `max_confirmatory_attempts_per_candidate` is 1. The protocol allows a further
# attempt only under a new candidate identity or a new protocol version, and this
# campaign takes the second route. Refuse to run it until the addendum is
# re-stamped to a protocol version above 1, which is what lifting this guard
# means: the re-stamp happens before any measurement, never after a result.
if [[ "$MODE" == confirmation-2 ]] && \
   [[ $(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["protocol"]["version"])' \
        "$ADDENDUM") -le 1 ]]; then
  echo "$ADDENDUM still declares protocol version 1; confirmation-2 is a second" >&2
  echo 'confirmatory attempt and needs protocol version 2 before it may run' >&2
  exit 2
fi
# Correctness evidence precedes timing.
if ! grep -q '"passed": true' dev/active/$ISSUE/survey/validation.json || \
   grep -q '"passed": false' dev/active/$ISSUE/survey/validation.json; then
  echo 'survey/run-validation.sh has not recorded a passing validation' >&2
  exit 2
fi
for variant in conservative tuned native; do
  for binary in gf2-poly-arm gf2x-poly-arm; do
    test -x "$EXT/arms-$variant/release/$binary" || {
      echo "missing $EXT/arms-$variant/release/$binary; run survey/fetch-build.sh" >&2
      exit 2
    }
  done
done

# Builds finish before timed work.
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)

CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"

python3 - "$PLAN" "$CAMPAIGN" "$EXT" "$LOCK" "$MODE" "$LABEL" "$ADDENDUM" "$SEED" <<'PY_PLAN'
import json, sys

plan_path, campaign, ext, lock, mode, label, addendum, seed = sys.argv[1:]

CFLAGS = {
    "conservative": "-O2",
    "tuned": "-O3 -march=x86-64-v3",
    "native": "-O3 -march=native",
}
RUSTFLAGS = {
    "conservative": None,
    "tuned": "-C target-cpu=x86-64-v3",
    "native": "-C target-cpu=native",
}
BUILD = {
    "conservative": "conservative-portable",
    "tuned": "tuned-portable",
    "native": "native",
}


def gf2_arm(variant, schoolbook=False):
    """The gf2 arm of one build variant; `schoolbook` selects the public
    long-product API instead of the dispatched wide kernel."""
    environment = {"GF2_POLY_PATH": "schoolbook"} if schoolbook else {}
    description = (
        "gf2 {variant} build: {what}, gf2-core and gf2-kernels-simd by path, "
        "RUSTFLAGS={flags}"
    ).format(
        variant=variant,
        what=(
            "clmul_wide_slice public long-product API"
            if schoolbook
            else "dispatched wide kernels and clmul_batch"
        ),
        flags=RUSTFLAGS[variant] or "(none)",
    )
    return {
        "build": BUILD[variant],
        "description": description,
        "executable": f"{ext}/arms-{variant}/release/gf2-poly-arm",
        "arguments": [],
        "environment": environment,
        "rustflags": RUSTFLAGS[variant],
        "tuning_profile": None,
    }


def gf2x_arm(variant):
    """The gf2x arm of one build variant. Its build identity is `external`
    for every variant, so the C flags are recorded in the description and the
    executable digest separates the three."""
    return {
        "build": "external",
        "description": (
            f"gf2x 1.3.0 (commit 27ba588f03bf6e1e74763903bab25e6e8bb6d0f0, "
            f"GPL-3.0-or-later) built with CFLAGS={CFLAGS[variant]}; configure "
            f"appended -msse2 -msse3 -mssse3 -msse4.1 -mpclmul and selected "
            f"hwdir=x86_64_pclmul. Rust shell RUSTFLAGS="
            f"{RUSTFLAGS[variant] or '(none)'}; the arm asserts at run time "
            f"that it mapped {ext}/prefix-{variant}/lib/libgf2x.so."
        ),
        "executable": f"{ext}/arms-{variant}/release/gf2x-poly-arm",
        "arguments": [],
        "environment": {},
        "rustflags": RUSTFLAGS[variant],
        "tuning_profile": None,
    }


def poly(words, seed, inner=1):
    return {"kind": "poly-mul", "words": words, "inner": inner, "seed": seed}


def batch(count, seed, inner=1):
    return {"kind": "clmul-batch", "count": count, "inner": inner, "seed": seed}


def dot(count, seed, inner=1):
    return {"kind": "gf2m-dot", "count": count, "inner": inner, "seed": seed}


def cell(cell_id, baseline, candidate, case, pilot=None):
    return {
        "cell_id": cell_id,
        "baseline_arm": baseline,
        "candidate_arm": candidate,
        "case": case,
        "pilot_pairs": pilot,
    }


if mode == "pilot":
    arms = {
        "gf2-conservative": gf2_arm("conservative"),
        "gf2-tuned": gf2_arm("tuned"),
        "gf2-native": gf2_arm("native"),
        "gf2-native-schoolbook": gf2_arm("native", schoolbook=True),
        "gf2x-conservative": gf2x_arm("conservative"),
        "gf2x-tuned": gf2x_arm("tuned"),
        "gf2x-native": gf2x_arm("native"),
    }
    cells = [
        cell("poly-mul-4w-native-pilot", "gf2-native", "gf2x-native", poly(4, 101), 6),
        cell("poly-mul-256w-native-pilot", "gf2-native", "gf2x-native", poly(256, 102), 6),
        cell("poly-mul-256w-conservative-pilot", "gf2-conservative", "gf2x-conservative", poly(256, 103), 6),
        cell("poly-mul-256w-tuned-pilot", "gf2-tuned", "gf2x-tuned", poly(256, 104), 6),
        cell("poly-mul-4w-public-api-pilot", "gf2-native-schoolbook", "gf2-native", poly(4, 105), 6),
        cell("clmul-batch-1024-native-pilot", "gf2-native", "gf2x-native", batch(1024, 106), 6),
    ]
    session_budget = 3
elif mode == "confirmation":
    arms = {
        "gf2-native": gf2_arm("native"),
        "gf2x-native": gf2x_arm("native"),
    }
    cells = [
        cell("poly-mul-4w-1core", "gf2-native", "gf2x-native", poly(4, 201)),
        cell("poly-mul-9w-1core", "gf2-native", "gf2x-native", poly(9, 202)),
        cell("poly-mul-64w-1core", "gf2-native", "gf2x-native", poly(64, 203)),
        cell("poly-mul-256w-1core", "gf2-native", "gf2x-native", poly(256, 204)),
        cell("poly-mul-2048w-streaming-1core", "gf2-native", "gf2x-native", poly(2048, 205)),
        cell("poly-mul-256w-6core", "gf2-native", "gf2x-native", poly(256, 206, inner=32)),
        cell("poly-mul-256w-12core", "gf2-native", "gf2x-native", poly(256, 207, inner=32)),
        cell("poly-mul-256w-24smt", "gf2-native", "gf2x-native", poly(256, 208, inner=32)),
        cell("clmul-batch-1024-1core", "gf2-native", "gf2x-native", batch(1024, 209)),
        cell("gf2m-dot-1024-1core", "gf2-native", "gf2x-native", dot(1024, 210)),
    ]
    session_budget = 5
else:
    # confirmation-2: the ten cells above, unchanged, plus the build ladder.
    # Each ladder cell pairs the gf2 arm of one variant with the gf2x arm of
    # the same variant, which is the equivalent host-targeting opportunity the
    # addendum declares; the addendum can only say `external` for every gf2x
    # arm, so the pairing lives here and in the receipt's arm descriptors.
    arms = {
        "gf2-conservative": gf2_arm("conservative"),
        "gf2-tuned": gf2_arm("tuned"),
        "gf2-native": gf2_arm("native"),
        "gf2x-conservative": gf2x_arm("conservative"),
        "gf2x-tuned": gf2x_arm("tuned"),
        "gf2x-native": gf2x_arm("native"),
    }
    cells = [
        cell("poly-mul-4w-1core", "gf2-native", "gf2x-native", poly(4, 201)),
        cell("poly-mul-9w-1core", "gf2-native", "gf2x-native", poly(9, 202)),
        cell("poly-mul-64w-1core", "gf2-native", "gf2x-native", poly(64, 203)),
        cell("poly-mul-256w-1core", "gf2-native", "gf2x-native", poly(256, 204)),
        cell("poly-mul-2048w-streaming-1core", "gf2-native", "gf2x-native", poly(2048, 205)),
        cell("poly-mul-256w-6core", "gf2-native", "gf2x-native", poly(256, 206, inner=32)),
        cell("poly-mul-256w-12core", "gf2-native", "gf2x-native", poly(256, 207, inner=32)),
        cell("poly-mul-256w-24smt", "gf2-native", "gf2x-native", poly(256, 208, inner=32)),
        cell("clmul-batch-1024-1core", "gf2-native", "gf2x-native", batch(1024, 209)),
        cell("gf2m-dot-1024-1core", "gf2-native", "gf2x-native", dot(1024, 210)),
        cell("poly-mul-4w-conservative-1core", "gf2-conservative", "gf2x-conservative", poly(4, 211)),
        cell("poly-mul-4w-tuned-1core", "gf2-tuned", "gf2x-tuned", poly(4, 212)),
        cell("poly-mul-4w-native-1core", "gf2-native", "gf2x-native", poly(4, 213)),
        cell("poly-mul-256w-conservative-1core", "gf2-conservative", "gf2x-conservative", poly(256, 214)),
        cell("poly-mul-256w-tuned-1core", "gf2-tuned", "gf2x-tuned", poly(256, 215)),
        cell("poly-mul-256w-native-1core", "gf2-native", "gf2x-native", poly(256, 216)),
    ]
    # Sixteen cells in four bounded sessions rather than two long ones: the
    # exclusive mutex is released three times mid-campaign so sibling workers
    # are not starved, and the runner resumes from its checkpoints.
    session_budget = 4

plan = {
    "schema": "zen3-benchmark-plan-v1",
    "campaign_id": campaign,
    "issue": "c7113c5a",
    "label": label,
    "campaign_seed": int(seed),
    "addendum": addendum,
    "lock_path": lock,
    "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
    "timing_override": None,
    "arms": arms,
    "cells": cells,
    "max_cells_per_session": session_budget,
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
  echo "# staging: $EXT"
  echo "# runner: $RUNNER"
  echo "# acceptance: $ACCEPTANCE"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"

# Bounded resumable sessions: each holds the exclusive mutex for at most
# `max_cells_per_session` cells so sibling workers are not starved.
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
