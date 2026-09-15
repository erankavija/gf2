#!/usr/bin/env python3
"""Write the pilot addenda of the byte-field consumer assessment (jit:19513245).

Usage: make-addenda.py PROTOCOL_VERSION FROZEN_UTC

Three families, one per scientific question, each with its own append-only
ledger:

* bytefield-consumer-vector: the vector-shaped consumer `FieldVec::axpy` and
  the region-shaped workload of the same arithmetic, a byte region in and out.
* bytefield-consumer-matrix: the matrix-shaped consumer `field::matrix::gemm`,
  on the matrices the consumer holds and across the `FieldMatrix` boundary.
* bytefield-consumer-control: arbitrary independent multiplication, the
  control that separates what coefficient reuse contributes from what byte
  arithmetic contributes on its own.

Every cell is exploratory: a pilot sizes its family's measurement resolution,
records both routes' costs and decides nothing. The cell tables below are the
whole design; the effect and budget text is shared.

Every cell is an `improvement` cell whose baseline is the current gf2-core
route and whose candidate is the byte-field prototype, so a speedup above one
favours the prototype.
"""

import json
import sys

ISSUE = "19513245"
LEDGERS = f"dev/bench_results/{ISSUE}"
KIB = 1024
MIB = 1024 * KIB

RNG = (
    "Operands come from SplitMix64 [Steele2014] as implemented in tuning-campaign-support 0.1.0 "
    "(dev/tools/tuning-campaign-support/src/abtest.rs), seeded with the cell's workload seed and "
    "read through OperandStream in dev/active/6c6b09b1/survey/arm-common/src/lib.rs: each operand "
    "byte is the low byte of one output, and the fixed coefficient of a region or vector cell is "
    "the next output's low byte with its low bit set, so no cell measures the zero coefficient "
    "(the validation covers that case and every other byte coefficient)."
)
SCOPE = (
    "Every cell is single-core: FieldVec::axpy, field::matrix::gemm and gf2m::batch::batch_mul "
    "have no parallel form in these builds and neither has the prototype, so a 6-, 12- or 24-CPU "
    "arm would time a harness thread pool rather than either route, and none is declared."
)
FIELD = (
    "Every cell works in GF(2^8) modulo 0x11D (x^8+x^4+x^3+x^2+1), the field Gf2mField::gf256() "
    "builds and the field the byte-field comparison survey 6c6b09b1 measured every pinned library "
    "in, so a cell of this assessment and a cell of that survey at the same shape describe the "
    "same arithmetic. The prototype builds its table from the field's own reduction polynomial and "
    "is validated over 0x11B as well; no cell measures 0x11B, because gf2 ships no GF(2^8) field "
    "modulo 0x11B and the comparison survey has no ISA-L baseline for one."
)
ARMS = (
    "A cell identifier ends in the representation the consumer holds: element = "
    "FieldVec/FieldMatrix<Gf2mElement> over Gf2mField::gf256(); wide = "
    "FieldVec/FieldMatrix<Gf2mWide<1,Gf256x11d>>, the survey's 0x11D Gf2mWideConfig; batch = the "
    "u64 lanes gf2m::batch::batch_mul reads. The baseline arm is that representation's current "
    "gf2-core route and the candidate arm is the byte-field prototype over the same field, both "
    "from one executable that selects the route from its environment, so the two arms of a pair "
    "share a build identity and differ only in the route. Both arms are conservative-portable "
    "builds with no target-cpu setting, because that is the build a gf2 consumer gets: the "
    "carry-less-multiply kernels the current routes reach are dispatched at run time and are "
    "present in such a build, while a -march=native build would give the prototype's scalar loop "
    "an auto-vectorisation no shipped library provides."
)
PROTOTYPE = (
    "The prototype is byte-oriented GF(2^8) arithmetic: the products of one fixed coefficient with "
    "all 256 field elements are a 256-byte table, so multiplying by a reused coefficient costs one "
    "indexed load and accumulating costs one XOR, whatever width the element is stored in. A "
    "vector cell prepares that table inside the timed call, because the consumer passes a new "
    "coefficient with every call. A matrix or control cell reaches the full 256-by-256 table "
    "instead, which depends on the field alone; it is prepared during preparation and reported as "
    "setup, because a production design would build it once per field. The prototype is safe "
    "scalar Rust and uses no intrinsic."
)

EFFECT = {
    "worthwhile_speedup": 1.30,
    "rationale": (
        "Adopting the prototype means gf2-core carries a second GF(2^8) multiplication "
        "implementation: a table type with a lifecycle the consumer has to manage, a dispatch "
        "branch in each consumer that uses it, and a conformance surface over every byte "
        "coefficient of every supported polynomial. Thirty percent of the consumer's measured "
        "runtime is the smallest return that repays that permanent second path, and it is "
        "the point at which a consumer whose work is dominated by these calls sees the change. "
        "The threshold is a complexity judgement, not a measurement one: the confirmation freezes "
        "a pilot-derived resolution far below it."
    ),
    "measurement_resolution": None,
    "resolution_evidence": None,
    "equivalence_margin": 1.15,
    "equivalence_rationale": (
        "Within fifteen percent the two routes are interchangeable for a consumer, which is the "
        "band in which this assessment would report a route as no improvement rather than as a "
        "regression. The confirmation freezes a pilot-derived resolution strictly below this "
        "margin."
    ),
    "material_gap_threshold": None,
    "material_gap_rationale": (
        "No cell is a comparator-gap cell: every cell compares two gf2 routes over the same field "
        "rather than gf2 against an external library, so no material-gap threshold applies. The "
        "external gap at these shapes is the committed evidence of the byte-field comparison "
        "survey 6c6b09b1."
    ),
}

BUDGET = {
    "max_new_unsafe_kernels": 0,
    "max_added_source_lines": 400,
    "maintenance_rationale": (
        "The prototype is safe scalar Rust: a table type, one hook override per consumer and the "
        "table lifecycle around them. Four hundred lines with their rustdoc and their conformance "
        "hooks is what that shape costs at this repository's documentation density, and no new "
        "unsafe kernel is allowed, because a vectorised successor would be a separate proposal "
        "with its own feasibility evidence. This issue adopts nothing, so the budget bounds the "
        "proposal a positive outcome would put up for review."
    ),
}


def cell(cell_id, identity, size, seed, metric, cache):
    whole = metric == "whole-consumer"
    return {
        "cell_id": cell_id,
        "objective": "improvement",
        "role": "exploratory",
        "workload": {"identity": identity, "size": size, "seed": seed},
        "metric_kind": metric,
        "scaling": "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache,
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": whole,
        "decoder": None,
    }


KI, WC = "kernel-isolated", "whole-consumer"


def vector_cells():
    """Vector-shaped cells in place on the consumer's own vector, and
    region-shaped cells from and to a byte region."""
    warm = {"4k": (4 * KIB, 1101), "128k": (128 * KIB, 1102), "8m": (8 * MIB, 1103)}
    cells = []
    for repr_tag in ("element", "wide"):
        for label, (nbytes, seed) in warm.items():
            cells.append((f"axpy-{label}-{repr_tag}", "vector", nbytes, seed, KI, "warm"))
        cells.append((f"axpy-2m-stream-{repr_tag}", "vector", 2 * MIB, 1104, KI, "streaming"))
        for label in ("4k", "128k"):
            nbytes, seed = warm[label]
            cells.append((f"region-{label}-{repr_tag}", "region", nbytes, seed, WC, "warm"))
    out = []
    for cell_id, shape, nbytes, seed, metric, cache in cells:
        identity = f"gf256-0x11d-{shape}-axpy-{nbytes}-bytes"
        out.append(cell(cell_id, identity, {"bytes": nbytes}, seed, metric, cache))
    return out


def matrix_cells():
    """Dense products on the matrices the consumer holds, and across the
    FieldMatrix boundary the library consumer starts and ends at."""
    squares = {64: 1201, 256: 1202, 512: 1203}
    cells = []
    for repr_tag in ("element", "wide"):
        for n, seed in squares.items():
            cells.append((f"matmul-n{n}-{repr_tag}", n, seed, KI))
        cells.append((f"matmul-n256-whole-{repr_tag}", 256, 1202, WC))
    return [
        cell(cell_id, f"gf256-0x11d-square-product-n{n}", {"n": n}, seed, metric, "warm")
        for cell_id, n, seed, metric in cells
    ]


def control_cells():
    """Arbitrary independent multiplication: the control."""
    cells = [
        ("pairwise-4k-batch", 4 * KIB, 1301, KI),
        ("pairwise-128k-batch", 128 * KIB, 1302, KI),
        ("pairwise-128k-whole-batch", 128 * KIB, 1302, WC),
    ]
    return [
        cell(cell_id, f"gf256-0x11d-pairwise-multiply-{nbytes}-bytes", {"bytes": nbytes}, seed,
             metric, "warm")
        for cell_id, nbytes, seed, metric in cells
    ]


FAMILIES = {
    "vector": (
        "bytefield-consumer-vector",
        "Question: does byte-oriented GF(2^8) arithmetic make gf2's fixed-coefficient "
        "multiply-accumulate faster for the consumer, after every cost the byte-oriented route "
        "adds? Two shapes answer it and are kept apart. A vector cell measures FieldVec::axpy in "
        "place on the vector the consumer already holds, at 4 KiB, 128 KiB and 8 MiB warm and at "
        "2 MiB rotated through eight banks, for both element representations: the prototype "
        "converts nothing there and pays only its table, so the cell isolates the arithmetic. A "
        "region cell starts and ends at a byte region at 4 KiB and 128 KiB, so the current route "
        "pays its conversion into the representation and out again while the prototype works on "
        "the region itself: that is the region-shaped workload, and it is a different question "
        "from the vector one. ",
        vector_cells,
    ),
    "matrix": (
        "bytefield-consumer-matrix",
        "Question: does byte-oriented GF(2^8) arithmetic make gf2's dense product faster for the "
        "consumer, after the conversion a byte-oriented product needs? A kernel-isolated cell "
        "times each route on the representation it holds, at square dimensions 64, 256 and 512 "
        "for both element representations, which covers the two different accelerated paths gemm "
        "reaches: the element representation calls the carry-less-multiply batch dot product once "
        "per output cell and the wide representation takes the whole product through the "
        "carry-less-multiply GEMM kernel. A whole-consumer cell fixes the boundary at the "
        "FieldMatrix a library consumer holds, so there the prototype pays converting both "
        "operands out of the representation and the product back into it, and the current route "
        "pays nothing beyond what it already pays. A dense product is not a region "
        "multiply-accumulate and shares no cell or family with one. ",
        matrix_cells,
    ),
    "control": (
        "bytefield-consumer-control",
        "Question: how much of any vector or matrix result belongs to coefficient reuse rather "
        "than to byte arithmetic as such? Arbitrary independent multiplication z[i] = x[i]*y[i] "
        "reuses no coefficient, so its byte-oriented route can amortise no coefficient table and "
        "reaches the full 256-by-256 table instead, while the current route is "
        "gf2m::batch::batch_mul on u64 lanes, the one gf2 GF(2^8) consumer that already dispatches "
        "to a carry-less-multiply batch kernel. Cells at 4 KiB and 128 KiB kernel-isolated and at "
        "128 KiB from and to a byte region. This family is the control: it shares no cell with the "
        "reuse families and decides no adoption of its own. ",
        control_cells,
    ),
}


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__.strip())
    version = int(sys.argv[1])
    frozen = sys.argv[2]
    for short, (family, question, cells) in FAMILIES.items():
        addendum = {
            "schema": f"zen3-benchmark-addendum-v{version}",
            "protocol": {"id": "zen3-benchmark-protocol", "version": version},
            "family": {
                "id": family,
                "issue": ISSUE,
                "purpose": "consumer-family",
                "description": (
                    f"Exploratory pilot of issue {ISSUE}, protocol version {version}: this "
                    f"addendum is the pilot and every cell is exploratory. " + question + FIELD
                    + " " + ARMS + " " + PROTOTYPE + " " + RNG + " " + SCOPE
                ),
            },
            "frozen": {"frozen_utc": frozen},
            "effect": EFFECT,
            "complexity_budget": BUDGET,
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": f"{LEDGERS}/{short}-family-ledger.jsonl",
            },
            "search_budget": {
                "max_pilot_trials_per_cell": 2,
                "max_confirmatory_attempts_per_candidate": 1,
            },
            "holdout": {"required": False, "cells": []},
            "cells": cells(),
        }
        path = f"dev/active/{ISSUE}/addendum-v{version}-{short}-pilot.json"
        with open(path, "w") as output:
            json.dump(addendum, output, indent=2)
            output.write("\n")
        print(f"{path}: {len(addendum['cells'])} exploratory cells")


if __name__ == "__main__":
    main()
