#!/usr/bin/env bash
# 5G NR rate-matched encoder external-baseline campaign (jit:12fdeb5b).
#
# Usage (from the worktree root):
#   dev/bench_results/12fdeb5b/run-nr-encode-baselines.sh smoke|pilot|confirmation [date-utc]
#
# GF2_AFF3CT_ROOT names the AFF3CT v4.7.0 tree and GF2_SRSRAN_ROOT the srsRAN
# Project tree that `dev/active/c077a88b/survey/fetch-build.sh` stages. Both
# default to that script's staging location under the primary checkout, which a
# linked worktree reaches through the common git directory, so an invocation
# from any worktree needs no path of its own. GF2_12FDEB5B_EXT names this
# survey's own build directory, `.agents/ext/12fdeb5b` inside the invoking
# checkout, which is git-ignored and rebuildable.
# `dev/active/12fdeb5b/survey/nr-encode-build.sh` verifies their commits and
# the AFF3CT static-library digest, builds the four arm executables, records
# the source and build evidence, and runs the bit-exact equivalence gate. This
# script then builds the protocol runner and acceptance tool under `--release`,
# projects a `zen3-benchmark-plan-v1` from the frozen family addendum, and
# measures it as a sequence of bounded sessions under the canonical CCX1 mutex.
# Each session takes the lock, measures at most `max_cells_per_session` cells,
# checkpoints and releases, so a queued sibling worker gets the host between
# sessions and a killed session resumes without repeating a completed cell.
#
# Every numeric setting comes from the addendum and from the protocol's frozen
# shared settings; this script adds none. The cell-to-arm wiring is derived
# from the cell identifiers, which name the two arms they compare.
#
# The smoke label is a functional check that reaches a result line from every
# arm; it is not a performance result about gf2. The pilot must be published,
# and its digest placed in the confirmatory addendum, before the confirmation
# runs.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }

# The comparator source trees `dev/active/c077a88b/survey/fetch-build.sh` clones
# are shared: they live under the primary checkout, which a linked worktree
# reaches through the common git directory. This survey's own build output is
# not shared; it stays inside the invoking checkout, so two worktrees never
# write the same target directory.
primary=$(dirname "$(realpath "$(git rev-parse --git-common-dir)")")
: "${GF2_AFF3CT_ROOT:=$primary/.agents/ext/c077a88b/aff3ct}"
: "${GF2_SRSRAN_ROOT:=$primary/.agents/ext/c077a88b/srsran}"

# The arms and the tooling are release executables the campaign digests; pin
# the MSRV toolchain so every stage builds the same bytes.
export PATH="$HOME/.cargo/bin:$PATH" RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.95}"

ISSUE=12fdeb5b
MODE=${1:-}
DATE_UTC=${2:-$(date -u +%Y-%m-%d)}
case "$MODE" in
  smoke)
    LABEL=smoke
    ADDENDUM=dev/active/12fdeb5b/addendum-nr-encode-smoke.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-nr-encode-smoke"
    MAX_CELLS=5
    PILOT_PAIRS=6
    ;;
  pilot)
    LABEL=pilot
    ADDENDUM=dev/active/12fdeb5b/addendum-nr-encode-pilot.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-nr-encode-pilot"
    MAX_CELLS=2
    PILOT_PAIRS=24
    ;;
  confirmation)
    LABEL=confirmation
    ADDENDUM=dev/active/12fdeb5b/addendum-nr-encode-confirmation.json
    OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-nr-encode-confirmation"
    MAX_CELLS=2
    PILOT_PAIRS=
    ;;
  *)
    echo "usage: $0 smoke|pilot|confirmation [date-utc]" >&2
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
for tree in "$GF2_AFF3CT_ROOT" "$GF2_SRSRAN_ROOT"; do
  [[ -d "$tree" ]] || { echo "comparator tree $tree is not staged" >&2; exit 2; }
done

# Builds, evidence and the equivalence gate finish before any timed work.
EXT=${GF2_12FDEB5B_EXT:-$repo/.agents/ext/12fdeb5b}
dev/active/12fdeb5b/survey/nr-encode-build.sh "$GF2_AFF3CT_ROOT" "$GF2_SRSRAN_ROOT" "$EXT"
EXT=$(realpath "$EXT")
AFF3CT_CONF=$(realpath "$GF2_AFF3CT_ROOT/conf")
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
GF2_NATIVE=$(realpath "$EXT/target-nr-encode-native/release/gf2-nr-encode-arm")
GF2_PORTABLE=$(realpath "$EXT/target-nr-encode-portable/release/gf2-nr-encode-arm")
SRSRAN=$(realpath "$EXT/target-nr-encode-native/release/srsran-nr-encode-arm")
AFF3CT=$(realpath "$EXT/target-nr-encode-native/release/aff3ct-nr-encode-arm")

CAMPAIGN="nr-encode-$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE=$repo/target/nr-encode-campaigns/$CAMPAIGN
PLAN=$STAGE.plan.json
mkdir -p "$(dirname "$STAGE")"
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$LOCK" "$MAX_CELLS" "$PILOT_PAIRS" \
  "$GF2_NATIVE" "$GF2_PORTABLE" "$SRSRAN" "$AFF3CT" "$AFF3CT_CONF" <<'PY_PLAN'
import json, sys

(plan_path, campaign, label, addendum_path, lock, max_cells, pilot_pairs,
 gf2_native, gf2_portable, srsran, aff3ct, aff3ct_conf) = sys.argv[1:]

with open(addendum_path) as handle:
    addendum = json.load(handle)

NATIVE_RUSTFLAGS = "-C target-cpu=native"
PORTABLE_RUSTFLAGS = "-C target-cpu=x86-64"
ARMS = {
    "gf2-native": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "environment": {},
        "description": "gf2 <Nr5gRateMatchedCode as BlockEncoder>::encode on a target_k-bit BitVec, built with -C target-cpu=native",
    },
    "gf2-native-control": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "environment": {},
        "description": "identity control: the same gf2 native executable as the baseline arm, launched as a second arm",
    },
    "gf2-portable": {
        "build": "conservative-portable", "executable": gf2_portable, "rustflags": PORTABLE_RUSTFLAGS,
        "environment": {},
        "description": "scalar/compiler control: the same gf2 encode built with -C target-cpu=x86-64 (baseline x86-64, no target feature beyond SSE2)",
    },
    "srsran-external": {
        "build": "external", "executable": srsran, "rustflags": NATIVE_RUSTFLAGS,
        "environment": {},
        "description": "srsRAN Project 25.10 (commit d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc, AGPL-3.0-or-later) ldpc_encoder::encode plus ldpc_rate_matcher::rate_match through a C-ABI shim over the LDPC translation units, compiled -O3 -march=native -std=c++17; the timed call converts the BitVec message into srsRAN's packed bit_buffer over K_LDPC bits with fillers zero, runs both public entry points at redundancy version 0 and one bit per symbol, and converts the packed output back to a BitVec",
    },
    "aff3ct-external": {
        "build": "external", "executable": aff3ct, "rustflags": NATIVE_RUSTFLAGS,
        "environment": {"GF2_AFF3CT_CONF": aff3ct_conf},
        "description": "AFF3CT v4.7.0 (commit e8a65c5047262d97a15563b9edc961f69b2792cc, MIT) Encoder_LDPC_QC_fast<int32_t>::encode plus Puncturer_5G<int32_t, float>::puncture through a C-ABI shim, static library sha256 b9605974b1d1413524e3a393a79c0842594683fdc9e240eee4f72a287cb92529 built -O3 -march=native -funroll-loops; the timed call converts the BitVec message to one int32_t per bit, allocates the mother and transmitted buffers, runs both public entry points, and converts the transmitted values back to a BitVec",
    },
}
# The cell identifier names the two arms it compares; baseline is always the
# established gf2 arm and candidate is always the arm whose lead needs
# attribution, so a speedup above 1 favours the candidate.
SUFFIX_ARMS = [
    ("-null-native-vs-native", ("gf2-native", "gf2-native-control")),
    ("-gap-native-vs-srsran", ("gf2-native", "srsran-external")),
    ("-gap-native-vs-aff3ct", ("gf2-native", "aff3ct-external")),
    ("-control-portable-vs-native", ("gf2-portable", "gf2-native")),
]
WORKLOAD_PREFIX = "nr-encode-"


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
        raise SystemExit(f"cell {cell_id}: workload identity {identity!r} is not an NR encoder configuration")
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
    "campaign_seed": 20260912,
    "addendum": addendum_path,
    "producing_manifest": "dev/active/12fdeb5b/producing-inputs.json",
    "lock_path": lock,
    "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
    "timing_override": None,
    "arms": {name: {"build": arm["build"], "description": arm["description"],
                    "executable": arm["executable"], "arguments": [],
                    "environment": arm["environment"],
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
  echo "# aff3ct conf: $AFF3CT_CONF"
  echo "# aff3ct static library sha256: $(sha256sum "$GF2_AFF3CT_ROOT/build/lib/libaff3ct-4.7.0.a" | cut -d' ' -f1)"
  echo "# srsran root: $(realpath "$GF2_SRSRAN_ROOT")"
  echo "# toolchain: $(rustc --version)"
  echo "# c++: $(c++ --version | head -1)"
  echo "# arm digests:"
  for arm in "$GF2_NATIVE" "$GF2_PORTABLE" "$SRSRAN" "$AFF3CT"; do
    echo "#   $(sha256sum "$arm")"
  done
  echo "# survey source digests:"
  for source in dev/active/12fdeb5b/survey/nr-encode-build.sh \
                dev/active/12fdeb5b/survey/nr-encode/cpp/aff3ct_encode_shim.cpp \
                dev/active/12fdeb5b/survey/nr-encode/cpp/srsran_encode_shim.cpp \
                dev/active/12fdeb5b/survey/nr-encode/src/lib.rs \
                dev/active/12fdeb5b/survey/nr-encode/src/aff3ct.rs \
                dev/active/12fdeb5b/survey/nr-encode/src/srsran.rs \
                dev/active/12fdeb5b/survey/nr-encode/src/bin/arm_common.rs \
                dev/active/12fdeb5b/survey/nr-encode/src/bin/gf2-nr-encode-arm.rs \
                dev/active/12fdeb5b/survey/nr-encode/src/bin/srsran-nr-encode-arm.rs \
                dev/active/12fdeb5b/survey/nr-encode/src/bin/aff3ct-nr-encode-arm.rs \
                dev/active/12fdeb5b/survey/nr-encode/Cargo.toml \
                dev/active/12fdeb5b/survey/nr-encode/Cargo.lock \
                dev/active/12fdeb5b/survey/nr-encode/build.rs \
                dev/active/12fdeb5b/survey/build-evidence.json \
                dev/active/12fdeb5b/survey/nr-encode-validation.json \
                dev/bench_results/12fdeb5b/run-nr-encode-baselines.sh; do
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
