#!/usr/bin/env python3
"""Write the frozen pilot addendum of the transpose-lane family (jit:1d4fd63d).

Usage: make-pilot-addendum.py --frozen-utc <YYYY-MM-DDTHH:MM:SSZ> --output <json>

The cell grid is the product of the candidate lanes and the workloads the
issue's REQ-09 names, plus the identity control that pins the whole BCH
consumer the conversion sits inside. A code cell names a mother-field degree;
the family description takes that code's words from the committed registry
`dev/active/04b85d10/survey/code-rows.json` rather than from free text, and
`make-plan.py` resolves the same row into the modulus and designed distance the
arm constructs from.

The script writes the addendum and nothing else. The confirmation addendum is
derived from the committed pilot receipt by the canonical freezer,
`dev/active/c7113c5a/survey/freeze-confirmation.py`.
"""

import argparse
import json
import pathlib
import subprocess

REGISTRY = "dev/active/04b85d10/survey/code-rows.json"

# The candidate lanes, in the order `TransposeLane::ALL` lists them, with the
# mechanism each cell's report names.
CANDIDATES = [
    ("avx2-ymm6", "all six mask-shift-XOR stages in YMM registers with no stack scratch"),
    ("avx2-pshufb", "8x8 byte tiles through a vpshufb bit-reversal lookup"),
    (
        "avx2-movemask",
        "an SSE byte transpose followed by vpmovmskb bit-plane extraction, "
        "which is the movemask geometry's tile assembly and packing inside the "
        "64x64 contract",
    ),
]

# One workload per row: the cell-identifier suffix, the workload identity the
# plan derivation reads, the declared size, the metric kind, the scaling, the
# core arm, the cache state and the objective.
WORKLOADS = [
    (
        "block-256-1core",
        "transpose-block-64x64",
        {"blocks": 256},
        "kernel-isolated",
        "single-core-latency",
        "single-core",
        "warm",
        "improvement",
    ),
    (
        "bulk-4096-6core",
        "transpose-bulk-64x64",
        {"blocks": 4096},
        "kernel-isolated",
        "sustained-throughput",
        "physical-cores-6",
        "streaming",
        "improvement",
    ),
    (
        "bitslice-absorb-m14-1core",
        "bch-bitslice-absorb",
        {"degree": 14},
        "kernel-isolated",
        "single-core-latency",
        "single-core",
        "warm",
        "improvement",
    ),
    (
        "bitslice-unpack-m14-1core",
        "bch-bitslice-unpack",
        {"degree": 14},
        "kernel-isolated",
        "single-core-latency",
        "single-core",
        "warm",
        "improvement",
    ),
    (
        "matrix-transpose-4096-1core",
        "bitmatrix-transpose",
        {"rows": 4096, "cols": 4096},
        "whole-consumer",
        "single-core-latency",
        "single-core",
        "warm",
        "improvement",
    ),
    (
        "matrix-transpose-65-1core",
        "bitmatrix-transpose",
        {"rows": 65, "cols": 65},
        "whole-consumer",
        "single-core-latency",
        "single-core",
        "warm",
        "non-regression",
    ),
]

CONTROL = (
    "bch-encode-bitslice-m14-b256-control-1core",
    "bch-encode-batch-bitslice",
    {"degree": 14, "batch": 256},
    "whole-consumer",
    "single-core-latency",
    "single-core",
    "warm",
    "non-regression",
)

SEED_BASE = 20260913000


def cell(cell_id, identity, size, metric, scaling, core_arm, cache_state, objective, seed):
    return {
        "cell_id": cell_id,
        "objective": objective,
        "role": "exploratory",
        "workload": {"identity": identity, "size": size, "seed": seed},
        "metric_kind": metric,
        "scaling": scaling,
        "core_arm": core_arm,
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache_state,
        "builds": {
            "baseline": "conservative-portable",
            "candidate": "conservative-portable",
        },
        "conversion_costs_included": metric == "whole-consumer",
        "decoder": None,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--frozen-utc", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    root = pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    rows = json.loads((root / REGISTRY).read_text())["packed_bch_mother_codes"]
    row = next(entry for entry in rows if entry["degree"] == 14)

    cells, seed = [], SEED_BASE
    for lane, _mechanism in CANDIDATES:
        for suffix, identity, size, metric, scaling, core_arm, cache_state, objective in WORKLOADS:
            seed += 1
            cells.append(
                cell(
                    f"lane-{lane}-{suffix}",
                    identity,
                    size,
                    metric,
                    scaling,
                    core_arm,
                    cache_state,
                    objective,
                    seed,
                )
            )
    seed += 1
    cells.append(cell(*CONTROL, seed))

    mechanisms = "; ".join(f"{lane} is {mechanism}" for lane, mechanism in CANDIDATES)
    description = (
        "Which 64x64 bit-block transpose lane gf2 should publish. The baseline "
        "arm of every comparison is the kernel gf2_kernels_simd::transpose::detect "
        "publishes on this host, the pinned pre-change implementation that "
        "BitMatrix::transpose and gf2-coding's bit-sliced BCH encoding reach. The "
        "candidate arms are the three other lanes of the same contract, each named "
        "through the public transpose::lane and reaching the consumers through the "
        "same abstraction the production kernel does: " + mechanisms + ". One "
        "executable serves every arm and selects its lane from GF2_TRANSPOSE_LANE, "
        "so the arms share a build, a fixture generator, a warm pass and a timing "
        "loop. Each candidate is measured on six workloads: the block kernel alone "
        "over a 256-block L2-resident run and over a 4096-block streaming run on a "
        "six-core arm; the two halves of the BCH bit-slice conversion, which are one "
        "lane group's whole message absorbed through the transpose and the "
        "bit-sliced recurrence and the reduced register read back as packed "
        "per-frame parity, both on "
        + row["label"]
        + " (mother-field degree "
        + str(row["degree"])
        + ", corpus row "
        + row["corpus_row"]
        + ", resolved from "
        + REGISTRY
        + "); and the whole BitMatrix::transpose consumer at 4096 squared and at 65 "
        "squared, whose output allocation, tile assembly, partial boundary tiles and "
        "tail mask are inside the timed call. The 65-squared cell and the control are "
        "non-regression cells; every other cell is an improvement cell. The control "
        "cell puts the whole bit-sliced BCH batch encode of the same code at batch "
        "256 against itself through the production family entry point, which "
        "resolves its own block kernel: it measures no lane difference and pins the "
        "consumer latency the conversion cells sit inside. This addendum is the "
        "pilot: every cell is exploratory, it ranks the candidates and fixes the "
        "measurement resolution the confirmation freezes against, and none of its "
        "samples enters a confirmation."
    )

    addendum = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": "transpose-lane-selection",
            "issue": "1d4fd63d",
            "purpose": "kernel-family",
            "description": description,
        },
        "frozen": {"frozen_utc": args.frozen_utc},
        "effect": {
            "worthwhile_speedup": 1.1,
            "rationale": (
                "A published lane carries an unsafe kernel, its safety contract, its "
                "share of the shared contract cases and its section of the committed "
                "asm artefact, and the lane it replaces stays in the family as a "
                "candidate. Ten percent of the measured transform is the smallest "
                "gain worth that standing maintenance: below it a consumer cannot "
                "tell the lanes apart, and the conversion and whole-matrix cells "
                "measure the transform with its tile assembly and packing included, "
                "so the threshold applies to the complete conversion cost rather "
                "than to a kernel in isolation."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.05,
            "equivalence_rationale": (
                "The partial-tile geometry and the whole BCH consumer gain nothing "
                "from a faster block kernel and must not pay for one. Five percent is "
                "the smallest slowdown at those cells that would outweigh publishing "
                "a lane that wins elsewhere, so it is the margin the non-regression "
                "cells declare."
            ),
            "material_gap_threshold": None,
            "material_gap_rationale": (
                "Every cell compares two gf2 lanes of the same contract, so no "
                "comparator-gap objective is exercised and no threshold applies. The "
                "external comparators of this question are surveyed in 6fb89a3c, "
                "whose own family carries them."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 2,
            "max_added_source_lines": 900,
            "maintenance_rationale": (
                "The two new candidate kernels, the lane enumeration and dispatch "
                "that replaces the per-lane entry points, the shared block and "
                "matrix contract cases every lane runs, and the rustdoc the changed "
                "mechanism needs. It excludes the committed asm artefact and its "
                "annotation, and the campaign harness under dev/, neither of which "
                "is production source."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": "dev/bench_results/1d4fd63d/transpose-lane-selection-family-ledger.jsonl",
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": cells,
    }
    output = pathlib.Path(args.output)
    with output.open("w") as handle:
        json.dump(addendum, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    print(f"{len(cells)} cells -> {output}")


if __name__ == "__main__":
    main()
