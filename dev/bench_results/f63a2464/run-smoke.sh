#!/usr/bin/env bash
# Non-timed wire-contract smoke of this issue's arms (jit:f63a2464).
#
# Times nothing and publishes no receipt: every output lands under `target/`,
# which is not committed. The throwaway addendum keeps its family's declared id,
# so the plan resolves the same arms as the campaign, and redirects the ledger
# into `target/` so the family's own append-only ledger stays untouched.
#
# Two checks run over each throwaway plan. `ldpc-plan-check` decodes the plan
# strictly, validates the addendum against `addendum.schema.json` and the
# protocol's semantic rules, and validates the plan against the addendum: the
# checks the runner applies before opening a campaign. `qc-arm-smoke` then
# drives every arm of every cell with the runner's own case encoder, request
# sentinel and child environment in the `validation` role, so each arm performs
# one untimed dispatch, applies its placement and decision checks and returns no
# timing window. A campaign is queued only after both pass, because reading the
# arm sources does not establish the wire contract between the runner and a
# child.
#
# Usage (from the worktree root): run-smoke.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
SURVEY=dev/active/f63a2464/survey
SCRATCH=target/ldpc-qc-smoke  # repo-relative: the plan names the addendum by a literal relative path
BASELINE=$repo/target/ldpc-qc-baseline/release
CANDIDATE=$repo/target/ldpc-qc-arms/release
QUALITY=$repo/dev/bench_results/c077a88b/v3-preparation/quality

rm -rf "$SCRATCH"
mkdir -p "$SCRATCH"

for family in intra-frame-single-worker intra-frame-multicore comparator-single-worker; do
  python3 - "$family" "$SCRATCH" <<'PY'
import json, pathlib, sys

family, scratch = sys.argv[1], pathlib.Path(sys.argv[2])
source = pathlib.Path(f"dev/active/f63a2464/addendum-ldpc-qc-{family}-pilot.json")
addendum = json.loads(source.read_text())
addendum["family"]["description"] = (
    "Throwaway wire-contract smoke of the arms this issue measures. It publishes no receipt "
    "and supports no performance claim."
)
addendum["family_wise"]["ledger_path"] = str(scratch / f"{family}-ledger.jsonl")
path = scratch / f"{family}-addendum.json"
path.write_text(json.dumps(addendum, indent=2) + "\n")
pathlib.Path(addendum["family_wise"]["ledger_path"]).touch()
print(path)
PY
  python3 "$SURVEY/make-plan.py" --family "ldpc-qc-${family}-v1" --label pilot \
    --addendum "$SCRATCH/$family-addendum.json" \
    --baseline-dir "$BASELINE" --candidate-dir "$CANDIDATE" \
    --bundles-dir "$repo/target/ldpc-inputs" --quality-dir "$QUALITY" \
    --campaign-id "f63a2464-smoke-$family" --pilot-pairs 6 \
    --max-cells-per-session 1 --output "$SCRATCH/$family.plan.json"
  "$BASELINE/ldpc-plan-check" "$SCRATCH/$family.plan.json"
  "$CANDIDATE/qc-arm-smoke" "$SCRATCH/$family.plan.json"
done
echo "smoke complete: $SCRATCH"
