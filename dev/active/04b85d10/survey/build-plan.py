#!/usr/bin/env python3
"""Derives the runner plan of the bit-storage consumer profile (jit:04b85d10).

The plan's cases come from the frozen addendum's cells, so a cell's declared
workload identity, sizes and seed and the case its arms actually receive cannot
drift apart. Only the arm pairing lives here, because the addendum declares
what a cell measures and the plan declares which executables measure it.

Usage:
  build-plan.py <plan.json> <campaign> <arm-exe> <lock> <mode> <label> <addendum>
"""

import json
import sys

# Every arm runs the same executable and differs only in its environment, so
# the two children of a pair read byte-identical input and separate only at
# the production route they call.
ARMS = {
    "ops-dispatched": ("ops-dispatched", "size-dispatched kernel entry point, re-selected per call", {}),
    "ops-resolved": ("ops-resolved", "the same kernel with backend resolution hoisted out of the loop", {}),
    "scalar-backend": ("scalar-backend", "the portable scalar backend called directly", {}),
    "simd-backend": ("simd-backend", "the detected SIMD backend called directly, detection hoisted", {}),
    "count-ones": ("count-ones", "zero test spelled as a full population count", {}),
    "find-first-one": ("find-first-one", "zero test spelled as a first-set-bit search", {}),
    "transpose-scalar": ("transpose-scalar", "portable 64x64 block transpose", {}),
    "transpose-detected": ("transpose-detected", "detected 64x64 block-transpose lane", {}),
    "family-reference": ("family-poly-remainder-scalar", "bit-serial reference encoding family", {}),
    "family-bitslice": ("family-bitslice-interleaved", "bit-sliced interleaved encoding family", {}),
    "family-fold": ("family-clmul-fold", "carry-less-multiply fold encoding family", {}),
    "current-a": ("current", "the route the production dispatcher selects", {}),
    "current-b": ("current", "the route the production dispatcher selects", {}),
    "current-12-a": ("current", "the production route across twelve rayon workers", {"RAYON_NUM_THREADS": "12"}),
    "current-12-b": ("current", "the production route across twelve rayon workers", {"RAYON_NUM_THREADS": "12"}),
    "current-24-a": ("current", "the production route across twenty-four rayon workers", {"RAYON_NUM_THREADS": "24"}),
    "current-24-b": ("current", "the production route across twenty-four rayon workers", {"RAYON_NUM_THREADS": "24"}),
}

# Baseline and candidate arm of each declared cell. A cell whose two arms are
# the same route pairs `current-a` with `current-b`: it measures the spread of
# the pinned pre-change implementation against itself, which is what a
# non-regression margin and a pinned baseline both need.
PAIRING = {
    "logical-row-xor-dispatch-1core": ("ops-dispatched", "ops-resolved"),
    "logical-row-xor-threshold-1core": ("ops-dispatched", "simd-backend"),
    "logical-dense-rref-1core": ("current-a", "current-b"),
    "logical-ldpc-syndrome-1core": ("current-a", "current-b"),
    "count-popcount-dispatch-1core": ("ops-dispatched", "scalar-backend"),
    "count-popcount-threshold-1core": ("scalar-backend", "simd-backend"),
    "count-zero-test-syndrome-1core": ("count-ones", "find-first-one"),
    "count-ldpc-check-1core": ("count-ones", "find-first-one"),
    "layout-transpose-block-1core": ("transpose-scalar", "transpose-detected"),
    "layout-bch-encode-bitslice-1core": ("family-reference", "family-bitslice"),
    "layout-bch-encode-fold-1core": ("family-reference", "family-fold"),
    "layout-bch-encode-alloc-1core": ("current-a", "current-b"),
    "layout-transpose-block-6core": ("transpose-scalar", "transpose-detected"),
    "layout-bch-encode-parallel-12core": ("current-12-a", "current-12-b"),
    "layout-bch-encode-parallel-24logical": ("current-24-a", "current-24-b"),
}

# Pairs an exploratory cell measures. Eight is above the frozen pilot minimum
# of six and below the confirmatory count, so the pilot estimates a resolution
# without approaching a confirmatory sample.
PILOT_PAIRS = 8

# Cells measured in one session before the runner pauses. Four bounds a
# session to a few minutes on a shared host, so the mutex returns to sibling
# workers between sessions.
MAX_CELLS_PER_SESSION = 4

# Every non-parallel arm pins one rayon worker, so a stray pool cannot enter a
# single-core cell.
SINGLE_WORKER_ENVIRONMENT = {"RAYON_NUM_THREADS": "1"}


def main():
    if len(sys.argv) != 8:
        sys.exit(__doc__)
    plan_path, campaign, arm_exe, lock, mode, label, addendum_path = sys.argv[1:]

    with open(addendum_path, encoding="utf-8") as handle:
        addendum = json.load(handle)

    arms = {}
    for name, (route, description, extra) in ARMS.items():
        environment = dict(SINGLE_WORKER_ENVIRONMENT)
        environment.update(extra)
        environment["GF2_CONSUMER_PATH"] = route
        arms[name] = {
            "build": "conservative-portable",
            "description": description,
            "executable": arm_exe,
            "arguments": [],
            "environment": environment,
            "rustflags": None,
            "tuning_profile": None,
        }

    cells = []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        if cell_id not in PAIRING:
            sys.exit(f"the addendum declares {cell_id}, which this plan does not pair")
        baseline, candidate = PAIRING[cell_id]
        workload = declared["workload"]
        cells.append(
            {
                "cell_id": cell_id,
                "baseline_arm": baseline,
                "candidate_arm": candidate,
                "case": {
                    "workload": workload["identity"],
                    "size": workload["size"],
                    "seed": workload["seed"],
                },
                "pilot_pairs": PILOT_PAIRS if declared["role"] == "exploratory" else None,
            }
        )

    used = {name for cell in cells for name in (cell["baseline_arm"], cell["candidate_arm"])}
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": addendum["family"]["issue"],
        "label": label,
        "campaign_seed": 20260907,
        "addendum": addendum_path,
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {name: arms[name] for name in sorted(used)},
        "cells": cells,
        "max_cells_per_session": MAX_CELLS_PER_SESSION,
    }
    with open(plan_path, "w", encoding="utf-8") as output:
        json.dump(plan, output, indent=2)
        output.write("\n")
    print(f"plan {plan_path}: {len(cells)} cells, {len(plan['arms'])} arms, mode {mode}",
          file=sys.stderr)


if __name__ == "__main__":
    main()
