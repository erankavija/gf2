#!/usr/bin/env bash
# Wire-contract smoke of this issue's arms (jit:07ca8585). Times nothing that
# any conclusion rests on and publishes no receipt: every output lands under
# `target/`, which is not committed.
#
# The throwaway addendum keeps its family's declared id, so the plan resolves
# the same arms as the campaign, and redirects the ledger into `target/` so
# the family's own append-only ledger stays untouched.
#
# It runs the real `benchmark-ab-runner` over a throwaway plan at the protocol's
# smallest admissible pair count, whose cells cover every arm the campaigns use:
# the before and after gf2 arms in one cell, the after gf2 and AFF3CT throughput
# arms in another, the two isolated check-node arms in a third, and the
# throughput arms at a fixed iteration count against this issue's own prepared
# quality corpus in a fourth. A campaign is queued only after this passes,
# because reading the arm sources does not establish the wire contract between
# the runner and a child.
#
# Usage (from the worktree root): run-smoke.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
export PATH="$HOME/.cargo/bin:$PATH" RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1
SURVEY=dev/active/07ca8585/survey
SCRATCH=target/ldpc-update-smoke  # repo-relative: the runner snapshots the addendum by a literal relative path
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance

rm -rf "$SCRATCH"
mkdir -p "$SCRATCH"
./scripts/cargo-budget.sh cargo +1.95 build --offline --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance

for family in single-worker comparator-single-worker checknode fixed-iteration; do
  python3 - "$family" "$SCRATCH" <<'PY'
import json, pathlib, sys

family, scratch = sys.argv[1], pathlib.Path(sys.argv[2])
source = pathlib.Path(f"dev/active/07ca8585/addendum-ldpc-update-{family}-pilot.json")
addendum = json.loads(source.read_text())
addendum["family"]["description"] = (
    "Throwaway wire-contract smoke of the arms this issue measures. It publishes no receipt "
    "and supports no performance claim."
)
addendum["family_wise"]["ledger_path"] = str(scratch / f"{family}-ledger.jsonl")
addendum["cells"] = [cell for cell in addendum["cells"] if cell["cell_id"].startswith("dvb-t2-r12-")]
path = scratch / f"{family}-addendum.json"
path.write_text(json.dumps(addendum, indent=2) + "\n")
pathlib.Path(addendum["family_wise"]["ledger_path"]).touch()
print(path)
PY
  quality=$repo/dev/bench_results/c077a88b/v3-preparation/quality
  producing=dev/active/07ca8585/survey/producing-inputs.json
  case "$family" in
    fixed-iteration) quality=$repo/dev/bench_results/07ca8585/preparation/quality-fixed ;;
  esac
  case "$family" in
    checknode|fixed-iteration) producing=dev/active/07ca8585/survey/producing-inputs-kernel.json ;;
  esac
  python3 "$SURVEY/make-plan.py" --family "ldpc-update-${family}-v1" --label pilot \
    --addendum "$SCRATCH/$family-addendum.json" \
    --before-dir "$repo/target/ldpc-throughput-before/release" \
    --after-dir "$repo/target/ldpc-throughput/release" \
    --kernel-dir "$repo/target/ldpc-update-arms/release" \
    --bundles-dir "$repo/target/ldpc-inputs" \
    --quality-dir "$quality" --producing-manifest "$producing" \
    --campaign-id "07ca8585-smoke-$family" --pilot-pairs 6 \
    --max-cells-per-session 1 --output "$SCRATCH/$family.plan.json"
  CARGO_CI_NO_LOCK=1 GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$SCRATCH/$family-stage" "$SCRATCH/$family.plan.json"
  "$RUNNER" finalize "$SCRATCH/$family-stage" "$SCRATCH/$family-out"
  "$ACCEPTANCE" "$SCRATCH/$family-out"
done
echo "smoke complete: $SCRATCH"
