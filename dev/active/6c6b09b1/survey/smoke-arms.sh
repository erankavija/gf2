#!/usr/bin/env bash
# Carry every byte-field arm through the real runner before a campaign is
# queued (jit:6c6b09b1).
#
# A campaign that reaches the benchmark window and dies on its first arm
# spends the window and measures nothing: the version-3 attempts of this issue
# ended that way, in thirteen milliseconds, on a request field the arms and the
# runner spelled differently. Reading the code did not catch it. This script
# does, because it is the same `benchmark-ab-runner` binary, the same plan
# projector, the same arm executables and the same wire the campaigns use.
#
# It measures a throwaway family on its own throwaway ledger under `target/`,
# at the smallest pair count the protocol allows, over four cells that between
# them name all six arms and all four case operations:
#
#   element  vs isal       region multiply-accumulate
#   wide     vs m4rie      square product
#   batch    vs gfcomplete pairwise product
#   element  vs isal       generator encode
#
# Nothing it writes is evidence: the stage, the receipt and the ledger live in
# `target/` and the run is deleted and rebuilt on every invocation. It is a
# smoke test of the wire, not a measurement, and its numbers decide nothing.
#
# Usage (from the repository root): survey/smoke-arms.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=6c6b09b1
SURVEY=dev/active/$ISSUE/survey
TARGET=$repo/target/$ISSUE-survey
RUNNER=$repo/target/release/benchmark-ab-runner
# The addendum and the ledger are artifacts the receipt pins by content, and a
# pin names a repository-relative path, so the smoke keeps both relative.
SMOKE=target/$ISSUE-campaigns/arms-smoke
LEDGER=$SMOKE/ledger.jsonl
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
export RUSTUP_TOOLCHAIN=1.95
ARMS=(gf2-element gf2-wide gf2-batch isal m4rie gfcomplete)

[[ -x "$RUNNER" ]] || { echo "build the runner first: $RUNNER" >&2; exit 2; }
rm -rf "$SMOKE"
mkdir -p "$SMOKE"
touch "$LOCK"
: >"$LEDGER"

python3 - "$SMOKE/addendum.json" "$LEDGER" <<'PY'
import json, sys

def cell(cell_id, identity, size):
    return {
        "cell_id": cell_id, "objective": "comparator-gap", "role": "exploratory",
        "workload": {"identity": identity, "size": size, "seed": 9001},
        "metric_kind": "kernel-isolated", "scaling": "single-core-latency",
        "core_arm": "single-core", "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": "warm", "builds": {"baseline": "native", "candidate": "external"},
        "conversion_costs_included": False, "decoder": None,
    }

json.dump({
    "schema": "zen3-benchmark-addendum-v4",
    "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
    "family": {
        "id": "byte-field-arms-smoke", "issue": "6c6b09b1", "purpose": "consumer-family",
        "description": (
            "Throwaway wire smoke of issue 6c6b09b1: every arm and every case operation the "
            "byte-field campaigns name, at the smallest pair count the protocol allows, on a "
            "throwaway ledger under target/. It decides nothing and no receipt cites it."),
    },
    "frozen": {"frozen_utc": "2026-09-12T13:30:00Z"},
    "effect": {
        "worthwhile_speedup": None,
        "rationale": "A wire smoke adopts nothing, so no worthwhile-speedup threshold applies.",
        "measurement_resolution": None, "resolution_evidence": None,
        "equivalence_margin": None,
        "equivalence_rationale": "No cell of a wire smoke decides equivalence.",
        "material_gap_threshold": None,
        "material_gap_rationale": "No cell of a wire smoke decides a material gap.",
    },
    "complexity_budget": {
        "max_new_unsafe_kernels": 0, "max_added_source_lines": 0,
        "maintenance_rationale": "A wire smoke changes no production code.",
    },
    "family_wise": {
        "alpha": 0.05, "prior_confirmatory_trials": 0, "prior_trials": [],
        "ledger_path": sys.argv[2],
    },
    "search_budget": {"max_pilot_trials_per_cell": 2,
                      "max_confirmatory_attempts_per_candidate": 1},
    "holdout": {"required": False, "cells": []},
    "cells": [
        cell("smoke-4k-element-vs-isal", "gf256-0x11d-region-axpy-4096-bytes", {"bytes": 4096}),
        cell("smoke-n64-wide-vs-m4rie", "gf256-0x11d-square-product-n64", {"n": 64}),
        cell("smoke-4k-batch-vs-gfcomplete", "gf256-0x11d-pairwise-multiply-4096-bytes",
             {"bytes": 4096}),
        cell("smoke-encode-element-vs-isal",
             "gf256-0x11d-generator-encode-4x10-by-10x4096",
             {"k": 10, "rows": 4, "bytes": 4096}),
    ],
}, open(sys.argv[1], "w"), indent=2)
PY

python3 "$SURVEY/make-plan-versioned.py" --addendum "$SMOKE/addendum.json" --label smoke \
  --campaign-id "$ISSUE-arms-smoke" --campaign-seed 20260912 --lock "$(realpath "$LOCK")" \
  --target "$TARGET" --max-cells-per-session 4 \
  --producing-manifest "$SURVEY/producing-inputs-v4.json" --pilot-pairs 6 \
  --output "$SMOKE/plan.json"

GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
  "$RUNNER" run "$SMOKE/stage" "$SMOKE/plan.json"
"$RUNNER" finalize "$SMOKE/stage" "$SMOKE/receipt"

# The run is judged by what every arm wrote, not by the exit codes above: a
# result line per arm, in a cell that carries paired executions.
python3 - "$SMOKE/receipt/receipt.json" "${ARMS[@]}" <<'PY'
import json, sys
receipt = json.load(open(sys.argv[1]))
expected = set(sys.argv[2:])
seen, cells = {}, 0
for cell in receipt["cells"]:
    pairs = cell.get("pairs") or []
    if not pairs:
        print(f"cell {cell['cell_id']} carries no paired execution", file=sys.stderr)
        sys.exit(1)
    cells += 1
    for pair in pairs:
        for role in ("baseline", "candidate"):
            arm = cell[f"{role}_arm"]
            if not pair[role].get("windows"):
                print(f"{arm} in {cell['cell_id']} reported no window", file=sys.stderr)
                sys.exit(1)
            seen[arm] = seen.get(arm, 0) + 1
missing = sorted(expected - set(seen))
if missing:
    print(f"no result line from {', '.join(missing)}", file=sys.stderr)
    sys.exit(1)
for arm in sorted(seen):
    print(f"  {arm:<12} {seen[arm]} result lines")
print(f"smoke-arms: {cells} cells, every one paired, all {len(expected)} arms reported")
PY
