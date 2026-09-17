#!/usr/bin/env bash
# Carry both residual-shift arms through the real runner before the profile is
# queued (jit:85fc5ff4).
#
# A campaign that reaches the benchmark window and dies on its first arm spends
# the window and measures nothing. Reading the code does not establish the wire
# contract; this script does, because it is the same `benchmark-ab-runner`
# binary, the same plan projector, the same arm executable and the same
# canonical child-v2 framing the queued profile uses.
#
# It measures a throwaway family on its own throwaway ledger under `target/`,
# at the smallest pair count the protocol allows, over two cells that between
# them name both arms, both shift directions and both cache states the frozen
# addendum declares:
#
#   left-small-r1-w64        warm       residual offset 1
#   right-streaming-r65-w64  streaming  residual offset 65
#
# Nothing it measures is evidence: the stage, the receipt and the ledger live
# in `target/` and are rebuilt on every invocation, the run takes no host
# mutex, and its durations decide nothing. The committed artifact is the
# structural summary `shift-profile-smoke.json`, which records only what the
# receipt and the execution log state about the handshake.
#
# Usage (from the repository root):
#   dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/smoke-shift-arms.sh [--check]
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
export PATH="$HOME/.cargo/bin:$PATH"
export RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1

mode=${1:-}
case "$mode" in
  ""|--check) ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

active=dev/active/c04dd4ac-zen3-shifts-and-permutations
survey="$active/survey"
summary="$active/shift-profile-smoke.json"
smoke=target/85fc5ff4-campaigns/arms-smoke
build_dir=target/85fc5ff4-build

# The smoked executable is the queued executable: `build` freezes the addendum
# check, the semantic oracle and the plan, and resolves the arm the window job
# resolves.
"$survey/run-shift-profile.sh" build >/dev/null
arm=$(python3 "$survey/find-shift-executable.py" "$build_dir/shifts-build.jsonl")
runner=$(realpath target/release/benchmark-ab-runner)

rm -rf "$smoke"
mkdir -p "$smoke"
: >"$smoke/ledger.jsonl"
: >"$smoke/lock"

python3 - "$active/shift-profile-addendum.json" "$smoke" <<'PY'
import json, sys

source, smoke = sys.argv[1], sys.argv[2]
addendum = json.load(open(source))
keep = {"left-small-r1-w64", "right-streaming-r65-w64"}
addendum["family"]["id"] = "bitvec-residual-shift-arms-smoke"
addendum["family"]["description"] = (
    "Throwaway wire smoke of issue 85fc5ff4: both residual-shift arms, both "
    "directions and both cache states the frozen profile declares, at the "
    "smallest pair count the protocol allows, on a throwaway ledger under "
    "target/. It decides nothing and no receipt cites it.")
addendum["family_wise"]["ledger_path"] = f"{smoke}/ledger.jsonl"
addendum["cells"] = [cell for cell in addendum["cells"] if cell["cell_id"] in keep]
json.dump(addendum, open(f"{smoke}/addendum.json", "w"), indent=2)
PY

python3 "$survey/make-shift-plan.py" "$smoke/plan.json" "$arm" "$(realpath "$smoke/lock")" \
  --addendum "$smoke/addendum.json" --label smoke \
  --campaign-id 85fc5ff4-arms-smoke --max-cells-per-session 2

# The smoke takes no host mutex: it holds a throwaway descriptor its own plan
# names, which is what `host::inherited_lock` requires and what keeps the run
# outside the benchmark window's serialization domain.
flock -x "$smoke/lock" "$runner" run "$smoke/stage" "$smoke/plan.json"
"$runner" finalize "$smoke/stage" "$smoke/receipt"

python3 "$survey/summarize-shift-smoke.py" \
  --receipt "$smoke/receipt/receipt.json" --log "$smoke/stage/execution.log" \
  --arm "$arm" --runner "$runner" --output "$summary" ${mode:+--check}
