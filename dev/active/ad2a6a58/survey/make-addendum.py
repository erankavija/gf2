#!/usr/bin/env python3
"""Write the frozen pilot addendum of the GF(2^8) axpy family (jit:ad2a6a58).

Usage: make-addendum.py [output]
       (default dev/active/ad2a6a58/addendum-v4-axpy-pilot.json)

Every cell of this family measures the same operation through the same entry
point and differs only in the element representation, the operand size and the
cache state, so the cells are projected from one table rather than written out
one by one. Nothing here is a measurement: the file it writes is a frozen
declaration, and the confirmation stage derives its own addendum from the
committed pilot receipt with the repository's canonical freezer.
"""

import json
import os
import subprocess
import sys

FROZEN_UTC = "2026-09-17T21:44:04Z"

FAMILY_DESCRIPTION = (
    "Exploratory pilot of issue ad2a6a58, protocol version 4: this addendum is the pilot and "
    "every cell is exploratory. Question: does the GF(2^8) product-table lane that gf2-core "
    "ships make FieldVec::axpy faster than the route the library takes without it, on the "
    "representations and sizes a consumer holds? Both arms are one executable, built from the "
    "shipped crate with the simd and test-support features. The baseline arm holds every GF(2^8) "
    "call on the scalar element lane through the shipped process-global lane switch "
    "(gf2m::force_scalar_gf256_table), so it runs the element loop FieldVec::axpy runs when the "
    "hook declines; the candidate arm leaves the switch clear and lets gf256_table_dispatch "
    "select the cached table. A pair therefore differs in the lane and in nothing else: one "
    "executable, one build identity, one set of operands, one entry point. Each execution reports "
    "the lane the shipped witness (gf2m::last_gf256_table_lane) recorded for its measured calls "
    "and the number of product tables the process built, both read after the timed windows. "
    "Every cell works in GF(2^8) modulo 0x11D (x^8+x^4+x^3+x^2+1), the field Gf2mField::gf256() "
    "builds. A cell identifier ends in the representation the consumer holds: element = "
    "FieldVec<Gf2mElement> over Gf2mField::gf256(); wide = FieldVec<Gf2mWide<1,Gf256x11d>>, the "
    "byte-field survey's 0x11D Gf2mWideConfig. Every cell is kernel-isolated, because the shipped "
    "hook reads and writes the representation the caller already holds and offers no byte region: "
    "there is no representation conversion to include, while the dispatch and, for the element "
    "representation, the field-handle pre-check the hook performs are inside the measured call. "
    "Sizes span the cache regimes: 4 KiB and 128 KiB warm are cache-resident, 8 MiB warm is the "
    "memory-resident end, and the 2 MiB streaming cell rotates through the eight fixture banks. "
    "The 1 KiB cells are cold with one frozen call per window, so the candidate's first timed "
    "window carries the one product-table build a process pays and no later call repeats; at that "
    "size a single call cannot amortize it, which is the first-touch cost the design's D-03 and "
    "RISK-02 name. Those two cells are expected to be structurally unstable as confirmatory "
    "cells rather than merely noisy: the protocol flags a window at or above twice its own "
    "execution median, so a first-touch build flags exactly one of every candidate execution's "
    "five windows, one in ten of the cell's windows across both arms, which is the whole of the "
    "frozen max_flagged_fraction with no margin left for any other flagged window. They are "
    "pilot cells for that reason and the selection rule below drops them from the confirmation. "
    "Every cell is single-core: FieldVec::axpy has no parallel form and neither lane adds one, so "
    "a 6-, 12- or 24-CPU arm would time a harness thread pool rather than either lane, and none "
    "is declared. Frozen confirmation selection: P-20's tail-support bound admits six "
    "confirmatory comparisons on this family's first attempt, so the confirmation retains "
    "axpy-4k-element, axpy-128k-element, axpy-2m-stream-element, axpy-4k-wide, axpy-128k-wide and "
    "axpy-2m-stream-wide, which are both representations at an L1-resident size, an L2-resident "
    "size and the streaming size, and it drops axpy-8m-element and axpy-8m-wide, whose "
    "memory-resident regime the streaming cells already carry, and axpy-1k-cold-element and "
    "axpy-1k-cold-wide for the instability above; the dropped cells keep their pilot evidence. "
    "Frozen decision rule for the shipped lane: the accelerated lane is retained when no "
    "confirmatory cell records fail and at least one records pass, and is removed otherwise, "
    "including when every confirmatory cell records not-material or inconclusive, because the "
    "permanent second GF(2^8) multiplication route is not repaid by a gain no measured consumer "
    "shape shows. The rule reads the acceptance summary's recorded outcomes and nothing else. "
    "Direction agreement is stated against the accepted vector-family confirmation receipt of "
    "issue 19513245, pinned by path and SHA-256 in "
    "dev/active/ad2a6a58/pinned-vector-confirmation.json; that receipt measured a prototype that "
    "converted operands and did not write results in place, so it is cited for the direction and "
    "not inherited as evidence, and its sizes need not match."
)

WORTHWHILE_RATIONALE = (
    "Retaining the shipped lane keeps a permanent second GF(2^8) multiplication route in "
    "gf2-core: a cached table type with a process-lifetime registry, a dispatch branch each "
    "consumer consults, and a conformance surface over every byte coefficient of every supported "
    "degree-8 polynomial. Thirty percent of the consumer's measured runtime is the smallest "
    "return that repays that second path, and it is the point at which a consumer whose work is "
    "dominated by these calls sees the change. The threshold is a complexity judgement, not a "
    "measurement one: the confirmation freezes a pilot-derived resolution far below it."
)

EQUIVALENCE_RATIONALE = (
    "Within fifteen percent the two lanes are interchangeable for a consumer, which is the band "
    "in which this family reports a lane as no improvement rather than as a regression. The "
    "confirmation freezes a pilot-derived resolution strictly below this margin."
)

MATERIAL_GAP_RATIONALE = (
    "No cell is a comparator-gap cell: every cell compares two lanes of one gf2 executable over "
    "the same field rather than gf2 against an external library, so no material-gap threshold "
    "applies. The external gap at these shapes is the committed evidence of the byte-field "
    "comparison survey 6c6b09b1."
)

MAINTENANCE_RATIONALE = (
    "What retention keeps is production source, not new source: the cached-table module with its "
    "registry, its region kernel and its single dispatch point, plus the hook overrides on the "
    "two GF(2^8) representations. Seven hundred lines with their rustdoc is the ceiling this "
    "family allows that route to occupy, and no new unsafe kernel is allowed, because a "
    "vectorised successor would be a separate proposal with its own feasibility evidence."
)

# size in bytes, cache state, frozen cold calls, workload seed
SHAPES = [
    ("1k-cold", 1024, "cold", 1, 4101),
    ("4k", 4096, "warm", None, 4102),
    ("128k", 131072, "warm", None, 4103),
    ("8m", 8388608, "warm", None, 4104),
    ("2m-stream", 2097152, "streaming", None, 4105),
]

REPRESENTATIONS = ["element", "wide"]


def cell(shape, size, cache_state, cold_calls, seed, representation):
    declaration = {
        "cell_id": f"axpy-{shape}-{representation}",
        "objective": "improvement",
        "role": "exploratory",
        "workload": {
            "identity": f"gf256-0x11d-vector-axpy-{size}-bytes",
            "size": {"bytes": size},
            "seed": seed,
        },
        "metric_kind": "kernel-isolated",
        "scaling": "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache_state,
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": False,
        "decoder": None,
    }
    if cold_calls is not None:
        declaration["cold_calls"] = cold_calls
    return declaration


def main():
    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    output = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        root, "dev/active/ad2a6a58/addendum-v4-axpy-pilot.json")
    addendum = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": "gf256-shipped-axpy-lane",
            "issue": "ad2a6a58",
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
            "ledger_path": "dev/bench_results/ad2a6a58/axpy-family-ledger.jsonl",
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
