#!/usr/bin/env bash
# DVB-T2 bit-interleaver external-baseline campaign (jit:eda07788).
#
# Usage: dev/bench_results/eda07788/run-dvb-t2-baselines.sh pilot-v3|pilot-v3-r2|confirmation-v3 [date-utc]
#
# Builds the three arm executables through the committed fetch/build path,
# builds the protocol runner and acceptance tool under `--release`, projects a
# `zen3-benchmark-plan-v1` from the frozen family addendum, then measures it as
# a sequence of bounded sessions under the canonical CCX1 mutex. Each session
# takes the lock, measures at most `max_cells_per_session` cells, checkpoints
# and releases, so a queued sibling worker gets the host between sessions and a
# killed session resumes without repeating a completed cell.
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
  pilot-v3)
    LABEL=pilot
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-dvb-t2-v3-pilot"
    MAX_CELLS=5
    PILOT_PAIRS=
    ;;
  pilot-v3-r2)
    LABEL=pilot
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-pilot-r2.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-dvb-t2-v3-pilot-r2"
    MAX_CELLS=1
    PILOT_PAIRS=24
    ;;
  confirmation-v3)
    LABEL=confirmation
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-confirmation.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-dvb-t2-v3-confirmation"
    MAX_CELLS=2
    PILOT_PAIRS=
    ;;
  *)
    echo "usage: $0 pilot-v3|pilot-v3-r2|confirmation-v3 [date-utc]" >&2
    exit 2
    ;;
esac
if [[ "$MODE" == confirmation-v3 ]] && \
   ! grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM"; then
  echo 'confirmation addendum does not identify a pilot receipt digest' >&2
  exit 2
fi
if [[ -e "$OUT" ]]; then
  echo "receipt directory $OUT already exists; remove it to re-run" >&2
  exit 2
fi

# Builds and the adapter correctness gate finish before any timed work.
EXT=${GF2_EDA07788_EXT:-$repo/.agents/ext/eda07788}
dev/active/eda07788/survey/fetch-build.sh "$EXT"
EXT=$(realpath "$EXT")
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
GF2_NATIVE=$(realpath "$EXT/target-native/release/gf2-dvb-t2-candidate")
GF2_PORTABLE=$(realpath "$EXT/target-portable/release/gf2-dvb-t2-candidate")
XDSOPL=$(realpath "$EXT/target-native/release/xdsopl-dvb-t2-baseline")

CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=/tmp/gf2-$CAMPAIGN
PLAN=/tmp/gf2-$CAMPAIGN.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$LOCK" "$MAX_CELLS" "$PILOT_PAIRS" \
  "$GF2_NATIVE" "$GF2_PORTABLE" "$XDSOPL" <<'PY_PLAN'
import json, sys

(plan_path, campaign, label, addendum_path, lock, max_cells, pilot_pairs,
 gf2_native, gf2_portable, xdsopl) = sys.argv[1:]

with open(addendum_path) as handle:
    addendum = json.load(handle)

NATIVE_RUSTFLAGS = "-C target-cpu=native"
PORTABLE_RUSTFLAGS = "-C target-cpu=x86-64"
ARMS = {
    "gf2-native": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "description": "gf2 DvbT2BitInterleaver::interleave on a packed BitVec, built with -C target-cpu=native",
    },
    "gf2-native-control": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "description": "identity control: the same gf2 native executable as the baseline arm, launched as a second arm",
    },
    "gf2-portable": {
        "build": "conservative-portable", "executable": gf2_portable, "rustflags": PORTABLE_RUSTFLAGS,
        "description": "scalar/compiler control: gf2 DvbT2BitInterleaver::interleave built with -C target-cpu=x86-64 (baseline x86-64, no target feature beyond SSE2)",
    },
    "xdsopl-external": {
        "build": "external", "executable": xdsopl, "rustflags": NATIVE_RUSTFLAGS,
        "description": "xdsopl/LDPC PCTITL template at commit 32357d8ad55a6a302c34e093759f0454e45cca56, C++ shim built with g++ -O3 -march=native -std=c++17; the timed call unpacks the packed frame to one int32 per bit, applies PCTITL::fwd and packs the result back",
    },
}
# The cell identifier names the two arms it compares; baseline is always the
# established gf2 arm and candidate is always the arm whose lead needs
# attribution, so a speedup above 1 favours the candidate.
SUFFIX_ARMS = [
    ("-null-native-vs-native", ("gf2-native", "gf2-native-control")),
    ("-gap-native-vs-xdsopl", ("gf2-native", "xdsopl-external")),
    ("-gap-portable-vs-xdsopl", ("gf2-portable", "xdsopl-external")),
    ("-control-portable-vs-native", ("gf2-portable", "gf2-native")),
]
WORKLOAD_PREFIX = "dvb-t2-bit-interleave-"


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
        raise SystemExit(f"cell {cell_id}: workload identity {identity!r} is not a DVB-T2 MODCOD")
    cells.append({
        "cell_id": cell_id,
        "baseline_arm": baseline,
        "candidate_arm": candidate,
        "case": {"modcod": identity[len(WORKLOAD_PREFIX):], "seed": declared["workload"]["seed"]},
        # `null` selects the frozen pilot minimum for an exploratory cell and
        # is rejected outright for a confirmatory one.
        "pilot_pairs": int(pilot_pairs) if pilot_pairs else None,
    })

plan = {
    "schema": "zen3-benchmark-plan-v1",
    "campaign_id": campaign,
    "issue": addendum["family"]["issue"],
    "label": label,
    "campaign_seed": 20260908,
    "addendum": addendum_path,
    "producing_manifest": "dev/active/eda07788/producing-inputs.json",
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
  echo "# toolchain: $(rustc --version)"
  echo "# c++: $(g++ --version | head -1)"
  echo "# arm digests:"
  for arm in "$GF2_NATIVE" "$GF2_PORTABLE" "$XDSOPL"; do
    echo "#   $(sha256sum "$arm")"
  done
  echo "# survey source digests:"
  for source in dev/active/eda07788/survey/fetch-build.sh \
                dev/active/eda07788/survey/xdsopl-shim/xdsopl_shim.cpp \
                dev/active/eda07788/survey/gf2-side/src/lib.rs \
                dev/active/eda07788/survey/gf2-side/src/bin/gf2-dvb-t2-candidate.rs \
                dev/active/eda07788/survey/gf2-side/src/bin/xdsopl-dvb-t2-baseline.rs \
                dev/active/eda07788/survey/gf2-side/src/bin/validate-permutation-equivalence.rs \
                dev/active/eda07788/survey/gf2-side/Cargo.toml \
                dev/active/eda07788/survey/gf2-side/Cargo.lock \
                dev/active/eda07788/survey/gf2-side/build.rs \
                dev/bench_results/eda07788/run-dvb-t2-baselines.sh; do
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
