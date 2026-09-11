#!/usr/bin/env python3
"""Write the protocol-v3 pilot addenda of the byte-field survey (jit:6c6b09b1).

Usage: make-addenda-v3.py FROZEN_UTC

One family per scientific question, each with its own append-only ledger:

* byte-field-region-axpy: fixed-coefficient region multiply-accumulate,
  `FieldVec::axpy` against the pinned external region kernels.
* byte-field-matrix-product: dense matrix products, `field::matrix::gemm`
  against M4RIE at square shapes and ISA-L/M4RIE at the generator-encode
  shape.
* byte-field-pairwise-control: arbitrary pairwise products, the control that
  keeps pairwise multiplication apart from fixed-coefficient reuse.

Every cell is exploratory: a pilot sizes its family's measurement
resolution, records the arms' current costs and decides nothing. The cell
tables below are the whole design; the effect and budget text is shared.
"""

import json
import sys

ISSUE = "6c6b09b1"
LEDGERS = "dev/bench_results/6c6b09b1"
KIB = 1024
MIB = 1024 * KIB

RNG = (
    "Operands come from the in-harness SplitMix64 [Steele2014] of "
    "dev/active/6c6b09b1/survey/arm-common/src/lib.rs (SplitMix64::fill: the low byte of "
    "each output), seeded with the cell's workload seed; the coefficient of a region cell is "
    "the next output with its low bit set."
)
SCOPE = (
    "Every cell is single-core: none of the measured gf2 entry points (FieldVec::axpy, "
    "field::matrix::gemm, gf2m::batch::batch_mul) and none of the external region, "
    "element or matrix calls has a parallel form in these builds (M4RI is configured "
    "without OpenMP), so a 6-, 12- or 24-CPU arm would time a harness thread pool rather "
    "than either library, and none is declared."
)
FIELD = (
    "Every cell works in GF(2^8) modulo 0x11D (x^8+x^4+x^3+x^2+1), the one field all "
    "compared arms share: ISA-L implements no other, and the gf2 arms use "
    "Gf2mField::gf256() or the survey's Gf2mWideConfig with the same polynomial. gf2 "
    "ships no GF(2^8) field modulo 0x11B, so no cell uses it."
)
ARMS = (
    "Arm names in a cell identifier: element = FieldVec/FieldMatrix<Gf2mElement>; wide = "
    "FieldVec/FieldMatrix<Gf2mWide<1,Gf256x11d>>; batch = gf2m::batch::batch_mul on u64 "
    "lanes; isal, gfcomplete, m4rie = the pinned libraries [IsaL2026] [GfComplete2026] "
    "[Mfourrie2026] through the survey's C shim. "
    "In a cell <x>-vs-<y> the baseline is x and the candidate y, so a speedup above one "
    "favours y; with gf2 as baseline, a value below one means gf2 is faster."
)

EFFECT = {
    "worthwhile_speedup": None,
    "rationale": (
        "No cell is an improvement cell: the survey compares current gf2 consumer paths "
        "with pinned external libraries and adopts nothing, so no worthwhile-speedup "
        "threshold applies."
    ),
    "measurement_resolution": None,
    "resolution_evidence": None,
    "equivalence_margin": 1.15,
    "equivalence_rationale": (
        "Within fifteen percent a consumer choosing between gf2 and an external byte-field "
        "library decides on licensing, portability and the conversion it must add, not on "
        "the kernel speed. The confirmation freezes a pilot-derived resolution strictly "
        "below this margin."
    ),
    "material_gap_threshold": 1.25,
    "material_gap_rationale": (
        "A quarter of the runtime is the smallest gap that survives the byte conversion a "
        "gf2 consumer would add to reach an external library, so it is the smallest gap "
        "that would change a decision about the gf2 path for the downstream feasibility "
        "work. The confirmation freezes a pilot-derived resolution strictly below it."
    ),
}

BUDGET = {
    "max_new_unsafe_kernels": 0,
    "max_added_source_lines": 0,
    "maintenance_rationale": (
        "The survey establishes comparison arms and baselines and changes no production "
        "kernel or library, so its budget for production change is zero by construction."
    ),
}


def cell(cell_id, identity, size, seed, metric, cache, baseline, candidate):
    whole = metric == "whole-consumer"
    return {
        "cell_id": cell_id,
        "objective": "comparator-gap",
        "role": "exploratory",
        "workload": {"identity": identity, "size": size, "seed": seed},
        "metric_kind": metric,
        "scaling": "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache,
        "builds": {"baseline": baseline, "candidate": candidate},
        "conversion_costs_included": whole,
        "decoder": None,
    }


BUILD = {"element": "native", "wide": "native", "batch": "native",
         "isal": "external", "gfcomplete": "external", "m4rie": "external"}
KI, WC = "kernel-isolated", "whole-consumer"


def region_cells():
    sizes = {"4k": (4 * KIB, 301), "128k": (128 * KIB, 302), "8m": (8 * MIB, 303)}
    cells = []
    for gf2 in ("element", "wide"):
        for label, (nbytes, seed) in sizes.items():
            cells.append((f"axpy-{label}-{gf2}-vs-isal", nbytes, seed, KI, "warm", gf2, "isal"))
        cells.append((f"axpy-2m-stream-{gf2}-vs-isal", 2 * MIB, 304, KI, "streaming", gf2, "isal"))
        for label in ("4k", "128k"):
            nbytes, seed = sizes[label]
            cells.append((f"axpy-{label}-whole-{gf2}-vs-isal", nbytes, seed, WC, "warm", gf2, "isal"))
    for label in ("4k", "128k"):
        nbytes, seed = sizes[label]
        for other in ("gfcomplete", "m4rie"):
            cells.append((f"axpy-{label}-isal-vs-{other}", nbytes, seed, KI, "warm", "isal", other))
    cells.append(("axpy-128k-element-vs-wide", 128 * KIB, 302, KI, "warm", "element", "wide"))
    return [cell(cid, f"gf256-0x11d-region-axpy-{n}-bytes", {"bytes": n}, seed, metric, cache,
                 BUILD[b], BUILD[c])
            for cid, n, seed, metric, cache, b, c in cells]


def matrix_cells():
    cells = []
    squares = {64: 401, 256: 402, 512: 403}
    for gf2 in ("element", "wide"):
        for n, seed in squares.items():
            cells.append((f"matmul-n{n}-{gf2}-vs-m4rie", "square", {"n": n}, seed, KI, gf2, "m4rie"))
        cells.append((f"matmul-n256-whole-{gf2}-vs-m4rie", "square", {"n": 256}, 402, WC, gf2, "m4rie"))
    encode = {"k": 10, "rows": 4, "bytes": 64 * KIB}
    for gf2 in ("element", "wide"):
        cells.append((f"encode-k10r4-64k-{gf2}-vs-isal", "encode", encode, 404, KI, gf2, "isal"))
        cells.append((f"encode-k10r4-64k-whole-{gf2}-vs-isal", "encode", encode, 404, WC, gf2, "isal"))
    cells.append(("encode-k10r4-64k-isal-vs-m4rie", "encode", encode, 404, KI, "isal", "m4rie"))
    cells.append(("matmul-n256-element-vs-wide", "square", {"n": 256}, 402, KI, "element", "wide"))
    out = []
    for cid, shape, size, seed, metric, b, c in cells:
        if shape == "square":
            identity = f"gf256-0x11d-square-product-n{size['n']}"
        else:
            identity = (f"gf256-0x11d-generator-encode-{size['rows']}x{size['k']}-by-"
                        f"{size['k']}x{size['bytes']}")
        out.append(cell(cid, identity, size, seed, metric, "warm", BUILD[b], BUILD[c]))
    return out


def pairwise_cells():
    sizes = {"4k": (4 * KIB, 501), "128k": (128 * KIB, 502)}
    cells = []
    for other in ("gfcomplete", "isal", "m4rie"):
        for label, (nbytes, seed) in sizes.items():
            cells.append((f"pairwise-{label}-batch-vs-{other}", nbytes, seed, KI, other))
        cells.append((f"pairwise-128k-whole-batch-vs-{other}", 128 * KIB, 502, WC, other))
    return [cell(cid, f"gf256-0x11d-pairwise-multiply-{n}-bytes", {"bytes": n}, seed, metric,
                 "warm", BUILD["batch"], BUILD[c])
            for cid, n, seed, metric, c in cells]


FAMILIES = {
    "region-axpy": (
        "byte-field-region-axpy",
        "Exploratory pilot of issue 6c6b09b1, protocol version 3. Question: how much slower "
        "is gf2's fixed-coefficient GF(2^8) region multiply-accumulate y[i] += a*x[i] "
        "(FieldVec::axpy, one coefficient reused across the region, accumulation by XOR, "
        "source and destination distinct) than the fastest pinned external region kernel, "
        "per cache regime (4 KiB, 128 KiB and 8 MiB byte regions warm; 2 MiB regions "
        "rotated through eight banks), kernel-isolated and as a whole byte-region consumer "
        "that converts its operands in and its result out? Both gf2 element "
        "representations are measured against ISA-L gf_vect_mad; the isal-vs-gfcomplete "
        "and isal-vs-m4rie cells establish which external arm is fastest (GF-Complete "
        "multiply_region.w32 with add=1; M4RIE mzed_add_multiple_of_row on one-row "
        "matrices); element-vs-wide compares the two gf2 representations directly. ",
        region_cells,
    ),
    "matrix-product": (
        "byte-field-matrix-product",
        "Exploratory pilot of issue 6c6b09b1, protocol version 3. Question: how much slower "
        "is gf2's dense GF(2^8) matrix product (field::matrix::gemm on FieldMatrix) than "
        "the pinned external matrix products, at square dimensions 64, 256 and 512 "
        "(M4RIE mzed_mul, whose recursion at each dimension the arm provenance record "
        "observes) and at the generator-encode shape 4x10 by 10x65536 that ISA-L "
        "ec_encode_data computes, "
        "kernel-isolated and as a whole byte-matrix consumer? Both gf2 element "
        "representations are measured; isal-vs-m4rie establishes the faster external arm "
        "at the encode shape; element-vs-wide compares the gf2 representations directly. "
        "A matrix product is not a region multiply-XOR and shares no cell with the region "
        "family. ",
        matrix_cells,
    ),
    "pairwise-control": (
        "byte-field-pairwise-control",
        "Exploratory pilot of issue 6c6b09b1, protocol version 3. Question: how does gf2's "
        "arbitrary pairwise GF(2^8) product z[i] = x[i]*y[i] (gf2m::batch::batch_mul on u64 "
        "lanes, no coefficient reuse) compare with the only pairwise form each pinned "
        "library offers, its single-element multiply applied per byte (ISA-L gf_mul, "
        "GF-Complete multiply.w32, M4RIE gf2e_mul), at 4 KiB and 128 KiB byte regions, "
        "kernel-isolated and as a whole byte-region consumer that widens to lanes and "
        "narrows back? No library has a pairwise region kernel; this "
        "family is the control that keeps pairwise multiplication apart from the "
        "fixed-coefficient reuse the region family measures. ",
        pairwise_cells,
    ),
}


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__.strip())
    frozen = sys.argv[1]
    for short, (family, question, cells) in FAMILIES.items():
        addendum = {
            "schema": "zen3-benchmark-addendum-v3",
            "protocol": {"id": "zen3-benchmark-protocol", "version": 3},
            "family": {
                "id": family,
                "issue": ISSUE,
                "purpose": "consumer-family",
                "description": question + FIELD + " " + ARMS + " " + RNG + " " + SCOPE,
            },
            "frozen": {"frozen_utc": frozen},
            "effect": EFFECT,
            "complexity_budget": BUDGET,
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": f"{LEDGERS}/v3-{short}-family-ledger.jsonl",
            },
            "search_budget": {
                "max_pilot_trials_per_cell": 2,
                "max_confirmatory_attempts_per_candidate": 1,
            },
            "holdout": {"required": False, "cells": []},
            "cells": cells(),
        }
        path = f"dev/active/{ISSUE}/addendum-v3-{short}-pilot.json"
        with open(path, "w") as output:
            json.dump(addendum, output, indent=2)
            output.write("\n")
        print(f"{path}: {len(addendum['cells'])} exploratory cells")


if __name__ == "__main__":
    main()
