#!/usr/bin/env python3
"""Write the frozen pilot addendum of the GF(2^8) dense-product family (jit:4c1e441f).

Usage: make-addendum.py [output]
       (default dev/active/4c1e441f/addendum-v4-dense-product-pilot.json)

Every cell of this family measures the same operation through the same entry
point and differs only in the element representation, the square dimension and
the consumer boundary, so the cells are projected from one table rather than
written out one by one. Nothing here is a measurement: the file it writes is a
frozen declaration, and the confirmation stage derives its own addendum from the
committed pilot receipt with the repository's canonical freezer.
"""

import json
import os
import subprocess
import sys

FROZEN_UTC = "2026-09-18T12:58:14Z"

FAMILY_DESCRIPTION = (
    "Exploratory pilot of issue 4c1e441f, protocol version 4: this addendum is the pilot and "
    "every cell is exploratory. Question: does the cached GF(2^8) product table that gf2-core "
    "routes the dense FieldMatrix product through make field::matrix::gemm faster than the route "
    "the library takes without it, at the square dimensions and at the consumer boundary a "
    "consumer holds? Both arms are one executable, built from the shipped crate with the simd and "
    "test-support features. The baseline arm holds every GF(2^8) call on the route without the "
    "table through the shipped process-global lane switch (gf2m::force_scalar_gf256_table); the "
    "candidate arm leaves the switch clear and lets gf256_table_dispatch select the cached table. "
    "A pair therefore differs in the lane and in nothing else: one executable, one build identity, "
    "one set of operands, one entry point. Each execution reports the lane the shipped witness "
    "(gf2m::last_gf256_table_lane) recorded for its measured calls, the number of product tables "
    "the process built, the route the representation takes when the dispatch declines, and the "
    "allocating calls and bytes one call of the cell's own body makes, all read at run time. "
    "Every cell works in GF(2^8) modulo 0x11D (x^8+x^4+x^3+x^2+1), the field Gf2mField::gf256() "
    "builds. A cell identifier ends in the representation the consumer holds: element = "
    "FieldMatrix<Gf2mElement> over Gf2mField::gf256(); wide = FieldMatrix<Gf2mWide<1,Gf256x11d>>, "
    "the byte-field survey's 0x11D Gf2mWideConfig. Each representation is its own cell because the "
    "two reach different routes when the dispatch declines: the runtime-context element runs a "
    "batched dot product per output cell, and the compile-time-configured value runs the "
    "carry-less-product whole-gemm kernel when the host carries AVX2 and VPCLMULQDQ and its "
    "panelized fallback otherwise, so one cell cannot speak for both. A cell whose identifier "
    "carries 'whole' is the whole-matrix consumer boundary: its window starts and ends at the "
    "row-major byte region, so it includes both operand conversions and the output conversion as "
    "well as the product, which is where the accepted path's three scratch byte buffers and its "
    "restoration of the transposed right operand are paid; it declares metric_kind "
    "whole-consumer with conversion costs included. Every other cell is kernel-isolated: its "
    "operands are already the FieldMatrix the consumer keeps, and the scratch buffers, the operand "
    "restoration and the dispatch are inside the measured call while no representation conversion "
    "is. The square dimensions span the regimes the product runs in: 64 is the smallest dimension "
    "at which the O(kn) operand restoration and the three scratch allocations are still a "
    "noticeable share of an O(mkn) kernel, 256 is the middle dimension the consumer boundary is "
    "also measured at, and 512 is the largest, where both representations' operands leave the "
    "per-core caches. Every cell is warm: one untimed pass over the working set precedes "
    "calibration. No cell is cold, because this family's arm probes its per-call allocation count "
    "with untimed calls, which a cold cell's first-use contract forbids, and because the single "
    "table build a process pays is a once-per-process cost that the vector family's cold cells "
    "measure at the size where it dominates. No cell is streaming, because rotating eight square "
    "operand banks would measure bank rotation rather than the product, and the memory-resident "
    "end is what the 512 cells carry. Every cell is single-core: field::matrix::gemm has no "
    "parallel form and neither lane adds one, so a 6-, 12- or 24-CPU arm would time a harness "
    "thread pool rather than either lane, and none is declared. The measured executable links a "
    "counting global allocator so each cell reports what one call allocates; its counters are "
    "armed for one untimed probe call and disarmed for every timed window, where they cost one "
    "relaxed atomic load per allocation on both arms of a pair and cancel in the ratio the "
    "estimator forms. Frozen confirmation selection: P-20's tail-support bound admits six "
    "confirmatory comparisons on this family's first attempt, so the confirmation retains "
    "matmul-n256-element, matmul-n512-element, matmul-n256-whole-element, matmul-n256-wide, "
    "matmul-n512-wide and matmul-n256-whole-wide, which are both representations at the middle and "
    "the largest square dimension and at the consumer boundary, and it drops matmul-n64-element "
    "and matmul-n64-wide, whose dimension is the pilot's sizing shape for the restoration and "
    "allocation overhead rather than a dimension a consumer's product is dominated by; the dropped "
    "cells keep their pilot evidence. Frozen decision rule for the accelerated path: it is "
    "retained when no confirmatory cell records fail and at least one records pass, and is removed "
    "otherwise, including when every confirmatory cell records not-material or inconclusive, "
    "because the permanent second GF(2^8) dense-product route is not repaid by a gain no measured "
    "consumer shape shows. The rule reads the acceptance summary's recorded outcomes and nothing "
    "else. Direction agreement is stated against the accepted matrix-family confirmation receipt "
    "of issue 19513245, pinned by path and SHA-256 in "
    "dev/active/4c1e441f/pinned-matrix-confirmation.json; that receipt measured a prototype that "
    "rebuilt its table per product, converted its operands and did not write results in place, so "
    "it is cited for the direction and not inherited as evidence."
)

WORTHWHILE_RATIONALE = (
    "Retaining the accelerated path keeps a permanent second GF(2^8) dense-product route in "
    "gf2-core: a cached table type with a process-lifetime registry, a whole-product kernel that "
    "restores the transposed operand into scratch of its own, and an override on the shared "
    "whole-product hook for each of the two representations. Thirty percent of the consumer's "
    "measured runtime is the smallest return that repays that second path, and it is the point at "
    "which a consumer whose work is dominated by these products sees the change. The threshold is "
    "a complexity judgement, not a measurement one: the confirmation freezes a pilot-derived "
    "resolution far below it."
)

EQUIVALENCE_RATIONALE = (
    "Within fifteen percent the two routes are interchangeable for a consumer, which is the band "
    "in which this family reports a route as no improvement rather than as a regression. The "
    "confirmation freezes a pilot-derived resolution strictly below this margin."
)

MATERIAL_GAP_RATIONALE = (
    "No cell is a comparator-gap cell: every cell compares two routes of one gf2 executable over "
    "the same field rather than gf2 against an external library, so no material-gap threshold "
    "applies. The external gap at these shapes is the committed evidence of the byte-field "
    "comparison survey 6c6b09b1."
)

MAINTENANCE_RATIONALE = (
    "What retention keeps is production source, not new source: the cached-table module with its "
    "registry, its region kernel, its whole-product kernel and its single dispatch point, plus the "
    "hook overrides on the two GF(2^8) representations. Seven hundred lines with their rustdoc is "
    "the ceiling this family allows that route to occupy, and no new unsafe kernel is allowed, "
    "because a vectorised successor would be a separate proposal with its own feasibility "
    "evidence."
)

# square dimension, metric kind, workload seed
SHAPES = [
    ("n64", 64, "kernel-isolated", 4401),
    ("n256", 256, "kernel-isolated", 4402),
    ("n512", 512, "kernel-isolated", 4403),
    ("n256-whole", 256, "whole-consumer", 4402),
]

REPRESENTATIONS = ["element", "wide"]


def cell(shape, dimension, metric_kind, seed, representation):
    return {
        "cell_id": f"matmul-{shape}-{representation}",
        "objective": "improvement",
        "role": "exploratory",
        "workload": {
            "identity": f"gf256-0x11d-square-product-n{dimension}",
            "size": {"n": dimension},
            "seed": seed,
        },
        "metric_kind": metric_kind,
        "scaling": "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": metric_kind == "whole-consumer",
        "decoder": None,
    }


def main():
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, "dev/active/4c1e441f/addendum-v4-dense-product-pilot.json")
    addendum = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": "gf256-shipped-dense-product",
            "issue": "4c1e441f",
            "purpose": "consumer-family",
            "description": FAMILY_DESCRIPTION,
        },
        "frozen": {"frozen_utc": FROZEN_UTC},
        "effect": {
            "worthwhile_speedup": 1.3,
            "rationale": WORTHWHILE_RATIONALE,
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.15,
            "equivalence_rationale": EQUIVALENCE_RATIONALE,
            "material_gap_threshold": None,
            "material_gap_rationale": MATERIAL_GAP_RATIONALE,
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 700,
            "maintenance_rationale": MAINTENANCE_RATIONALE,
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": "dev/bench_results/4c1e441f/dense-product-family-ledger.jsonl",
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 2,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": [
            cell(*shape, representation)
            for representation in REPRESENTATIONS
            for shape in SHAPES
        ],
    }
    with open(output, "w") as handle:
        json.dump(addendum, handle, indent=2)
        handle.write("\n")
    print(f"{output}: {len(addendum['cells'])} exploratory cells")


if __name__ == "__main__":
    main()
