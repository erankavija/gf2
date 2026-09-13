#!/usr/bin/env python3
"""Write the runner plan for one transpose-lane campaign (jit:1d4fd63d).

Usage: make-plan.py <plan.json> <campaign-id> <label> <addendum> <seed> <arm> <lock>

Every cell, size and seed comes from the frozen addendum; this script adds no
numeric setting of its own. It resolves three things the addendum does not
carry:

* the arm pair, from the cell identifier. A cell named `lane-<tag>-...` puts
  the production dispatch against the lane `<tag>`; a cell whose identifier
  ends `-control-<arm>` puts the production dispatch against itself.
* the case the arm decodes, from the cell's `workload.identity` and `size`.
* the mother-field row of a code cell, from the committed code registry
  `dev/active/04b85d10/survey/code-rows.json`, so the modulus and designed
  distance behind a declared `degree` are read from that registry rather than
  restated here.
"""

import json
import pathlib
import sys

REGISTRY = "dev/active/04b85d10/survey/code-rows.json"

ARMS = {
    "production": (
        "production",
        "the 64x64 block kernel gf2_kernels_simd::transpose::detect publishes "
        "for this host, which is what BitMatrix::transpose and gf2-coding's "
        "bit-sliced BCH encoding reach: the pinned pre-change implementation",
    ),
    "production-control": (
        "production",
        "identity control: the production dispatch launched as a second arm, "
        "so the cell measures the pinned consumer against itself",
    ),
    "lane-avx2-ymm6": (
        "avx2-ymm6",
        "the avx2-ymm6 candidate lane, named through transpose::lane: all six "
        "mask-shift-XOR stages in YMM registers with no stack scratch",
    ),
    "lane-avx2-pshufb": (
        "avx2-pshufb",
        "the avx2-pshufb candidate lane, named through transpose::lane: 8x8 "
        "byte tiles through a vpshufb bit-reversal lookup",
    ),
    "lane-avx2-movemask": (
        "avx2-movemask",
        "the avx2-movemask candidate lane, named through transpose::lane: an "
        "SSE byte transpose followed by vpmovmskb bit-plane extraction",
    ),
}


def repo_root():
    import subprocess

    return pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )


def mother_code(root, degree):
    rows = json.loads((root / REGISTRY).read_text())["packed_bch_mother_codes"]
    for row in rows:
        if row["degree"] == degree:
            return row
    raise SystemExit(f"{REGISTRY} registers no mother code of degree {degree}")


def arm_pair(cell_id):
    if "-control-" in cell_id:
        return "production", "production-control"
    if not cell_id.startswith("lane-"):
        raise SystemExit(f"cell {cell_id!r} names no arm pair")
    for name in ARMS:
        if name.startswith("lane-") and cell_id.startswith(f"{name}-"):
            return "production", name
    raise SystemExit(f"cell {cell_id!r} names no known candidate lane")


def case(root, declared):
    identity = declared["workload"]["identity"]
    size = declared["workload"]["size"]
    seed = declared["workload"]["seed"]
    if identity in ("transpose-block-64x64", "transpose-bulk-64x64"):
        kind = "transpose-block" if identity.endswith("block-64x64") else "transpose-bulk"
        return {"kind": kind, "blocks": size["blocks"], "seed": seed}
    if identity == "bitmatrix-transpose":
        return {
            "kind": "matrix-transpose",
            "rows": size["rows"],
            "cols": size["cols"],
            "seed": seed,
        }
    if identity in (
        "bch-bitslice-absorb",
        "bch-bitslice-unpack",
        "bch-encode-batch-bitslice",
    ):
        row = mother_code(root, size["degree"])
        built = {
            "kind": {
                "bch-bitslice-absorb": "bitslice-absorb",
                "bch-bitslice-unpack": "bitslice-unpack",
                "bch-encode-batch-bitslice": "bch-encode-bitslice",
            }[identity],
            "degree": row["degree"],
            "modulus": row["modulus"],
            "designed_distance": row["designed_distance"],
            "seed": seed,
        }
        if identity == "bch-encode-batch-bitslice":
            built["batch"] = size["batch"]
        return built
    raise SystemExit(f"cell {declared['cell_id']!r} names unknown workload {identity!r}")


def main():
    plan_path, campaign, label, addendum_path, seed, executable, lock = sys.argv[1:8]
    root = repo_root()
    addendum = json.loads(pathlib.Path(addendum_path).read_text())
    pilot_pairs = int(sys.argv[8]) if len(sys.argv) > 8 else None

    cells, used = [], set()
    for declared in addendum["cells"]:
        baseline, candidate = arm_pair(declared["cell_id"])
        used.update((baseline, candidate))
        cells.append(
            {
                "cell_id": declared["cell_id"],
                "baseline_arm": baseline,
                "candidate_arm": candidate,
                "case": case(root, declared),
                # A confirmatory cell leaves this null and takes the frozen
                # confirmatory sample; an exploratory one takes the launcher's
                # declared pilot sample.
                "pilot_pairs": pilot_pairs if declared["role"] == "exploratory" else None,
            }
        )

    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": addendum["family"]["issue"],
        "label": label,
        "campaign_seed": int(seed),
        "addendum": addendum_path,
        "producing_manifest": "dev/active/1d4fd63d/producing-inputs.json",
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh --full-host",
        "timing_override": None,
        "arms": {
            name: {
                "build": "conservative-portable",
                "description": description,
                "executable": executable,
                "arguments": [],
                "environment": {
                    "RAYON_NUM_THREADS": "1",
                    "GF2_TRANSPOSE_LANE": lane,
                },
                "rustflags": None,
                "tuning_profile": None,
            }
            for name, (lane, description) in ARMS.items()
            if name in used
        },
        "cells": cells,
        # The exclusive mutex is released between blocks of cells so sibling
        # workers are not starved, and the runner resumes from its checkpoints.
        "max_cells_per_session": 5,
    }
    with open(plan_path, "w") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(
        f"plan: {len(cells)} cells, {len(plan['arms'])} arms -> {plan_path}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
