#!/usr/bin/env bash
# Reproducible bounded pilot/confirmation launcher for jit:6fb89a3c.
#
# Usage: run-campaign.sh transpose|logical|bch pilot|confirmation [date-utc]
set -euo pipefail

REPO=$(git rev-parse --show-toplevel)
cd "$REPO"
ISSUE=6fb89a3c
FAMILY=${1:-}
MODE=${2:-}
DATE_UTC=${3:-$(date -u +%Y-%m-%d)}
case "$FAMILY" in transpose|logical|bch) ;; *) echo "usage: $0 transpose|logical|bch pilot|confirmation [date-utc]" >&2; exit 2;; esac
case "$MODE" in pilot|confirmation) ;; *) echo "usage: $0 transpose|logical|bch pilot|confirmation [date-utc]" >&2; exit 2;; esac
SUFFIX=
[[ "$MODE" == pilot ]] && SUFFIX=-pilot

case "$FAMILY" in
  transpose) ADDENDUM="dev/active/$ISSUE/addendum-transpose$SUFFIX.json" ;;
  logical) ADDENDUM="dev/active/$ISSUE/addendum-logical-buffer$SUFFIX.json" ;;
  bch) ADDENDUM="dev/active/$ISSUE/addendum-bch-genmatrix$SUFFIX.json" ;;
esac
[[ -f "$ADDENDUM" ]] || { echo "missing frozen addendum $ADDENDUM" >&2; exit 2; }

OUT="dev/bench_results/$ISSUE/$DATE_UTC-$ISSUE-$FAMILY-$MODE"
[[ ! -e "$OUT" ]] || { echo "receipt directory already exists: $OUT" >&2; exit 2; }

# Every build finishes before the exclusive timing lock is requested.
EXT=${GF2_SURVEY_EXT:-$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/.agents/ext/$ISSUE}
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
export RUSTUP_TOOLCHAIN=1.95.0
CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh make -C "dev/active/$ISSUE/survey" EXT="$EXT"
CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95.0 build --release \
  -p tuning-campaign-support --bin benchmark-ab-runner --bin benchmark-acceptance
CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo +1.95.0 build --release \
  --manifest-path "dev/active/$ISSUE/survey/gf2-side/Cargo.toml"

SURVEY="dev/active/$ISSUE/survey"
export GF2_SURVEY_EXT="$EXT"
export GF2_SURVEY_GF2_DUMP="$REPO/$SURVEY/gf2-side/target/release/gf2_dump_check"
"$SURVEY/record-build-evidence.py"
"$SURVEY/verify-bit-mapping.py"
"$SURVEY/verify-arm-framing.py"

RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
GF2_TRANSPOSE=$(realpath "$SURVEY/gf2-side/target/release/gf2_transpose_arm")
GF2_LOGICAL=$(realpath "$SURVEY/gf2-side/target/release/gf2_logical_xor_arm")
GF2_BCH=$(realpath "$SURVEY/gf2-side/target/release/gf2_bch_genmatrix_arm")
VALIDATE_ADDENDA=$(realpath "$SURVEY/gf2-side/target/release/validate_addenda")
M4RI_TRANSPOSE=$(realpath "$SURVEY/m4ri_transpose_arm")
BITSHUFFLE=$(realpath "$SURVEY/bitshuffle_transpose_arm")
ISAL=$(realpath "$SURVEY/isal_xor_arm")
M4RI_BCH=$(realpath "$SURVEY/m4ri_genmatrix_arm")
"$VALIDATE_ADDENDA" dev/active/f547c394/addendum.schema.json "$ADDENDUM"
CAMPAIGN="$FAMILY-$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
STAGE="/tmp/gf2-$CAMPAIGN"
PLAN="$STAGE.plan.json"

python3 - "$PLAN" "$CAMPAIGN" "$ADDENDUM" "$LOCK" "$FAMILY" "$MODE" \
  "$GF2_TRANSPOSE" "$GF2_LOGICAL" "$GF2_BCH" "$M4RI_TRANSPOSE" "$BITSHUFFLE" "$ISAL" "$M4RI_BCH" <<'PY_PLAN'
import json, sys
(
    plan_path, campaign, addendum_path, lock, family, mode,
    gf2_transpose, gf2_logical, gf2_bch, m4ri_transpose, bitshuffle, isal, m4ri_bch,
) = sys.argv[1:]
addendum = json.loads(open(addendum_path).read())

def arm(build, description, executable):
    return {"build": build, "description": description, "executable": executable,
            "arguments": [], "environment": {}, "rustflags": None, "tuning_profile": None}

arms = {}
cells = []
for declared in addendum["cells"]:
    cell_id = declared["cell_id"]
    size = declared["workload"]["size"]
    seed = declared["workload"]["seed"]
    case = dict(size)
    case["seed"] = seed
    if family == "transpose":
        baseline = "gf2-transpose"
        candidate = "bitshuffle" if "bitshuffle" in cell_id else "m4ri-transpose"
        arms[baseline] = arm(declared["builds"]["baseline"], "gf2 current pinned transpose; runtime selected path reported by child", gf2_transpose)
        arms["m4ri-transpose"] = arm("external", "M4RI 20260122 mzd_transpose; -O3 -march=native; word-SWAR path", m4ri_transpose)
        if candidate == "bitshuffle":
            arms[candidate] = arm("external", "Bitshuffle 0.5.2 bshuf_bitshuffle; -O3 -march=native; compiled backend reported by child", bitshuffle)
    elif family == "logical":
        baseline, candidate = "gf2-logical", "isal-base"
        arms[baseline] = arm("conservative-portable", "gf2 current pinned xor_inplace with aligned fresh-output arrangement", gf2_logical)
        arms[candidate] = arm("external", "ISA-L v2.32.1 xor_gen_base scalar; public multi-binary xor_gen unavailable without NASM", isal)
    else:
        baseline, candidate = "gf2-bch-genmatrix", "m4ri-bch-genmatrix"
        code = {15: "B1", 127: "B2", 255: "B3"}[size["n"]]
        case = {"code": code, "seed": seed}
        arms[baseline] = arm("conservative-portable", "current gf2 bch_generator_matrix_by_encoding; established bch_genmatrix rows", gf2_bch)
        arms[candidate] = arm("external", "M4RI 20260122 current genmatrix-rref route reused from issue 4e732b56", m4ri_bch)
    cells.append({"cell_id": cell_id, "baseline_arm": baseline, "candidate_arm": candidate,
                  "case": case, "pilot_pairs": 6 if mode == "pilot" else None})

plan = {
    "schema": "zen3-benchmark-plan-v1", "campaign_id": campaign,
    "issue": "6fb89a3c", "label": mode, "campaign_seed": 20260908,
    "addendum": addendum_path, "lock_path": lock,
    "wrapper": "dev/scripts/ccx1-bench-flock.sh", "timing_override": None,
    "arms": arms, "cells": cells, "max_cells_per_session": 2,
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
PY_PLAN

LAUNCH_LOG="$STAGE.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# family: $FAMILY"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# addendum_sha256: $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# lock invocation: RUSTUP_TOOLCHAIN=1.95.0 CARGO_CI_NO_LOCK=1 GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host $RUNNER run $STAGE $PLAN"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"
echo "GF2_CAMPAIGN_LAUNCHER_LOG=$LAUNCH_LOG"

session=0
while true; do
  session=$((session + 1))
  set +e
  CARGO_CI_NO_LOCK=1 GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN" | tee -a "$LAUNCH_LOG"
  rc=${PIPESTATUS[0]}
  set -e
  echo "# session $session exit: $rc" >>"$LAUNCH_LOG"
  [[ $rc -eq 3 ]] && continue
  [[ $rc -eq 0 ]] || exit "$rc"
  break
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
