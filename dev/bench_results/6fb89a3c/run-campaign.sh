#!/usr/bin/env bash
# Transpose / logical-buffer / BCH-genmatrix comparator campaigns (jit:6fb89a3c).
#
# Usage:
#   dev/bench_results/6fb89a3c/run-campaign.sh preflight
#   dev/bench_results/6fb89a3c/run-campaign.sh transpose|logical|bch pilot|confirmation [run-id]
#
# `preflight` builds the external libraries and every arm through the
# committed fetch/build path, records the build/backend evidence, runs the
# bit-mapping and child-framing gates and regenerates the producing-input
# manifest. A campaign run then checks that the arm binaries are the ones the
# committed evidence describes, builds the protocol runner and acceptance
# tool under `--release`, projects a `zen3-benchmark-plan-v1` from the frozen
# family addendum, and measures it as a sequence of bounded sessions under the
# canonical CCX1 mutex (`dev/scripts/ccx1-bench-flock.sh --full-host`). Each
# session takes the lock, measures at most `max_cells_per_session` cells,
# checkpoints and releases, so a queued sibling gets the host between sessions
# and a killed session resumes without repeating a completed cell.
#
# Every numeric setting comes from the addendum and the protocol's frozen
# shared settings; this script adds none. The cell-to-arm wiring is derived
# from the cell identifiers, which name the external arm they compare against.
# The pilot must be published, and its digest placed in the confirmatory
# addendum, before the confirmation runs.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=6fb89a3c
SURVEY=dev/active/$ISSUE/survey
EXT=${GF2_SURVEY_EXT:-$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/.agents/ext/$ISSUE}
export GF2_SURVEY_EXT="$EXT"
export GF2_SURVEY_GF2_DUMP="$repo/$SURVEY/gf2-side/target/release/gf2_dump_check"
export GF2_SURVEY_GF2_TARGET="$repo/$SURVEY/gf2-side/target/release"
export RUSTUP_TOOLCHAIN=1.95.0 CARGO_CI_NO_SCCACHE=1

if [[ "${1:-}" == preflight ]]; then
  "$SURVEY/fetch-build.sh" "$EXT"
  "$SURVEY/record-build-evidence.py"
  "$SURVEY/verify-bit-mapping.py"
  "$SURVEY/verify-arm-framing.py"
  "$SURVEY/make-producing-inputs.py"
  exit 0
fi

FAMILY=${1:-}
MODE=${2:-}
RUN_ID=${3:-v3-r1}
case "$FAMILY" in
  transpose) ADDENDUM_STEM=addendum-transpose ;;
  logical) ADDENDUM_STEM=addendum-logical-buffer ;;
  bch) ADDENDUM_STEM=addendum-bch-genmatrix ;;
  *) echo "usage: $0 preflight | transpose|logical|bch pilot|confirmation [run-id]" >&2; exit 2 ;;
esac
case "$MODE" in pilot|confirmation) ;; *) echo "usage: $0 preflight | transpose|logical|bch pilot|confirmation [run-id]" >&2; exit 2 ;; esac
ADDENDUM=dev/active/$ISSUE/$ADDENDUM_STEM-v3-$MODE.json
OUT=dev/bench_results/$ISSUE/$RUN_ID-$ISSUE-$FAMILY-$MODE
LAUNCH_LOG=dev/bench_results/$ISSUE/$RUN_ID-$FAMILY-$MODE-launcher.log
[[ -f "$ADDENDUM" ]] || { echo "missing frozen addendum $ADDENDUM" >&2; exit 2; }
[[ ! -e "$OUT" ]] || { echo "receipt directory $OUT already exists" >&2; exit 2; }
if [[ "$MODE" == confirmation ]]; then
  # Publication precedes confirmation: the addendum and its pilot digest are committed bytes.
  git ls-files --error-unmatch "$ADDENDUM" >/dev/null
  git diff --exit-code HEAD -- "$ADDENDUM" >/dev/null
  grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM" || { echo 'confirmation addendum does not pin a pilot receipt digest' >&2; exit 2; }
fi
LEDGER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["family_wise"]["ledger_path"])' "$ADDENDUM")
[[ -f "$LEDGER" ]] || { echo "family ledger $LEDGER must exist (genesis) before the first campaign" >&2; exit 2; }
git ls-files --error-unmatch "$LEDGER" >/dev/null

# Builds finish before timed work; the arm binaries must be the ones the
# committed build evidence describes.
./scripts/cargo-budget.sh cargo +1.95.0 build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
python3 - "$SURVEY" <<'PY_CHECK'
import hashlib, json, pathlib, sys
survey = pathlib.Path(sys.argv[1])
evidence = json.loads((survey / "build-evidence.json").read_text())
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
for name, record in evidence["harness_binaries"].items():
    if digest(survey / name) != record["sha256"]:
        raise SystemExit(f"{name} differs from build-evidence.json; run preflight and commit the evidence")
for name, record in evidence["rust_harness_binaries"].items():
    if digest(survey / "gf2-side/target/release" / name) != record["sha256"]:
        raise SystemExit(f"{name} differs from build-evidence.json; run preflight and commit the evidence")
print("arm binaries match build-evidence.json")
PY_CHECK
"$SURVEY/gf2-side/target/release/validate_addenda" dev/active/f547c394/addendum.schema.json "$ADDENDUM"

RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
CAMPAIGN="$ISSUE-$RUN_ID-$FAMILY-$MODE"
STAGE=$repo/target/$ISSUE-campaigns/$CAMPAIGN
PLAN=$STAGE.plan.json
mkdir -p "$(dirname "$STAGE")"
[[ ! -e "$PLAN" ]] || { echo "plan $PLAN already exists; resume the existing campaign identity" >&2; exit 2; }
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
export RAYON_NUM_THREADS=1

python3 - "$PLAN" "$CAMPAIGN" "$MODE" "$ADDENDUM" "$LOCK" "$FAMILY" "$SURVEY" "$repo" <<'PY_PLAN'
import json, sys
plan_path, campaign, label, addendum_path, lock, family, survey, repo = sys.argv[1:]
addendum = json.load(open(addendum_path))
gf2 = f"{repo}/{survey}/gf2-side/target/release"
PORTABLE = "-C target-cpu=x86-64"
ARMS = {
    "gf2-transpose": {"build": "conservative-portable", "executable": f"{gf2}/gf2_transpose_arm", "rustflags": PORTABLE,
                      "description": "gf2 transpose: fixed cells call the runtime-dispatched 64x64 kernel (gf2_kernels_simd::transpose::detect), consumer cells call BitMatrix::transpose with a fresh output; x86-64 baseline build, kernel selected at run time and reported by the child"},
    "m4ri-transpose": {"build": "external", "executable": f"{repo}/{survey}/m4ri_transpose_arm", "rustflags": None,
                       "description": "M4RI 20260122 mzd_transpose (release tarball, GPL-2.0-or-later, gcc -O3 -march=native -fPIC, no runtime dispatch); fixed cells reuse a preallocated output, consumer cells allocate a fresh one"},
    "bitshuffle-transpose": {"build": "external", "executable": f"{repo}/{survey}/bitshuffle_transpose_arm", "rustflags": None,
                             "description": "Bitshuffle 0.5.2 bshuf_bitshuffle (commit 52aec3b80d05606c090956aecfe868489d96b95c, MIT, gcc -O3 -march=native -fPIC, compile-time AVX2 route); fixed cells transform 64 eight-byte elements into a preallocated output, adapter cells pad/pack/unpack around one block into a fresh canonical output"},
    "gf2-logical": {"build": "conservative-portable", "executable": f"{gf2}/gf2_logical_xor_arm", "rustflags": PORTABLE,
                    "description": "gf2 xor_inplace with the fresh-output arrangement (copy source 0, XOR the remaining sources in place); x86-64 baseline build, backend selected at run time by buffer size and reported by the child"},
    "isal-base": {"build": "external", "executable": f"{repo}/{survey}/isal_xor_arm", "rustflags": None,
                  "description": "ISA-L v2.32.1 xor_gen_base (commit 7c3479e0a9dac17f448603ec1ad64c7c625f530c, BSD-3-Clause, gcc -O3 -march=native -fPIC); the portable C reference of xor_gen, vects = sources + 1, 32-byte aligned; the NASM multi-binary xor_gen dispatcher is unavailable on this host"},
    "gf2-bch-genmatrix": {"build": "conservative-portable", "executable": f"{gf2}/gf2_bch_genmatrix_arm", "rustflags": PORTABLE,
                          "description": "gf2 generator-matrix construction on the established bch_genmatrix rows with a fresh BitMatrix per call: the production BchCode::generator_matrix materialization, or (reference cells) the test-support oracle bch_generator_matrix_by_encoding; x86-64 baseline build, route reported by the child"},
    "m4ri-bch-genmatrix": {"build": "external", "executable": f"{repo}/{survey}/m4ri_genmatrix_arm", "rustflags": None,
                           "description": "M4RI 20260122 shifted-generator fill plus mzd_echelonize_m4ri, the established construction of issue 4e732b56; fresh mzd_copy per call"},
}
def wiring(cell_id, size, seed):
    case = dict(size)
    case["seed"] = seed
    if cell_id.startswith("transpose-"):
        if cell_id.endswith("-vs-m4ri"):
            candidate = "m4ri-transpose"
        elif cell_id.endswith("-vs-bitshuffle"):
            candidate = "bitshuffle-transpose"
        elif cell_id.endswith("-vs-bitshuffle-adapter"):
            candidate = "bitshuffle-transpose"
            case["adapter"] = "padded"
        else:
            raise SystemExit(f"cell {cell_id!r} names no known transpose arm")
        return "gf2-transpose", candidate, case
    if cell_id.startswith("logical-") and cell_id.endswith("-vs-isal-base"):
        return "gf2-logical", "isal-base", case
    if cell_id.startswith("bch-genmatrix-") and cell_id.endswith("-vs-m4ri"):
        code = {15: "B1", 127: "B2", 255: "B3"}[size["n"]]
        route = "reference" if cell_id.startswith("bch-genmatrix-reference-") else "materialize"
        return "gf2-bch-genmatrix", "m4ri-bch-genmatrix", {"code": code, "route": route, "seed": seed}
    raise SystemExit(f"cell {cell_id!r} names no known arm pair")
cells, used = [], set()
for declared in addendum["cells"]:
    baseline, candidate, case = wiring(declared["cell_id"], declared["workload"]["size"], declared["workload"]["seed"])
    for name, build in ((baseline, declared["builds"]["baseline"]), (candidate, declared["builds"]["candidate"])):
        if ARMS[name]["build"] != build:
            raise SystemExit(f"cell {declared['cell_id']}: arm {name} is {ARMS[name]['build']}, addendum declares {build}")
    used.update((baseline, candidate))
    cells.append({"cell_id": declared["cell_id"], "baseline_arm": baseline, "candidate_arm": candidate, "case": case,
                  # Pilots use the largest pilot sample the protocol allows so
                  # the derived resolution reflects the confirmatory sample;
                  # confirmatory cells must leave this null.
                  "pilot_pairs": 24 if declared["role"] == "exploratory" else None})
plan = {
    "schema": "zen3-benchmark-plan-v1",
    "campaign_id": campaign,
    "issue": addendum["family"]["issue"],
    "label": label,
    "campaign_seed": {"pilot": 20260910, "confirmation": 20260911}[label],
    "addendum": addendum_path,
    "producing_manifest": "dev/active/6fb89a3c/producing-inputs.json",
    "lock_path": lock,
    "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
    "timing_override": None,
    "arms": {name: {"build": arm["build"], "description": arm["description"], "executable": arm["executable"],
                    "arguments": [], "environment": {}, "rustflags": arm["rustflags"], "tuning_profile": None}
             for name, arm in ARMS.items() if name in used},
    "cells": cells,
    "max_cells_per_session": 2,
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
print(f"plan: {len(cells)} cells, {len(plan['arms'])} arms -> {plan_path}")
PY_PLAN

{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD)"
  echo "# family: $FAMILY  mode: $MODE  run-id: $RUN_ID"
  echo "# addendum: $ADDENDUM  sha256: $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
  echo "# ledger: $LEDGER  sha256: $(sha256sum "$LEDGER" | cut -d' ' -f1)"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# plan: $PLAN  sha256: $(sha256sum "$PLAN" | cut -d' ' -f1)"
  echo "# external staging: $EXT"
  echo "# toolchain: $(rustc --version)  cc: $(gcc --version | head -1)"
  echo "# arm digests:"
  for arm in "$SURVEY"/m4ri_transpose_arm "$SURVEY"/m4ri_genmatrix_arm "$SURVEY"/bitshuffle_transpose_arm "$SURVEY"/isal_xor_arm \
             "$SURVEY"/gf2-side/target/release/gf2_transpose_arm "$SURVEY"/gf2-side/target/release/gf2_logical_xor_arm "$SURVEY"/gf2-side/target/release/gf2_bch_genmatrix_arm; do
    echo "#   $(sha256sum "$arm")"
  done
  echo "# session command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 RAYON_NUM_THREADS=1 dev/scripts/ccx1-bench-flock.sh --full-host $RUNNER run $STAGE $PLAN"
  echo "# load_avg_start: $(uptime)"
} >"$LAUNCH_LOG"

# Bounded checkpointed sessions: exit 3 means the session paused at the cell
# budget and the campaign resumes; exit 0 means the campaign is complete.
# CARGO_CI_NO_LOCK=1 is required because this process holds the exclusive side
# of the mutex that scripts/cargo-budget.sh takes shared.
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
echo "$FAMILY $MODE receipt: $OUT" >&2
exit "$verdict"
