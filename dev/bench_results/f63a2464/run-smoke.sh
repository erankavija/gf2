#!/usr/bin/env bash
# Non-timed wire-contract smoke of this issue's arms (jit:f63a2464).
#
# Times nothing and publishes no receipt: every output lands under `target/`,
# which is not committed. The throwaway addendum keeps its family's declared id,
# so the plan resolves the same arms as the campaign, and redirects the ledger
# into `target/` so the family's own append-only ledger stays untouched.
#
# Two shared checks run over each throwaway plan. `benchmark-ab-runner check`
# applies the decode and validation the runner applies before its first
# measurement. `benchmark-ab-runner smoke`, whose contract
# `tuning_campaign_support::arm::smoke` states, then drives every arm of every
# cell once in the untimed validation position and fails an arm that reports a
# timing window. A campaign is queued only after both pass, because reading the
# arm sources does not establish the wire contract between the runner and a
# child.
#
# Both stages are smoked. A confirmation plan declares the confirmatory role
# and the protocol's confirmatory pair count, so its arms answer a different
# request than a pilot's; queueing it on the pilot's smoke would leave that
# wire contract unproven.
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
RUNNER=$repo/target/release/benchmark-ab-runner

./scripts/cargo-budget.sh cargo +1.95 build --offline --release -p tuning-campaign-support \
  --bin benchmark-ab-runner >/dev/null
rm -rf "$SCRATCH"
mkdir -p "$SCRATCH"

for family in intra-frame-single-worker intra-frame-multicore comparator-single-worker; do
 for mode in pilot confirmation; do
  python3 - "$family" "$mode" "$SCRATCH" <<'PY'
import json, pathlib, sys

family, mode, scratch = sys.argv[1], sys.argv[2], pathlib.Path(sys.argv[3])
suffix = "-pilot" if mode == "pilot" else ""
source = pathlib.Path(f"dev/active/f63a2464/addendum-ldpc-qc-{family}{suffix}.json")
addendum = json.loads(source.read_text())
addendum["family"]["description"] = (
    "Throwaway wire-contract smoke of the arms this issue measures. It publishes no receipt "
    "and supports no performance claim."
)
addendum["family_wise"]["ledger_path"] = str(scratch / f"{family}-{mode}-ledger.jsonl")
path = scratch / f"{family}-{mode}-addendum.json"
path.write_text(json.dumps(addendum, indent=2) + "\n")
pathlib.Path(addendum["family_wise"]["ledger_path"]).touch()
print(path)
PY
  python3 "$SURVEY/make-plan.py" --family "ldpc-qc-${family}-v1" --label "$mode" \
    --addendum "$SCRATCH/$family-$mode-addendum.json" \
    --baseline-dir "$BASELINE" --candidate-dir "$CANDIDATE" \
    --bundles-dir "$repo/target/ldpc-inputs" --quality-dir "$QUALITY" \
    --campaign-id "f63a2464-smoke-$family-$mode" --pilot-pairs 6 \
    --max-cells-per-session 1 --output "$SCRATCH/$family-$mode.plan.json"
  "$RUNNER" check "$SCRATCH/$family-$mode.plan.json"
  "$RUNNER" smoke "$SCRATCH/$family-$mode.plan.json" --record "$SCRATCH/$family-$mode.smoke.json"
 done
done
echo "smoke complete: $SCRATCH"
