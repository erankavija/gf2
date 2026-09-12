#!/usr/bin/env bash
# 5G NR LLR de-rate-matching external-baseline campaign (jit:eda07788).
#
# Usage: GF2_AFF3CT_ROOT=<aff3ct-tree> \
#   dev/bench_results/eda07788/run-nr-derate-baselines.sh pilot|confirmation [date-utc]
#
# GF2_AFF3CT_ROOT names the AFF3CT v4.7.0 tree `c077a88b`'s fetch/build script
# stages; `dev/active/eda07788/survey/nr-derate-build.sh` verifies its commit
# and static-library digest, builds the three arm executables and runs the
# equivalence gate. This script then builds the protocol runner and
# acceptance tool under `--release`, projects a `zen3-benchmark-plan-v1` from
# the frozen family addendum, and measures it as a sequence of bounded
# sessions under the canonical CCX1 mutex. Each session takes the lock,
# measures at most `max_cells_per_session` cells, checkpoints and releases, so
# a queued sibling worker gets the host between sessions and a killed session
# resumes without repeating a completed cell.
#
# Every numeric setting comes from the addendum and from the protocol's frozen
# shared settings; this script adds none. The cell-to-arm wiring is derived
# from the cell identifiers, which name the two arms they compare.
#
# The pilot must be published, and its digest placed in the confirmatory
# addendum, before the confirmation runs.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=eda07788
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
case "$MODE" in
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/eda07788/addendum-nr-llr-derate-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-nr-derate-pilot"
    MAX_CELLS=2
    PILOT_PAIRS=24
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/eda07788/addendum-nr-llr-derate-confirmation.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-nr-derate-confirmation"
    MAX_CELLS=2
    PILOT_PAIRS=
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
: "${GF2_AFF3CT_ROOT:?GF2_AFF3CT_ROOT must name the staged AFF3CT v4.7.0 tree}"

# Builds and the equivalence gate finish before any timed work.
EXT=${GF2_EDA07788_EXT:-$repo/.agents/ext/eda07788}
dev/active/eda07788/survey/nr-derate-build.sh "$GF2_AFF3CT_ROOT" "$EXT"
EXT=$(realpath "$EXT")
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
GF2_NATIVE=$(realpath "$EXT/target-nr-native/release/gf2-nr-derate-arm")
GF2_PORTABLE=$(realpath "$EXT/target-nr-portable/release/gf2-nr-derate-arm")
AFF3CT=$(realpath "$EXT/target-nr-native/release/aff3ct-nr-derate-arm")

CAMPAIGN="nr-derate-$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$LOCK" "$MAX_CELLS" "$PILOT_PAIRS" \
  "$GF2_NATIVE" "$GF2_PORTABLE" "$AFF3CT" <<'PY_PLAN'
import json, sys

(plan_path, campaign, label, addendum_path, lock, max_cells, pilot_pairs,
 gf2_native, gf2_portable, aff3ct) = sys.argv[1:]

with open(addendum_path) as handle:
    addendum = json.load(handle)

NATIVE_RUSTFLAGS = "-C target-cpu=native"
PORTABLE_RUSTFLAGS = "-C target-cpu=x86-64"
ARMS = {
    "gf2-native": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "description": "gf2 Nr5gRateMatchedCode::prepare_llrs on a &[Llr] channel frame, built with -C target-cpu=native",
    },
    "gf2-native-control": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "description": "identity control: the same gf2 native executable as the baseline arm, launched as a second arm",
    },
    "gf2-portable": {
        "build": "conservative-portable", "executable": gf2_portable, "rustflags": PORTABLE_RUSTFLAGS,
        "description": "scalar/compiler control: gf2 Nr5gRateMatchedCode::prepare_llrs built with -C target-cpu=x86-64 (baseline x86-64, no target feature beyond SSE2)",
    },
    "aff3ct-external": {
        "build": "external", "executable": aff3ct, "rustflags": NATIVE_RUSTFLAGS,
        "description": "AFF3CT v4.7.0 (commit e8a65c5047262d97a15563b9edc961f69b2792cc) Puncturer_5G<int32_t, float>::depuncture through a C-ABI shim, static library sha256 b9605974b1d1413524e3a393a79c0842594683fdc9e240eee4f72a287cb92529 built -O3 -march=native -funroll-loops; the timed call converts the &[Llr] frame to float, zero-fills the N_LDPC output, runs the public depuncture, and converts back to Vec<Llr> with +inf fillers replaced by gf2's filler value",
    },
}
# The cell identifier names the two arms it compares; baseline is always the
# established gf2 arm and candidate is always the arm whose lead needs
# attribution, so a speedup above 1 favours the candidate.
SUFFIX_ARMS = [
    ("-null-native-vs-native", ("gf2-native", "gf2-native-control")),
    ("-gap-native-vs-aff3ct", ("gf2-native", "aff3ct-external")),
    ("-control-portable-vs-native", ("gf2-portable", "gf2-native")),
]
WORKLOAD_PREFIX = "nr-llr-derate-"


def arms_for(cell_id):
    for suffix, pair in SUFFIX_ARMS:
        if cell_id.endswith(suffix):
            return pair
    raise SystemExit(f"cell {cell_id!r} does not name a known arm pair")


cells = []
used = set()
for declared in addendum["cells"]:
    cell_id = declared["cell_id"]
    baseline, candidate = arms_for(cell_id)
    for name, build in ((baseline, declared["builds"]["baseline"]),
                        (candidate, declared["builds"]["candidate"])):
        if ARMS[name]["build"] != build:
            raise SystemExit(f"cell {cell_id}: arm {name} is {ARMS[name]['build']}, addendum declares {build}")
    used.update((baseline, candidate))
    identity = declared["workload"]["identity"]
    if not identity.startswith(WORKLOAD_PREFIX):
        raise SystemExit(f"cell {cell_id}: workload identity {identity!r} is not an NR de-rate-matching configuration")
    cells.append({
        "cell_id": cell_id,
        "baseline_arm": baseline,
        "candidate_arm": candidate,
        "case": {"configuration": identity[len(WORKLOAD_PREFIX):], "seed": declared["workload"]["seed"]},
        # `null` selects the frozen pilot minimum for an exploratory cell and
        # is rejected outright for a confirmatory one.
        "pilot_pairs": int(pilot_pairs) if pilot_pairs else None,
    })

plan = {
    "schema": "zen3-benchmark-plan-v1",
    "campaign_id": campaign,
    "issue": addendum["family"]["issue"],
    "label": label,
    "campaign_seed": 20260910,
    "addendum": addendum_path,
    "producing_manifest": "dev/active/eda07788/producing-inputs-nr-derate.json",
    "lock_path": lock,
    "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
    "timing_override": None,
    "arms": {name: {"build": arm["build"], "description": arm["description"],
                    "executable": arm["executable"], "arguments": [], "environment": {},
                    "rustflags": arm["rustflags"], "tuning_profile": None}
             for name, arm in ARMS.items() if name in used},
    "cells": cells,
    "max_cells_per_session": int(max_cells),
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
print(f"plan: {len(cells)} cells, {len(plan['arms'])} arms -> {plan_path}")
PY_PLAN

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# addendum sha256: $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# external staging: $EXT"
  echo "# aff3ct root: $(realpath "$GF2_AFF3CT_ROOT")"
  echo "# aff3ct static library sha256: $(sha256sum "$GF2_AFF3CT_ROOT/build/lib/libaff3ct-4.7.0.a" | cut -d' ' -f1)"
  echo "# toolchain: $(rustc --version)"
  echo "# c++: $(c++ --version | head -1)"
  echo "# arm digests:"
  for arm in "$GF2_NATIVE" "$GF2_PORTABLE" "$AFF3CT"; do
    echo "#   $(sha256sum "$arm")"
  done
  echo "# survey source digests:"
  for source in dev/active/eda07788/survey/nr-derate-build.sh \
                dev/active/eda07788/survey/nr-derate/cpp/aff3ct_depuncture_shim.cpp \
                dev/active/eda07788/survey/nr-derate/src/lib.rs \
                dev/active/eda07788/survey/nr-derate/src/bin/gf2-nr-derate-arm.rs \
                dev/active/eda07788/survey/nr-derate/src/bin/aff3ct-nr-derate-arm.rs \
                dev/active/eda07788/survey/nr-derate/src/bin/validate-nr-derate-equivalence.rs \
                dev/active/eda07788/survey/nr-derate/Cargo.toml \
                dev/active/eda07788/survey/nr-derate/Cargo.lock \
                dev/active/eda07788/survey/nr-derate/build.rs \
                dev/bench_results/eda07788/run-nr-derate-baselines.sh; do
    echo "#   $(sha256sum "$source")"
  done
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"

# Bounded checkpointed sessions: exit 3 means the session paused at the cell
# budget and the campaign resumes; exit 0 means the campaign is complete.
# `CARGO_CI_NO_LOCK=1` is set for anything the session might shell out to,
# because this process holds the exclusive side of the same mutex that
# `scripts/cargo-budget.sh` takes shared.
session=0
while :; do
  session=$((session + 1))
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
echo "$MODE receipt: $OUT" >&2
exit "$verdict"
