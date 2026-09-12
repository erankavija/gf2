#!/usr/bin/env bash
# Runs one bounded protocol campaign for jit:26465e6c.
#
#   dev/active/26465e6c/survey/run-campaign.sh <family> [date-utc]
#
# where <family> is one of
#
#   popcount-pilot      exploratory pilot for the popcount-baselines family
#   popcount            confirmation for the popcount-baselines family
#   and-popcnt-pilot    exploratory pilot for the and-popcnt-baselines family
#   and-popcnt          confirmation for the and-popcnt-baselines family
#
# The runner checkpoints and resumes, so re-invoking after an interrupted
# session continues the same campaign from its stage directory rather than
# repeating completed cells. Timed work runs under the CCX1 exclusive mutex
# through `dev/scripts/ccx1-bench-flock.sh --full-host`, and no cargo command
# runs inside that lock.
#
# The plan derives every cell's workload from the frozen addendum: words,
# word_offset and seed are read out of `workload.size` and `workload.seed`, and
# the arm pairing, pattern and operation come from the table below, which is
# checked against the addendum's declared cell set in both directions. A cell
# the addendum does not declare, or a declared cell the table does not cover,
# fails the launch.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKTREE="$(git -C "${HERE}" rev-parse --show-toplevel)"
cd "${WORKTREE}"
ISSUE=26465e6c
FAMILY="${1:?usage: run-campaign.sh <popcount-pilot|popcount|and-popcnt-pilot|and-popcnt> [date-utc]}"
DATE_UTC="${2:-$(date -u +%Y-%m-%d)}"

case "${FAMILY}" in
  popcount-pilot)   ADDENDUM=dev/active/${ISSUE}/addendum-popcount-pilot.json;   LABEL=pilot ;;
  popcount)         ADDENDUM=dev/active/${ISSUE}/addendum-popcount.json;         LABEL=confirmation ;;
  and-popcnt-pilot) ADDENDUM=dev/active/${ISSUE}/addendum-and-popcnt-pilot.json; LABEL=pilot ;;
  and-popcnt)       ADDENDUM=dev/active/${ISSUE}/addendum-and-popcnt.json;       LABEL=confirmation ;;
  *) echo "unknown family ${FAMILY}" >&2; exit 2 ;;
esac

OUT="dev/bench_results/${ISSUE}/${DATE_UTC}-${ISSUE}-${FAMILY}"
if [[ -e "${OUT}" ]]; then
  echo "receipt directory ${OUT} already exists; remove it to re-run this family" >&2
  exit 2
fi
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)

# The runner records the absolute path of every arm executable it launches, and
# that record lands in the committed receipt. Staging the arms outside the
# checkout keeps a `git worktree` path from being frozen into a receipt that
# outlives the worktree. `.agents/ext` is untracked and lives beside the main
# checkout, where fetch-build.sh already keeps this issue's external material.
# Acceptance identifies an arm by `executable_sha256`, and the staged copy has
# the same digest as the built binary, which the copy loop asserts.
REPO="$(dirname "$(git -C "${HERE}" rev-parse --path-format=absolute --git-common-dir)")"
ARM_BIN="${GF2_SURVEY_ARM_BIN:-${REPO}/.agents/ext/${ISSUE}/bin}"
mkdir -p "${ARM_BIN}"
stage_arm() {
  local source="$1" name staged
  name=$(basename "${source}")
  staged="${ARM_BIN}/${name}"
  [[ -x "${source}" ]] || { echo "missing arm binary: ${source}" >&2; exit 2; }
  install -m 0755 "${source}" "${staged}"
  if [[ "$(sha256sum "${source}" | cut -d' ' -f1)" != "$(sha256sum "${staged}" | cut -d' ' -f1)" ]]; then
    echo "staged ${name} does not match the built binary" >&2
    exit 1
  fi
  echo "${staged}"
}
GF2SIDE=$(stage_arm "${HERE}/gf2-side/target/release/popcount-gf2-side")
LIBPOPCNT=$(stage_arm "${HERE}/libpopcnt-arm")
MULA=$(stage_arm "${HERE}/mula-avx2-harleyseal-arm")
[[ -x "${RUNNER}" && -x "${ACCEPTANCE}" ]] || {
  echo "build the protocol tools before a timed run" >&2
  exit 2
}

# The campaign identity is stable for a family and date, so an interrupted
# session resumes into the same stage rather than starting a fresh campaign.
CAMPAIGN="${FAMILY}-${ISSUE}-${DATE_UTC//-/}"
STAGE=/tmp/gf2-${CAMPAIGN}
PLAN=/tmp/gf2-${CAMPAIGN}.plan.json
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "${LOCK}"
LOCK=$(realpath "${LOCK}")
export GF2_CCX1_LOCK="${LOCK}"

python3 - "${PLAN}" "${CAMPAIGN}" "${LABEL}" "${ADDENDUM}" "${GF2SIDE}" "${LIBPOPCNT}" "${MULA}" "${LOCK}" <<'PY_PLAN'
import json, sys

plan_path, campaign, label, addendum_path, gf2side, libpopcnt, mula, lock = sys.argv[1:]
addendum = json.load(open(addendum_path))

def gf2_arm(build, arm_env, description):
    return {"build": build, "description": description, "executable": gf2side,
            "arguments": [], "environment": {"GF2_POPCOUNT_ARM": arm_env},
            "rustflags": None, "tuning_profile": None}

def external_arm(executable, description):
    return {"build": "external", "description": description, "executable": executable,
            "arguments": [], "environment": {}, "rustflags": None, "tuning_profile": None}

# Every arm this issue declares. `build` is the arm's instruction-selection
# regime, not a separate compiler invocation: the one baseline-x86-64 gf2-side
# binary reaches `native` paths through runtime detection, `tuned-portable`
# through a #[target_feature] opt-in, and `conservative-portable` through no
# opt-in at all. objdump-evidence.txt records the instructions each emits.
ARMS = {
    "baseline": gf2_arm("native", "production-dispatch",
                        "gf2 production popcount dispatcher, kernels::ops::popcount"),
    "nibble-lut": gf2_arm("native", "nibble-lut",
                          "gf2 AVX2 nibble-LUT kernel called directly, bypassing the size threshold"),
    "scalar-popcnt": gf2_arm("tuned-portable", "scalar-popcnt",
                             "internal control: forced POPCNT-instruction scalar loop"),
    "compiler-count-ones": gf2_arm("conservative-portable", "compiler-count-ones",
                                   "internal control: portable u64::count_ones loop, no target feature"),
    "libpopcnt": external_arm(libpopcnt, "libpopcnt v4.2 with its own CPUID dispatch"),
    "mula-avx2-harleyseal": external_arm(mula, "Mula AVX2 Harley-Seal carry-save reference"),
    "and-fused": gf2_arm("native", "and-popcnt-fused",
                         "gf2 fused AVX2 AND+popcount kernel, called directly"),
    "and-scalar-control": gf2_arm("conservative-portable", "and-popcnt-scalar-control",
                                  "internal control: single-pass scalar AND+popcount loop"),
    "and-two-pass": gf2_arm("native", "and-popcnt-two-pass-consumer",
                            "public-API two-pass model: and_inplace into a fresh buffer, then popcount"),
}

# Arm pairing, bit pattern and operation for each declared cell. Sizes, word
# offsets and seeds are never repeated here: they come from the addendum.
CELLS = {
    # popcount-baselines-pilot
    "popcount-pilot-small-vs-nibble-lut": ("baseline", "nibble-lut", "random", "popcount"),
    "popcount-pilot-alignment-vs-compiler-count-ones": ("baseline", "compiler-count-ones", "random", "popcount"),
    "popcount-pilot-streaming-vs-mula": ("baseline", "mula-avx2-harleyseal", "random", "popcount"),
    # popcount-baselines
    "popcount-small-vs-nibble-lut": ("baseline", "nibble-lut", "random", "popcount"),
    "popcount-boundary-vs-scalar-popcnt": ("baseline", "scalar-popcnt", "random", "popcount"),
    "popcount-alignment-vs-compiler-count-ones": ("baseline", "compiler-count-ones", "random", "popcount"),
    "popcount-bitpattern-allones-vs-libpopcnt": ("baseline", "libpopcnt", "all_one", "popcount"),
    "popcount-bitpattern-allzero-vs-mula": ("baseline", "mula-avx2-harleyseal", "all_zero", "popcount"),
    "popcount-csa-boundary-below-vs-mula": ("baseline", "mula-avx2-harleyseal", "random", "popcount"),
    "popcount-csa-boundary-at-vs-mula": ("baseline", "mula-avx2-harleyseal", "random", "popcount"),
    "popcount-cache-resident-vs-libpopcnt": ("baseline", "libpopcnt", "random", "popcount"),
    "popcount-streaming-vs-mula": ("baseline", "mula-avx2-harleyseal", "random", "popcount"),
    # and-popcnt-baselines-pilot
    "and-popcnt-pilot-cache-resident": ("and-fused", "and-scalar-control", "random", "and_popcnt"),
    "and-popcnt-pilot-streaming": ("and-fused", "and-scalar-control", "random", "and_popcnt"),
    "and-popcnt-pilot-two-pass-consumer": ("and-fused", "and-two-pass", "random", "and_popcnt_two_pass"),
    # and-popcnt-baselines
    "and-popcnt-cache-resident-vs-scalar-control": ("and-fused", "and-scalar-control", "random", "and_popcnt"),
    "and-popcnt-streaming-vs-scalar-control": ("and-fused", "and-scalar-control", "random", "and_popcnt"),
    "and-popcnt-fused-vs-two-pass-consumer": ("and-fused", "and-two-pass", "random", "and_popcnt_two_pass"),
}

# The pattern token every workload identity must carry, so a cell cannot claim
# one bit pattern in the addendum and measure another.
IDENTITY_TOKEN = {"random": "random", "all_one": "allones", "all_zero": "allzero"}

cells, used_arms = [], set()
for declared in addendum["cells"]:
    cell_id = declared["cell_id"]
    if cell_id not in CELLS:
        raise SystemExit(f"addendum cell {cell_id!r} has no plan entry")
    baseline, candidate, pattern, op = CELLS[cell_id]
    identity = declared["workload"]["identity"]
    if IDENTITY_TOKEN[pattern] not in identity:
        raise SystemExit(f"cell {cell_id!r} plans pattern {pattern!r}, identity {identity!r} does not say so")
    size = declared["workload"]["size"]
    words = size["words"]
    word_offset = size.get("word_offset", 0)
    seed = declared["workload"]["seed"]
    if op == "popcount":
        case = {"op": op, "words": words, "seed": seed, "pattern": pattern, "word_offset": word_offset}
    else:
        # The fused operations need two independent buffers. The right-hand
        # seed is the declared seed offset by a fixed constant, so one addendum
        # field still determines the whole case.
        case = {"op": op, "words": words, "seed_lhs": seed, "seed_rhs": seed + 1000,
                "pattern": pattern, "word_offset": word_offset}
    entry = {"cell_id": cell_id, "baseline_arm": baseline, "candidate_arm": candidate, "case": case}
    if declared["role"] == "exploratory":
        entry["pilot_pairs"] = 6
    else:
        entry["pilot_pairs"] = None
    cells.append(entry)
    used_arms.update((baseline, candidate))

plan = {
    "schema": "zen3-benchmark-plan-v1",
    "campaign_id": campaign,
    "issue": "26465e6c",
    "label": label,
    "campaign_seed": 20260908,
    "addendum": addendum_path,
    "lock_path": lock,
    "wrapper": "dev/scripts/ccx1-bench-flock.sh",
    "timing_override": None,
    "arms": {name: ARMS[name] for name in sorted(used_arms)},
    "cells": cells,
    "max_cells_per_session": None,
}
with open(plan_path, "w") as output:
    json.dump(plan, output, indent=2)
    output.write("\n")
print(f"planned {len(cells)} cells over {len(used_arms)} arms", file=sys.stderr)
PY_PLAN

# A plan-only launch validates the plan against the addendum and the built
# arms without taking the timing lock, so a launch error costs no lock time.
if [[ -n "${GF2_PLAN_ONLY:-}" ]]; then
  python3 -m json.tool "${PLAN}" >/dev/null
  echo "plan written to ${PLAN}" >&2
  exit 0
fi

LAUNCH_LOG="${STAGE}.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# addendum: ${ADDENDUM} sha256=$(sha256sum "${ADDENDUM}" | cut -d' ' -f1)"
  echo "# campaign: ${CAMPAIGN}"
  echo "# stage: ${STAGE}"
  echo "# arm executables:"
  for binary in "${GF2SIDE}" "${LIBPOPCNT}" "${MULA}"; do
    echo "#   $(basename "${binary}") sha256=$(sha256sum "${binary}" | cut -d' ' -f1)"
  done
  echo "# load_avg_start: $(uptime)"
} >"${LAUNCH_LOG}"

# `--full-host` holds the CCX1 mutex exclusively; CARGO_CI_NO_LOCK keeps any
# cargo work inside it from deadlocking against the shared side.
set +e
GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
  "${RUNNER}" run "${STAGE}" "${PLAN}" | tee -a "${LAUNCH_LOG}"
verdict=${PIPESTATUS[0]}
set -e
echo "# session exit: ${verdict}" >>"${LAUNCH_LOG}"
if [[ ${verdict} -ne 0 ]]; then
  echo "runner session exited ${verdict}; re-invoke to resume the campaign" >&2
  exit 1
fi

"${RUNNER}" finalize "${STAGE}" "${OUT}" | tee -a "${LAUNCH_LOG}"
cp "${LAUNCH_LOG}" "${OUT}/launcher.log"
LAUNCH_LOG="${OUT}/launcher.log"
set +e
"${ACCEPTANCE}" "${OUT}" | tee -a "${LAUNCH_LOG}"
accept_verdict=${PIPESTATUS[0]}
set -e
{
  echo "# acceptance exit: ${accept_verdict}"
  echo "# arm provenance:"
  "${HERE}/record-provenance.sh" verify "${OUT}" 2>&1 | sed 's/^/#   /'
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"${LAUNCH_LOG}"
echo "${FAMILY} receipt: ${OUT}" >&2
exit "${accept_verdict}"
