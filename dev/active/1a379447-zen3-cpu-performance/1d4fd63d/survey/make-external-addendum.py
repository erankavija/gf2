#!/usr/bin/env python3
"""Write the frozen pilot addendum of the transpose-lane comparator family.

Usage: make-external-addendum.py --frozen-utc <UTC> --output <json>

The family asks whether a material gap separates each gf2 transpose lane from
M4RI's `mzd_transpose` and Bitshuffle's `bshuf_bitshuffle` at the kernel
geometry 6fb89a3c froze: one 64x64 bit block per call, into a preallocated
output, under an equivalent bit mapping this issue's `verify-bit-mapping.py`
checks against naive bit arithmetic for every arm before any timing. It is a
separate scientific question from `transpose-lane-selection`, which compares
gf2 lanes against each other, so it carries its own ledger.

Every cell is exploratory here. The confirmation is derived from the committed
pilot receipt by the canonical freezer,
`dev/active/c7113c5a/survey/freeze-confirmation.py`, which selects the two
cells P-20's tail-support bound admits.
"""

import argparse
import json
import pathlib

# The gf2 arms, in the order `TransposeLane::ALL` lists the lanes they name.
# `production` is the lane the dispatch publishes, which is the arm 6fb89a3c's
# own kernel cells measured.
LANES = [
    ("production", "the lane gf2_kernels_simd::transpose::detect publishes on this host"),
    ("avx2-ymm6", "all six mask-shift-XOR stages in YMM registers with no stack scratch"),
    ("avx2-pshufb", "8x8 byte tiles through a vpshufb bit-reversal lookup"),
    (
        "avx2-movemask",
        "an SSE byte transpose followed by vpmovmskb bit-plane extraction",
    ),
]

COMPARATORS = [
    ("m4ri", "M4RI 20260122 mzd_transpose into a preallocated output"),
    ("bitshuffle", "Bitshuffle 0.5.2 bshuf_bitshuffle over 64 eight-byte elements"),
]

SEED_BASE = 20260913300


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--frozen-utc", required=True)
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()

    cells, seed = [], SEED_BASE
    for lane, _mechanism in LANES:
        for comparator, _description in COMPARATORS:
            seed += 1
            cells.append(
                {
                    "cell_id": f"lane-{lane}-block-64-vs-{comparator}",
                    "objective": "comparator-gap",
                    "role": "exploratory",
                    "workload": {
                        "identity": "transpose-64-word-kernel",
                        "size": {"n": 64},
                        "seed": seed,
                    },
                    "metric_kind": "kernel-isolated",
                    "scaling": "single-core-latency",
                    "core_arm": "single-core",
                    "workers": {"declared": 1, "nested_pools_allowed": False},
                    "cache_state": "warm",
                    "builds": {
                        "baseline": "conservative-portable",
                        "candidate": "external",
                    },
                    "conversion_costs_included": False,
                    "decoder": None,
                }
            )

    lanes = "; ".join(f"{lane} is {mechanism}" for lane, mechanism in LANES)
    comparators = "; ".join(
        f"{name} is {description}" for name, description in COMPARATORS
    )
    description = (
        "Whether a material gap separates gf2's 64x64 bit-block transpose lanes "
        "from the two external comparators at the kernel geometry 6fb89a3c froze: "
        "one 64x64 block per call into a preallocated output, with no tail and no "
        "geometry adapter, because one block of 64 eight-byte elements is the "
        "canonical gf2 layout for both comparators. The baseline arm of every cell "
        "is a gf2 lane and the candidate is the external arm, so the speedup of "
        "medians is median(gf2) / median(external) and a value above one means the "
        "external arm is ahead; `improved` in a comparator-gap cell is a material "
        "gap in the external arm's favour that needs attribution and `regressed` "
        "means gf2 is materially ahead. The gf2 arms are one executable that "
        "selects its lane from GF2_TRANSPOSE_LANE: " + lanes + ". The external arms "
        "are 6fb89a3c's pinned executables, rebuilt by its committed fetch-build.sh "
        "and checked byte for byte against its committed build-evidence.json before "
        "any timed run: " + comparators + ". Every arm reads the same fixture, 64 "
        "words of SplitMix64 seeded at the cell's seed, and this issue's "
        "verify-bit-mapping.py checks every one of them against naive bit "
        "arithmetic at five seeds before any timing, so no cell times two routes "
        "that compute different bits. Adapter costs are zero at this geometry and "
        "are not hidden: the geometries that do pay an adapter, 63 and 65 rows, are "
        "6fb89a3c's own consumer cells and that survey carries them. Every cell "
        "here is exploratory; it ranks the lanes against the comparators and fixes "
        "the measurement resolution the confirmation freezes against."
    )

    addendum = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": "transpose-lane-vs-external",
            "issue": "1d4fd63d",
            "purpose": "kernel-family",
            "description": description,
        },
        "frozen": {"frozen_utc": arguments.frozen_utc},
        "effect": {
            "worthwhile_speedup": None,
            "rationale": (
                "Comparator-gap cells only. This family adopts no implementation "
                "and selects nothing, so no worthwhile-speedup threshold applies "
                "and none is declared; the lane gf2 publishes is decided by the "
                "transpose-lane-selection family."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.1,
            "equivalence_rationale": (
                "A one-sided non-inferiority margin of ten percent: a gf2 lane "
                "within ten percent of an external comparator at a kernel this "
                "small is not behind it in any way a consumer of the library can "
                "act on, because every consumer pays tile assembly and allocation "
                "around the block. It is provisional until the pilot measures the "
                "family's resolution, and the freezer replaces it if the "
                "resolution does not admit it."
            ),
            "material_gap_threshold": 1.2,
            "material_gap_rationale": (
                "A twenty percent gap in a nanosecond-scale block transpose is the "
                "smallest difference worth attributing to a named cause (the tile "
                "kernel, the instruction mix, the output handling) rather than to "
                "host noise. It is the threshold 6fb89a3c froze for the same "
                "comparison, kept so the two surveys' comparator-gap decisions are "
                "read under one rule."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 0,
            "maintenance_rationale": (
                "This family surveys external comparators and changes no production "
                "line; its whole diff is under dev/."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": "dev/bench_results/1d4fd63d/transpose-lane-vs-external-family-ledger.jsonl",
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": cells,
    }
    output = pathlib.Path(arguments.output)
    with output.open("w") as handle:
        json.dump(addendum, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    print(f"{len(cells)} cells -> {output}")


if __name__ == "__main__":
    main()
