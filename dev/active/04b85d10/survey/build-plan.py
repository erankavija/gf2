#!/usr/bin/env python3
"""Derives the runner plan of one bit-storage consumer family (jit:04b85d10).

The plan's cases come from the frozen addendum's cells, so a cell's declared
workload identity, sizes and seed and the case its arms actually receive cannot
drift apart. Only the arm pairing lives here, because the addendum declares
what a cell measures and the plan declares which executables measure it. The
pairing is keyed by cell identifier and is the same for a family's pilot and
its confirmation, so the confirmation measures exactly the routes the pilot
sized.

Usage:
  build-plan.py <plan.json> <campaign> <arm-exe> <lock> <label> <addendum> <max-cells>
"""

import json
import sys

PRODUCING_MANIFEST = "dev/active/04b85d10/survey/producing-inputs.json"

# Every arm runs the same executable and differs only in its environment, so
# the two children of a pair read byte-identical input and separate only at
# the production route they call.
ARMS = {
    "ops-dispatched": ("ops-dispatched", "size-dispatched kernel entry point, re-selected per call"),
    "ops-resolved": ("ops-resolved", "the same kernel with backend resolution hoisted out of the loop"),
    "scalar-backend": ("scalar-backend", "the portable scalar backend called directly"),
    "simd-backend": ("simd-backend", "the detected SIMD backend called directly, detection hoisted"),
    "count-ones": ("count-ones", "zero test spelled as a full population count"),
    "find-first-one": ("find-first-one", "zero test spelled as a first-set-bit search"),
    "transpose-scalar": ("transpose-scalar", "portable 64x64 block transpose"),
    "transpose-detected": ("transpose-detected", "detected 64x64 block-transpose lane"),
    "family-bitslice": ("family-bitslice-interleaved", "bit-sliced interleaved encoding family, forced"),
    "family-fold": ("family-clmul-fold", "carry-less-multiply fold encoding family, forced"),
    "caller-buffer": ("caller-buffer", "caller-buffer batch entry point encode_batch_into"),
    "current": ("current", "the route the production dispatcher selects"),
    "current-control": ("current", "identity control: the production route launched as a second arm"),
}

# Baseline and candidate arm of each declared cell. A `-control-` cell pairs
# the production route against itself: it measures the spread of the pinned
# pre-change implementation, which sizes the family resolution and records the
# consumer's baseline latency, and it never enters confirmation.
PAIRING = {
    # logical family
    "logical-row-xor-dispatch-64w-1core": ("ops-dispatched", "ops-resolved"),
    "logical-row-xor-dispatch-8w-1core": ("ops-dispatched", "ops-resolved"),
    "logical-row-xor-threshold-4w-1core": ("ops-dispatched", "simd-backend"),
    "logical-row-xor-dispatch-8192w-1core": ("ops-dispatched", "ops-resolved"),
    "logical-dense-rref-1024-control-1core": ("current", "current-control"),
    "logical-ldpc-syndrome-64800-control-1core": ("current", "current-control"),
    # count family
    "count-popcount-dispatch-4w-1core": ("ops-dispatched", "scalar-backend"),
    "count-popcount-threshold-8w-1core": ("scalar-backend", "simd-backend"),
    "count-popcount-bandwidth-65536w-1core": ("scalar-backend", "simd-backend"),
    "count-zero-test-507w-1core": ("count-ones", "find-first-one"),
    "count-ldpc-check-64800-1core": ("count-ones", "find-first-one"),
    "count-dense-matvec-1024x4096-control-1core": ("current", "current-control"),
    # layout family
    "layout-transpose-block-256-1core": ("transpose-scalar", "transpose-detected"),
    "layout-transpose-block-4096-6core": ("transpose-scalar", "transpose-detected"),
    "layout-bch-encode-bitslice-m14-b256-1core": ("current", "family-bitslice"),
    "layout-bch-encode-fold-m14-b256-1core": ("current", "family-fold"),
    "layout-bch-encode-caller-buffer-m14-b256-1core": ("current", "caller-buffer"),
    "layout-dense-transpose-4096-control-1core": ("current", "current-control"),
    "layout-dvb-bch-encode-7200-control-1core": ("current", "current-control"),
}

# Pairs an exploratory cell measures. The frozen pilot maximum: a pilot at the
# confirmatory sample size observes the resolution the confirmation will have.
PILOT_PAIRS = 24

# Every arm pins one rayon worker, so a stray pool cannot enter a cell.
SINGLE_WORKER_ENVIRONMENT = {"RAYON_NUM_THREADS": "1"}


def main():
    if len(sys.argv) != 8:
        sys.exit(__doc__)
    plan_path, campaign, arm_exe, lock, label, addendum_path, max_cells = sys.argv[1:]

    with open(addendum_path, encoding="utf-8") as handle:
        addendum = json.load(handle)

    arms = {}
    for name, (route, description) in ARMS.items():
        environment = dict(SINGLE_WORKER_ENVIRONMENT)
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
        if declared["role"] != "exploratory" and candidate == "current-control":
            sys.exit(f"{cell_id} is an identity control and cannot be confirmatory")
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
        "campaign_seed": 20260910,
        "addendum": addendum_path,
        "producing_manifest": PRODUCING_MANIFEST,
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {name: arms[name] for name in sorted(used)},
        "cells": cells,
        "max_cells_per_session": int(max_cells),
    }
    with open(plan_path, "w", encoding="utf-8") as output:
        json.dump(plan, output, indent=2)
        output.write("\n")
    print(
        f"plan {plan_path}: {len(cells)} cells, {len(plan['arms'])} arms, label {label}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
