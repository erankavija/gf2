#!/usr/bin/env python3
"""Write the frozen pilot addendum of the sparse product check-cost family (jit:bfc2ceab).

Usage: make-spmm-addendum.py, with no arguments

Writes `addendum-v4-spmm-check-pilot.json` in the directory above this
script. Nothing here is a measurement: the file is a frozen declaration, and
the confirmation stage derives its own addendum from the committed pilot
receipt with the repository's canonical freezer.
"""

import json
import sys

from locate import HERE, ISSUE, ROOT, repository_files

FROZEN_UTC = "2026-10-07T14:23:00Z"
FAMILY = "sparse-product-check-cost"
OUTPUT = HERE.parent / "addendum-v4-spmm-check-pilot.json"
LEDGER_NAME = "spmm-check-family-ledger.jsonl"

# (cell, size, seed)
CELLS = [
    ("spmm-small-n16",
     {"prime": 7, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 16}, 5101),
    ("spmm-small-n1024",
     {"prime": 7, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 1024}, 5102),
    ("spmm-medium-n16",
     {"prime": 65521, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 16}, 5103),
    ("spmm-medium-n1024",
     {"prime": 65521, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 1024}, 5104),
]

FAMILY_DESCRIPTION = (
    f"Exploratory pilot of issue {ISSUE}, protocol version 4: this addendum is the pilot and "
    "every cell is exploratory. Question: what does a caller of the sparse-times-dense product "
    "over a prime field pay for the column check its kernel entry makes once per product? Both "
    "arms are the one executable spmm-check-arm, built once from one tree, and the environment "
    "selects the form. The candidate is the shipped consumer SparseFieldMatrix::matmat, whose "
    "kernel entry checks the output shape and the largest column index once and then sweeps "
    "the sparse rows inside the kernel crate, writing a lane buffer of the whole product that "
    "the dispatcher converts to field elements. The baseline is the same product without the "
    "check: the arm holds the dispatcher's row loop, calls the kernel crate's unchecked row "
    "kernel once per non-empty sparse row and converts the operands and each output row "
    "between the field type and the lane type as the dispatcher does. The pair therefore "
    "differs in the check, in the crate that holds the row loop, and in the output lane buffer, "
    "which is one row in the baseline and the whole product in the candidate; the family prices "
    "the checked form as shipped against the unchecked form, not the check in isolation. Both "
    "forms take the operands a caller holds, so packing, the kernel and the output are inside "
    "the timed call and no conversion stands outside. The arm's tests require both forms to "
    "equal a scalar reference. Cells: two lanes, the byte lane at p = 7 and the u16 lane at "
    "p = 65521, each at 16 dense columns, where one 16-column block per non-zero leaves the "
    "check its largest share, and at 1024 dense columns, where 64 blocks per non-zero amortize "
    "it. Every cell is single-core and warm: both forms run on the calling thread, and a "
    "cache-resident operand exposes a per-product instruction cost most. Frozen confirmation "
    "selection: a first confirmatory attempt spends alpha/2 = 0.025, and P-20's tail-support "
    "bound, 10000 * (0.025 / m) / 2 >= 20, admits m = 6 confirmatory comparisons, so the "
    "confirmation retains all four cells. Frozen resolution rule: the canonical freezer's, the "
    "widest relative bootstrap half-width over every pilot cell, rounded up to two decimals; "
    "all cells are of one thermal class. Frozen decision rule: the per-product check is "
    "recorded as an accepted measured cost when every confirmatory cell records pass; with a "
    "cell that records fail or inconclusive the form goes back to the invoker with the "
    "measured ratio. The rule reads the acceptance summary's recorded outcomes and nothing "
    "else."
)

WORTHWHILE_RATIONALE = (
    "The candidate adds a check and moves the row loop, so no cell is expected to record "
    "improved; the schema requires the threshold of every non-regression family. Ten percent "
    "is the smallest apparent gain that would need an explanation other than the check, such "
    "as the whole-product lane buffer, before the family's numbers were read as the check's "
    "cost."
)

EQUIVALENCE_RATIONALE = (
    "The checked entry removes undefined behaviour from a safe entry point, and the caller pays "
    "for it on every product. Five percent of a whole consumer call is the largest slowdown "
    "this family records as an accepted cost, the margin of the family that measured the "
    "per-row form of the same check: below it a consumer's throughput budget is unchanged at "
    "the precision such budgets are stated, and above it the invoker decides. The confirmation "
    "freezes a pilot-derived resolution strictly below this margin."
)

MATERIAL_GAP_RATIONALE = (
    "No cell is a comparator-gap cell: every cell compares two forms of one gf2 product, so no "
    "material-gap threshold applies."
)

MAINTENANCE_RATIONALE = (
    "The checked form is the tree's: one safe whole-product entry per lane over the row kernel "
    "that exists, published as well through an `unsafe fn` pointer. No new unsafe kernel is "
    "allowed, and one hundred and fifty lines is the ceiling for a change to that production "
    "source."
)


def cell(cell_id, size, seed):
    return {
        "cell_id": cell_id,
        "objective": "non-regression",
        "role": "exploratory",
        "workload": {"identity": "sparse-matmat-" + "-".join(
            f"{key}{value}" for key, value in size.items()), "size": size, "seed": seed},
        "metric_kind": "whole-consumer",
        "scaling": "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": True,
        "decoder": None,
    }


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    addendum = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": FAMILY,
            "issue": ISSUE,
            "purpose": "consumer-family",
            "description": FAMILY_DESCRIPTION,
        },
        "frozen": {"frozen_utc": FROZEN_UTC},
        "effect": {
            "worthwhile_speedup": 1.1,
            "rationale": WORTHWHILE_RATIONALE,
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.05,
            "equivalence_rationale": EQUIVALENCE_RATIONALE,
            "material_gap_threshold": None,
            "material_gap_rationale": MATERIAL_GAP_RATIONALE,
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 150,
            "maintenance_rationale": MAINTENANCE_RATIONALE,
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": repository_files.live_file(ROOT, LEDGER_NAME),
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 2,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": [cell(cell_id, size, seed) for cell_id, size, seed in CELLS],
    }
    OUTPUT.write_text(json.dumps(addendum, indent=2) + "\n")
    print(f"{OUTPUT.relative_to(ROOT)}: {len(addendum['cells'])} exploratory cells")


if __name__ == "__main__":
    main()
