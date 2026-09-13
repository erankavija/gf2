#!/usr/bin/env python3
"""Write the runner plan for one transpose-lane comparator campaign (jit:1d4fd63d).

Usage: make-external-plan.py <plan.json> <campaign-id> <label> <addendum> <seed>
                             <gf2-arm> <survey-dir> <lock> [pilot-pairs]

Every cell, size and seed comes from the frozen addendum; this script adds no
numeric setting of its own. It resolves the arm pair from the cell identifier:
`lane-<tag>-block-64-vs-<comparator>` puts the gf2 lane `<tag>` as the baseline
against that comparator's external arm as the candidate, which is the direction
6fb89a3c's comparator-gap cells use, so a speedup above one means the external
arm is ahead.

The case is 6fb89a3c's fixed kernel case, `{n, seed}`, which both external arms
and this issue's gf2-side arm decode as one 64x64 block per timed call into a
preallocated output.
"""

import json
import pathlib
import sys

COMPARATORS = {
    "m4ri": (
        "m4ri_transpose_arm",
        "M4RI 20260122 mzd_transpose (release tarball, GPL-2.0-or-later, "
        "gcc -O3 -march=native -fPIC, no runtime dispatch), reusing a "
        "preallocated output; the executable 6fb89a3c pinned, checked byte for "
        "byte against its build-evidence.json before this run",
    ),
    "bitshuffle": (
        "bitshuffle_transpose_arm",
        "Bitshuffle 0.5.2 bshuf_bitshuffle (commit "
        "52aec3b80d05606c090956aecfe868489d96b95c, MIT, gcc -O3 -march=native "
        "-fPIC, compile-time AVX2 route) over 64 eight-byte elements into a "
        "preallocated output; the executable 6fb89a3c pinned, checked byte for "
        "byte against its build-evidence.json before this run",
    ),
}

LANES = {
    "production": "the 64x64 block kernel gf2_kernels_simd::transpose::detect publishes on this host",
    "avx2-ymm6": "the avx2-ymm6 lane: all six mask-shift-XOR stages in YMM registers, no stack scratch",
    "avx2-pshufb": "the avx2-pshufb lane: 8x8 byte tiles through a vpshufb bit-reversal lookup",
    "avx2-movemask": "the avx2-movemask lane: an SSE byte transpose then vpmovmskb bit-plane extraction",
}


def arm_pair(cell_id):
    if not cell_id.startswith("lane-") or "-vs-" not in cell_id:
        raise SystemExit(f"cell {cell_id!r} names no arm pair")
    head, comparator = cell_id.rsplit("-vs-", 1)
    if comparator not in COMPARATORS:
        raise SystemExit(f"cell {cell_id!r} names no known comparator")
    for lane in LANES:
        if head == f"lane-{lane}-block-64":
            return f"gf2-{lane}", f"external-{comparator}", lane, comparator
    raise SystemExit(f"cell {cell_id!r} names no known gf2 lane")


def main():
    (
        plan_path,
        campaign,
        label,
        addendum_path,
        seed,
        gf2_arm,
        survey,
        lock,
    ) = sys.argv[1:9]
    pilot_pairs = int(sys.argv[9]) if len(sys.argv) > 9 else None
    addendum = json.loads(pathlib.Path(addendum_path).read_text())
    survey = pathlib.Path(survey)

    cells, arms = [], {}
    for declared in addendum["cells"]:
        baseline, candidate, lane, comparator = arm_pair(declared["cell_id"])
        arms.setdefault(
            baseline,
            {
                "build": "conservative-portable",
                "description": LANES[lane],
                "executable": gf2_arm,
                "arguments": [],
                "environment": {"RAYON_NUM_THREADS": "1", "GF2_TRANSPOSE_LANE": lane},
                "rustflags": None,
                "tuning_profile": None,
            },
        )
        executable, description = COMPARATORS[comparator]
        arms.setdefault(
            candidate,
            {
                "build": "external",
                "description": description,
                "executable": str((survey / executable).resolve()),
                "arguments": [],
                "environment": {},
                "rustflags": None,
                "tuning_profile": None,
            },
        )
        cells.append(
            {
                "cell_id": declared["cell_id"],
                "baseline_arm": baseline,
                "candidate_arm": candidate,
                "case": {
                    "n": declared["workload"]["size"]["n"],
                    "seed": declared["workload"]["seed"],
                },
                "pilot_pairs": pilot_pairs
                if declared["role"] == "exploratory"
                else None,
            }
        )

    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": addendum["family"]["issue"],
        "label": label,
        "campaign_seed": int(seed),
        "addendum": addendum_path,
        "producing_manifest": "dev/active/1d4fd63d/external-producing-inputs.json",
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": arms,
        "cells": cells,
        "max_cells_per_session": 4,
    }
    with open(plan_path, "w") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(
        f"plan: {len(cells)} cells, {len(arms)} arms -> {plan_path}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
