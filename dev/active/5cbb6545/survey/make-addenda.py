#!/usr/bin/env python3
"""Write the frozen family addenda for one stage (jit:5cbb6545).

Usage: make-addenda.py <stage> <frozen-utc> [output-directory]

`stage` is `smoke` or `sweep`. Every cell of a generated addendum is
exploratory: a confirmation addendum is never written here but derived from a
committed pilot receipt by the canonical freezer
`dev/active/c7113c5a/survey/freeze-confirmation.py`, which stamps the
measurement resolution and the pilot digest.

The cell tables come from `families.py`, which the runner plan reads too, so
an addendum and its plan cannot disagree about a cell.
"""

import json
import os
import sys

import families

#: Smallest speedup worth carrying a second kernel for. The carry-save route
#: is roughly 190 machine instructions per kilobyte against the nibble
#: lookup's 380 (the committed asm artefact
#: `crates/gf2-kernels-simd/src/x86/asm/popcount.asm.txt`), so a consumer that
#: counts a cache-resident buffer is the beneficiary; below a tenth the choice
#: stops mattering to that consumer and the second kernel is not worth its
#: maintenance.
WORTHWHILE = 1.10
#: Largest slowdown a width that gains nothing may pay for the new boundary.
EQUIVALENCE = 1.03
#: Smallest residual gap against a pinned external implementation that would
#: justify further kernel work rather than a report.
MATERIAL_GAP = 1.20

FAMILIES = {
    "popcount": {
        "id": families.POPCOUNT_FAMILY,
        "purpose": "kernel-family",
        "ledger": "dev/bench_results/5cbb6545/popcount-route-selection-family-ledger.jsonl",
        "sweep": families.popcount_sweep,
        "sweep2": families.popcount_sweep2,
    },
    "fused": {
        "id": families.FUSED_FAMILY,
        "purpose": "consumer-family",
        "ledger": "dev/bench_results/5cbb6545/fused-count-consumers-family-ledger.jsonl",
        "sweep": families.fused_sweep,
        "sweep2": families.fused_sweep2,
    },
    "smoke": {
        "id": "count-optimization-smoke",
        "purpose": "kernel-family",
        "ledger": "dev/bench_results/5cbb6545/smoke-family-ledger.jsonl",
        "smoke": families.smoke,
    },
}

SWEEP_DESCRIPTION = {
    "popcount": (
        "The word count at which each population-count route of gf2-core "
        "overtakes the next, measured on identical buffers. Arms: the route "
        "ops::popcount took before this issue (the scalar backend below eight "
        "words, the AVX2 nibble lookup at or above), each kernel of the "
        "gf2-kernels-simd bundle called directly (the scalar POPCNT loop, the "
        "nibble lookup, the Harley-Seal carry-save loop), and the pinned "
        "libpopcnt and Mula references on the same windows. Cells sweep one "
        "word to a streaming set twice the last-level cache, with a "
        "three-word alignment offset and all-one and all-zero data at one mid "
        "width. Every cell is exploratory: this stage fixes the boundaries a "
        "later pilot and confirmation decide, and its samples enter no "
        "confirmation."
    ),
    "fused": (
        "The word count at which the fused Harley-Seal AND-population-count "
        "kernel overtakes the fused nibble lookup a gf2-core consumer reached "
        "before this issue, and the cost of the two-pass route a consumer "
        "without a fused kernel pays: a temporary copy, an in-place AND and a "
        "separate count inside every call. Every cell is exploratory: this "
        "stage fixes the boundary a later pilot and confirmation decide, and "
        "its samples enter no confirmation."
    ),
}


SWEEP2_DESCRIPTION = {
    "popcount": (
        "The word counts between the last width at which the established "
        "per-vector nibble lookup wins and the first width at which the "
        "Harley-Seal carry-save loop clears this family's worthwhile margin, "
        "which the first sweep bracketed between 128 and 256 words. Same two "
        "arms, same fixture generator, same timed loop. Every cell is "
        "exploratory: the stage places the boundary a later pilot and "
        "confirmation decide, and its samples enter no confirmation."
    ),
    "fused": (
        "The word counts between the last width at which the established "
        "fused nibble lookup wins and the first width at which the fused "
        "Harley-Seal loop clears this family's worthwhile margin, which the "
        "first sweep bracketed between 128 and 512 words. Every cell is "
        "exploratory: the stage places the boundary a later pilot and "
        "confirmation decide, and its samples enter no confirmation."
    ),
}


SMOKE_DESCRIPTION = (
    "Every arm identity of this issue, once, over all three case shapes, so "
    "the wire contract between benchmark-ab-runner and the arm binary is "
    "established by an execution rather than by reading code. The receipt is "
    "a throwaway: every cell is exploratory, the stage decides nothing, and "
    "its samples enter no pilot or confirmation."
)


def addendum(key, stage, frozen_utc, cells, description):
    family = FAMILIES[key]
    return {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": family["id"],
            "issue": "5cbb6545",
            "purpose": family["purpose"],
            "description": description,
        },
        "frozen": {"frozen_utc": frozen_utc},
        "effect": {
            "worthwhile_speedup": WORTHWHILE,
            "rationale": (
                "The candidate routes add one unsafe kernel module to "
                "gf2-kernels-simd and one selector to the canonical tuning "
                "mechanism. A tenth of the runtime at a width a consumer "
                "counts is the smallest gain that repays that: below it the "
                "choice of route stops changing what a caller of "
                "BitVec::count_ones or BitMatrix::matvec observes, and the "
                "established nibble lookup keeps the width."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": EQUIVALENCE,
            "equivalence_rationale": (
                "A width that gains nothing from the new boundary must not "
                "pay for it. Three percent is the largest slowdown at such a "
                "width that would still leave the boundary worth having; "
                "above it the route the width keeps is the wrong one."
            ),
            "material_gap_threshold": MATERIAL_GAP,
            "material_gap_rationale": (
                "A fifth of the runtime is the smallest residual gap against "
                "a pinned external implementation that would justify further "
                "kernel work rather than a reported measurement; smaller gaps "
                "are reported and left."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 3,
            "max_added_source_lines": 900,
            "maintenance_rationale": (
                "The budget covers one unsafe kernel module holding the "
                "scalar POPCNT loop and the two Harley-Seal kernels, the "
                "bundle fields and resolvers that reach them, the tuning "
                "selector that carries the boundary, and the rustdoc the "
                "changed mechanism needs. It excludes the shared behavioural "
                "suite and this survey, which are test and evidence code."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": family["ledger"],
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 2,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": [
            {
                "cell_id": item["cell_id"],
                "objective": item["objective"],
                "role": item["role"],
                "workload": {
                    "identity": item["identity"],
                    "size": workload_size(item["case"]),
                    "seed": workload_seed(item["case"]),
                },
                "metric_kind": item["metric_kind"],
                "scaling": "single-core-latency",
                "core_arm": "single-core",
                "workers": {"declared": 1, "nested_pools_allowed": False},
                "cache_state": item["cache_state"],
                "builds": {
                    "baseline": build_of(item["baseline_arm"]),
                    "candidate": build_of(item["candidate_arm"]),
                },
                "conversion_costs_included": item["conversion_costs_included"],
                "decoder": None,
            }
            for item in cells
        ],
    }


def build_of(arm):
    """External arms are compiled by their upstream's own flags."""
    return "external" if arm in ("libpopcnt", "mula-avx2-harley-seal") else "conservative-portable"


def workload_size(case):
    if case["op"] == "matvec":
        return {"rows": case["rows"], "cols": case["cols"]}
    return {"words": case["words"], "word_offset": case["word_offset"]}


def workload_seed(case):
    return case["seed"] if case["op"] != "and_popcnt" else case["seed_lhs"]


def main():
    if len(sys.argv) not in (3, 4):
        raise SystemExit("usage: make-addenda.py <stage> <frozen-utc> [directory]")
    stage, frozen_utc = sys.argv[1], sys.argv[2]
    directory = sys.argv[3] if len(sys.argv) == 4 else os.path.dirname(
        os.path.dirname(os.path.abspath(__file__))
    )
    if stage not in ("sweep", "sweep2", "smoke"):
        raise SystemExit(f"stage {stage!r} has no generated table yet")
    for key, family in FAMILIES.items():
        if stage not in family:
            continue
        cells = family[stage]()
        description = {
            "sweep": SWEEP_DESCRIPTION.get(key),
            "sweep2": SWEEP2_DESCRIPTION.get(key),
            "smoke": SMOKE_DESCRIPTION,
        }[stage]
        document = addendum(key, stage, frozen_utc, cells, description)
        path = os.path.join(directory, f"addendum-{key}-v4-{stage}.json")
        with open(path, "w") as handle:
            json.dump(document, handle, indent=2)
            handle.write("\n")
        print(f"{len(cells)} cells -> {path}", file=sys.stderr)


if __name__ == "__main__":
    main()
