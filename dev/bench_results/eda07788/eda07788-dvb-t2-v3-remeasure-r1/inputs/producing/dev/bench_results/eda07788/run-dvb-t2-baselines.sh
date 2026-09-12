#!/usr/bin/env bash
# DVB-T2 bit-interleaver external-baseline campaign (jit:eda07788, jit:3e59cb9a).
#
# Usage:
#   dev/bench_results/eda07788/run-dvb-t2-baselines.sh pilot-v3|pilot-v3-r2|confirmation-v3 [date-utc]
#   dev/bench_results/eda07788/run-dvb-t2-baselines.sh preflight-remeasure
#   dev/bench_results/eda07788/run-dvb-t2-baselines.sh remeasure-v3 [run-id]
#
# The pilot and confirmation modes build the three arm executables through the
# committed fetch/build path, build the protocol runner and acceptance tool
# under `--release`, project a `zen3-benchmark-plan-v1` from the frozen family
# addendum, then measure it as a sequence of bounded sessions under the
# canonical CCX1 mutex. Each session takes the lock, measures at most
# `max_cells_per_session` cells, checkpoints and releases, so a queued sibling
# worker gets the host between sessions and a killed session resumes without
# repeating a completed cell.
#
# Timing and statistical settings come from the addendum and from the
# protocol's frozen shared settings. The script adds only the plan's session
# cell budget, pilot pair count and campaign seed, which the plan records. The
# cell-to-arm wiring is derived from the cell identifiers, which name the two
# arms they compare.
#
# The pilot must be published, and its digest placed in the confirmatory
# addendum, before the confirmation runs.
#
# Every receipt of those modes ran arms whose `warm` cells skipped the
# protocol's untimed pass over the working set before calibration. The
# re-measurement uses the repaired arms in two steps. `preflight-remeasure`
# builds them through fetch-build.sh, which also runs the adapter gate, records
# that output with the arm digests in
# dev/active/eda07788/survey/validation-output-v3-remeasure.txt, derives the
# re-measurement's producing manifest and builds the runner; its outputs are
# committed before any timed work. `remeasure-v3` runs the committed
# exploratory addendum addendum-dvb-t2-bit-interleave-v3-remeasure.json
# (receipt label `pilot`, the protocol's pilot maximum of 24 pairs per cell,
# its own campaign seed) with exactly the recorded executables: it builds only
# the runner and refuses an arm whose digest differs from the record.
#
# Re-running the same `remeasure-v3` command resumes the same campaign
# identity: it re-projects the plan and continues only when the projection
# equals the staged plan byte for byte, a stage whose execution log already
# ends `complete` is only finalized, and a finalized receipt is only
# re-evaluated, so no cell is measured twice. The launcher log is appended to
# on every invocation.
set -euo pipefail
# The benchmark window starts this from a non-interactive shell; rustup's
# cargo and rustc proxies live in ~/.cargo/bin.
command -v cargo >/dev/null 2>&1 || PATH="$HOME/.cargo/bin:$PATH"
repo=$(git rev-parse --show-toplevel)
cd "$repo"
ISSUE=eda07788
SURVEY=dev/active/$ISSUE/survey
LAUNCHER=dev/bench_results/$ISSUE/run-dvb-t2-baselines.sh
EXT=${GF2_EDA07788_EXT:-$repo/.agents/ext/eda07788}
PRODUCING=dev/active/$ISSUE/producing-inputs.json
REMEASURE_PRODUCING=dev/active/$ISSUE/producing-inputs-dvb-t2-remeasure.json
REMEASURE_VALIDATION=$SURVEY/validation-output-v3-remeasure.txt
REMEASURE_FREEZE=$SURVEY/freeze-remeasure.py
SEED=20260908
MODE=${1:-}
USAGE="usage: $0 pilot-v3|pilot-v3-r2|confirmation-v3 [date-utc] | preflight-remeasure | remeasure-v3 [run-id]"

if [[ "$MODE" == preflight-remeasure ]]; then
  # No compiler-cache server may start under the CCX1 lock and keep it.
  export CARGO_CI_NO_SCCACHE=1
  "$SURVEY/fetch-build.sh" "$EXT" 2>&1 | tee "$REMEASURE_VALIDATION"
  ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  # The re-measurement's producing closure is the v3 closure plus the
  # committed arm-digest record the launcher checks and the addendum's
  # freeze script.
  python3 - "$PRODUCING" "$REMEASURE_PRODUCING" "$REMEASURE_VALIDATION" "$REMEASURE_FREEZE" <<'PY_PRODUCING'
import json, sys
source, output, validation, freeze = sys.argv[1:]
manifest = json.load(open(source))
manifest["behavior_sources"] = sorted(set(manifest["behavior_sources"]) | {freeze})
manifest["build_inputs"] = sorted(set(manifest["build_inputs"]) | {freeze, validation})
with open(output, "w") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
print(f"producing manifest: {output}")
PY_PRODUCING
  exit 0
fi

case "$MODE" in
  pilot-v3)
    LABEL=pilot
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-pilot.json
    OUT="dev/bench_results/$ISSUE/${2:-$(date -u +%Y-%m-%d)}-$ISSUE-dvb-t2-v3-pilot"
    MAX_CELLS=5
    PILOT_PAIRS=
    ;;
  pilot-v3-r2)
    LABEL=pilot
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-pilot-r2.json
    OUT="dev/bench_results/$ISSUE/${2:-$(date -u +%Y-%m-%d)}-$ISSUE-dvb-t2-v3-pilot-r2"
    MAX_CELLS=1
    PILOT_PAIRS=24
    ;;
  confirmation-v3)
    LABEL=confirmation
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-confirmation.json
    OUT="dev/bench_results/$ISSUE/${2:-$(date -u +%Y-%m-%d)}-$ISSUE-dvb-t2-v3-confirmation"
    MAX_CELLS=2
    PILOT_PAIRS=
    ;;
  remeasure-v3)
    LABEL=pilot
    ADDENDUM=dev/active/eda07788/addendum-dvb-t2-bit-interleave-v3-remeasure.json
    RUN_ID=${2:-r1}
    OUT="dev/bench_results/$ISSUE/$ISSUE-dvb-t2-v3-remeasure-$RUN_ID"
    MAX_CELLS=2
    PILOT_PAIRS=24
    SEED=20260912
    PRODUCING=$REMEASURE_PRODUCING
    ;;
  *)
    echo "$USAGE" >&2
    exit 2
    ;;
esac
if [[ "$MODE" == confirmation-v3 ]] && \
   ! grep -Eq '"sha256": "[0-9a-f]{64}"' "$ADDENDUM"; then
  echo 'confirmation addendum does not identify a pilot receipt digest' >&2
  exit 2
fi
if [[ -e "$OUT" && ( "$MODE" != remeasure-v3 || ! -f "$OUT/receipt.json" ) ]]; then
  echo "receipt directory $OUT already exists; inspect it before re-running" >&2
  exit 2
fi
LEDGER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["family_wise"]["ledger_path"])' "$ADDENDUM")

# Builds and the adapter correctness gate finish before any timed work.
if [[ "$MODE" == remeasure-v3 ]]; then
  export CARGO_CI_NO_SCCACHE=1
  # Publication precedes measurement: everything the campaign pins is
  # committed bytes.
  for committed in "$ADDENDUM" "$PRODUCING" "$REMEASURE_VALIDATION" "$REMEASURE_FREEZE" "$LAUNCHER" "$LEDGER"; do
    git ls-files --error-unmatch "$committed" >/dev/null
  done
  for committed in "$ADDENDUM" "$PRODUCING" "$REMEASURE_VALIDATION" "$REMEASURE_FREEZE" "$LAUNCHER"; do
    git diff --quiet HEAD -- "$committed" || { echo "$committed differs from HEAD" >&2; exit 2; }
  done
  EXT=$(realpath "$EXT")
  python3 - "$EXT" "$REMEASURE_VALIDATION" <<'PY_ARMS'
import hashlib, pathlib, re, sys
ext, validation = pathlib.Path(sys.argv[1]), sys.argv[2]
recorded = [re.fullmatch(r"([0-9a-f]{64})  (target-[a-z]+/release/[a-z0-9-]+)", line)
            for line in open(validation).read().splitlines()]
recorded = {match.group(2): match.group(1) for match in recorded if match}
if len(recorded) != 3:
    raise SystemExit(f"{validation} lists {len(recorded)} arm digests, expected 3")
for path, digest in sorted(recorded.items()):
    if hashlib.sha256((ext / path).read_bytes()).hexdigest() != digest:
        raise SystemExit(f"{ext / path} differs from {validation}; run preflight-remeasure and commit its output")
print(f"arm executables match {validation}")
PY_ARMS
else
  dev/active/eda07788/survey/fetch-build.sh "$EXT"
  EXT=$(realpath "$EXT")
fi
./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
RUNNER=$(realpath target/release/benchmark-ab-runner)
ACCEPTANCE=$(realpath target/release/benchmark-acceptance)
GF2_NATIVE=$(realpath "$EXT/target-native/release/gf2-dvb-t2-candidate")
GF2_PORTABLE=$(realpath "$EXT/target-portable/release/gf2-dvb-t2-candidate")
XDSOPL=$(realpath "$EXT/target-native/release/xdsopl-dvb-t2-baseline")

evaluate() {
  set +e
  "$ACCEPTANCE" "$OUT" | tee -a "$LAUNCH_LOG"
  verdict=${PIPESTATUS[0]}
  set -e
}

if [[ -e "$OUT" ]]; then
  # Finalized by an earlier invocation: evaluate again, measure nothing.
  LAUNCH_LOG="$OUT/launcher.log"
  echo "# re-evaluation_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)  command: $0 $*" >>"$LAUNCH_LOG"
  evaluate
  echo "# acceptance exit: $verdict" >>"$LAUNCH_LOG"
  echo "$MODE receipt: $OUT" >&2
  exit "$verdict"
fi

if [[ "$MODE" == remeasure-v3 ]]; then
  CAMPAIGN="remeasure-v3-$ISSUE-$RUN_ID"
  STAGE="$repo/target/$ISSUE-campaigns/$CAMPAIGN"
  mkdir -p "$(dirname "$STAGE")"
else
  CAMPAIGN="$MODE-$ISSUE-$(date -u +%Y%m%dt%H%M%Sz)"
  STAGE=/tmp/gf2-$CAMPAIGN
fi
PLAN=$STAGE.plan.json
LAUNCH_LOG="$STAGE.launcher.log"
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$LOCK"
LOCK=$(realpath "$LOCK")
export GF2_CCX1_LOCK="$LOCK"
python3 - "$PLAN.projected" "$CAMPAIGN" "$LABEL" "$ADDENDUM" "$LOCK" "$MAX_CELLS" "$PILOT_PAIRS" \
  "$SEED" "$PRODUCING" "$GF2_NATIVE" "$GF2_PORTABLE" "$XDSOPL" <<'PY_PLAN'
import json, sys

(plan_path, campaign, label, addendum_path, lock, max_cells, pilot_pairs, seed,
 producing, gf2_native, gf2_portable, xdsopl) = sys.argv[1:]

with open(addendum_path) as handle:
    addendum = json.load(handle)

NATIVE_RUSTFLAGS = "-C target-cpu=native"
PORTABLE_RUSTFLAGS = "-C target-cpu=x86-64"
WARM = "; warm cells run the timed call once, untimed, before calibration"
ARMS = {
    "gf2-native": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "description": "gf2 DvbT2BitInterleaver::interleave on a packed BitVec, built with -C target-cpu=native" + WARM,
    },
    "gf2-native-control": {
        "build": "native", "executable": gf2_native, "rustflags": NATIVE_RUSTFLAGS,
        "description": "identity control: the same gf2 native executable as the baseline arm, launched as a second arm" + WARM,
    },
    "gf2-portable": {
        "build": "conservative-portable", "executable": gf2_portable, "rustflags": PORTABLE_RUSTFLAGS,
        "description": "scalar/compiler control: gf2 DvbT2BitInterleaver::interleave built with -C target-cpu=x86-64 (baseline x86-64, no target feature beyond SSE2)" + WARM,
    },
    "xdsopl-external": {
        "build": "external", "executable": xdsopl, "rustflags": NATIVE_RUSTFLAGS,
        "description": "xdsopl/LDPC PCTITL template at commit 32357d8ad55a6a302c34e093759f0454e45cca56, C++ shim built with g++ -O3 -march=native -std=c++17; the timed call unpacks the packed frame to one int32 per bit, applies PCTITL::fwd and packs the result back" + WARM,
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
    "campaign_seed": int(seed),
    "addendum": addendum_path,
    "producing_manifest": producing,
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
if [[ -e "$PLAN" ]]; then
  if ! cmp -s "$PLAN.projected" "$PLAN"; then
    rm -f "$PLAN.projected"
    echo "plan $PLAN differs from the projection of the current inputs; a resume needs the identical plan" >&2
    exit 2
  fi
  rm -f "$PLAN.projected"
  INVOCATION=resume
else
  # A new campaign reserves on top of the committed ledger only.
  git diff --quiet HEAD -- "$LEDGER" || { echo "$LEDGER differs from HEAD before a new campaign" >&2; exit 2; }
  mv "$PLAN.projected" "$PLAN"
  INVOCATION=new
fi

stage_complete() {
  [[ -f "$STAGE/execution.log" ]] && python3 - "$STAGE/execution.log" <<'PY_TERMINAL'
import json, sys
events = [json.loads(line)["event"] for line in open(sys.argv[1])]
terminal = [e for e in events if e in ("complete", "failed", "paused", "budget-exhausted")]
sys.exit(0 if terminal and terminal[-1] == "complete" else 1)
PY_TERMINAL
}

{
  echo "# command: $0 $*"
  echo "# invocation: $INVOCATION"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# gf2 revision (informational): $(git rev-parse HEAD 2>/dev/null || true)"
  echo "# mode: $MODE"
  echo "# addendum: $ADDENDUM"
  echo "# addendum sha256: $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
  echo "# ledger: $LEDGER  sha256: $(sha256sum "$LEDGER" | cut -d' ' -f1)"
  echo "# producing manifest: $PRODUCING  sha256: $(sha256sum "$PRODUCING" | cut -d' ' -f1)"
  echo "# campaign: $CAMPAIGN"
  echo "# stage: $STAGE"
  echo "# plan: $PLAN  sha256: $(sha256sum "$PLAN" | cut -d' ' -f1)"
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
                "$LAUNCHER"; do
    echo "#   $(sha256sum "$source")"
  done
  echo "# session command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host $RUNNER run $STAGE $PLAN"
  echo "# load_avg_start: $(uptime)"
} >>"$LAUNCH_LOG"

# Bounded checkpointed sessions: exit 3 means the session paused at the cell
# budget and the campaign resumes; exit 0 means the campaign is complete.
# `CARGO_CI_NO_LOCK=1` is set for anything the session might shell out to,
# because this process holds the exclusive side of the same mutex that
# `scripts/cargo-budget.sh` takes shared.
session=0
while ! stage_complete; do
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
evaluate
{
  echo "# acceptance exit: $verdict"
  echo "# sessions this invocation: $session"
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$LAUNCH_LOG"
echo "$MODE receipt: $OUT" >&2
exit "$verdict"
