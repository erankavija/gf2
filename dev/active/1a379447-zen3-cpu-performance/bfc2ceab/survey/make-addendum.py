#!/usr/bin/env python3
"""Write the frozen pilot addendum of the entry-check cost family (jit:bfc2ceab).

Usage: make-addendum.py, with no arguments

Writes `addendum-v4-entry-check-pilot.json` in the directory above this
script. Nothing here is a measurement: the file is a frozen declaration, and
the confirmation stage derives its own addendum from the committed pilot
receipt with the repository's canonical freezer.
"""

import json
import sys

import review
from locate import HERE, ISSUE, ROOT, repository_files

FROZEN_UTC = "2026-10-05T21:29:00Z"
FAMILY = "caller-trusted-entry-check-cost"
OUTPUT = HERE.parent / "addendum-v4-entry-check-pilot.json"
LEDGER_NAME = "entry-check-family-ledger.jsonl"
CFG = "--cfg gf2_entry_checked"

# (cell, consumer, size, seed); `review.PILOT_CELLS` states the checked form
# each cell measures.
CELLS = [
    ("spmm-small-n16", "sparse-matmat",
     {"prime": 7, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 16}, 5101),
    ("spmm-small-n1024", "sparse-matmat",
     {"prime": 7, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 1024}, 5102),
    ("spmm-medium-n16", "sparse-matmat",
     {"prime": 65521, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 16}, 5103),
    ("spmm-medium-n1024", "sparse-matmat",
     {"prime": 65521, "rows": 1024, "nonzeros_per_row": 8, "dense_cols": 1024}, 5104),
    ("permanent-f3-n14", "permanent-f3", {"n": 14}, 5105),
    ("ple-small-n128", "ple", {"prime": 7, "n": 128}, 5106),
    ("ple-medium-n128", "ple", {"prime": 65521, "n": 128}, 5107),
    ("gemm-small-n32", "gemm", {"prime": 7, "n": 32}, 5108),
    ("gemm-medium-n32", "gemm", {"prime": 65521, "n": 32}, 5109),
]
RETAINED = review.RETAINED_CELLS

FAMILY_DESCRIPTION = (
    f"Exploratory pilot of issue {ISSUE}, protocol version 4: this addendum is the pilot and "
    "every cell is exploratory. Question: what does a caller of a shipped consumer pay when the "
    "caller-trusted safe entry points of gf2-kernels-simd check their conditions in release "
    "builds? Both arms are the one arm package entry-check-arm built twice from one tree with "
    "the same profile and features: the baseline with no flag, the candidate with RUSTFLAGS "
    f"`{CFG}`, under which each affected detection function of the kernel crate publishes its "
    "checked wrapper instead of the wrapper it publishes today. The arms differ in that cfg "
    "alone; the plan records each arm's RUSTFLAGS and executable digest, and each execution "
    "reports the entry-point form its build publishes, observed from the linked kernel crate's "
    "behaviour on a probe panel and refused when it disagrees with the plan. The checked "
    "wrapper is the form the issue would ship by deleting the gate: a named release assertion "
    "ahead of the same unsafe kernel call, hoisted out of every kernel loop. Every cell is "
    "whole-consumer: the timed call is the public consumer on operands in the representation a "
    "caller holds, so packing, dispatch, the kernel and the output are inside it and no "
    "conversion stands outside. What each cell's check is: "
    + "; ".join(f"{cell} ({review.PILOT_CELLS[cell][1]}): {review.PILOT_CELLS[cell][2]}"
                for cell, _, _, _ in CELLS)
    + ". Every cell is single-core and warm: the consumers run on the calling thread in this "
    "build, and the check's cost is a per-call or per-row instruction cost that a cache-resident "
    "operand exposes most. Frozen confirmation selection: a first confirmatory attempt spends "
    "alpha/2 = 0.025, and P-20's tail-support bound, 10000 * (0.025 / m) / 2 >= 20, admits m = 6 "
    "confirmatory comparisons, so the confirmation retains "
    + ", ".join(RETAINED)
    + ", one cell per check grain of each cost class, and drops spmm-small-n1024 and "
    "spmm-medium-n1024, whose amortized regime is the favourable end of the row scan that the "
    "16-column cells carry at its least favourable, and ple-medium-n128, whose panel check has "
    "the form of ple-small-n128; the dropped cells keep their pilot evidence. The dense class is "
    "measured at both of its grains: gemm-small-n32 per row and gemm-medium-n32 per whole "
    "product. Frozen decision rule: the checked form of a cost class is recorded "
    "as an accepted measured cost when every confirmatory cell of the class records pass; a "
    "class with a cell that records fail or inconclusive goes back to the invoker with the "
    "measured ratio, as the choice between the check and an unchecked form of that interface. "
    "The rule reads the acceptance summary's recorded outcomes and nothing else."
)

WORTHWHILE_RATIONALE = (
    "A check cannot make a consumer faster, so no cell is expected to record improved; the "
    "schema requires the threshold of every non-regression family. Ten percent is the smallest "
    "apparent gain that would need an explanation other than the check, such as a layout "
    "effect of the second build, before the family's numbers were read as the check's cost."
)

EQUIVALENCE_RATIONALE = (
    "The checked form removes undefined behaviour from safe entry points, and the caller pays "
    "for it on every call. Five percent of a whole consumer call is the largest slowdown this "
    "family records as an accepted cost: below it a consumer's throughput budget is unchanged "
    "at the precision such budgets are stated, and above it the invoker decides between the "
    "check and an unchecked form of that interface. The confirmation freezes a pilot-derived "
    "resolution strictly below this margin."
)

MATERIAL_GAP_RATIONALE = (
    "No cell is a comparator-gap cell: every cell compares two builds of one gf2 consumer, so "
    "no material-gap threshold applies."
)

MAINTENANCE_RATIONALE = (
    "Adoption deletes the cfg gate and the unchecked wrappers and keeps the shared assertion "
    "module, one checked wrapper per entry point and the contract test. Three hundred lines "
    "is the ceiling for that production source, and no new unsafe kernel is allowed: the "
    "checked wrappers call the kernels that exist."
)


def cell(cell_id, consumer, size, seed):
    return {
        "cell_id": cell_id,
        "objective": "non-regression",
        "role": "exploratory",
        "workload": {"identity": f"{consumer}-" + "-".join(
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
            "max_added_source_lines": 300,
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
        "cells": [cell(cell_id, consumer, size, seed)
                  for cell_id, consumer, size, seed in CELLS],
    }
    OUTPUT.write_text(json.dumps(addendum, indent=2) + "\n")
    print(f"{OUTPUT.relative_to(ROOT)}: {len(addendum['cells'])} exploratory cells")


if __name__ == "__main__":
    main()
