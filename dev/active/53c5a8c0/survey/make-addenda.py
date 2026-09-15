#!/usr/bin/env python3
"""Write the 53c5a8c0 pilot addenda from the frozen cell grid.

Usage: make-addenda.py --frozen-utc <YYYY-MM-DDTHH:MM:SSZ> [--output-dir <dir>]

Both families' pilot addenda are generated from `cells.py`, so an addendum can
never declare a cell the plan does not run or a size the plan does not use. The
confirmation addenda are not written here: they are derived from a committed
pilot receipt by `dev/active/c7113c5a/survey/freeze-confirmation.py`, which
pins that receipt and stamps the measurement resolution it observed.

Every margin and budget below is a declaration made before measurement. The
rationales state what the number is for; none of them is derived from a result.
"""

import argparse
import copy
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cells as grid  # noqa: E402

CROSSOVER_DESCRIPTION = (
    "Where the batched carry-less paths of gf2-core overtake the per-element "
    "paths a consumer can call instead, across a small/crossover/resident/"
    "streaming size grid. Baseline arm: the per-element path — the bundle's "
    "single carry-less multiply in a loop, FieldVec::dot_product, and "
    "Gf2mElement multiplication one pair at a time. Candidate arm: the batched "
    "path — the bundle's raw-batch kernel, FieldVec::simd_dot_product, and "
    "gf2m::batch::batch_mul. One executable serves both arms and selects the "
    "entry point from GF2_CROSSOVER_PATH, so the arms share a build, a fixture "
    "generator and a timing loop. The three operations are kept in separate "
    "cells: the raw independent 64x64 carry-less batch is not the reduced "
    "GF(2^m) element-wise batch and neither is the GF(2^m) dot product. The "
    "raw-batch cells are kernel-isolated; the two GF(2^m) consumer cells are "
    "whole-consumer, with packing, the operation and the extraction of the "
    "result inside every timed call and both arms paying the identical "
    "packing. The family calibrates a selector: its recommendation is a length "
    "threshold in the canonical tuning mechanism, which its holdout cells test "
    "at sizes that took no part in selection. This addendum is the pilot: "
    "every cell is exploratory, it fixes the measurement resolution the "
    "confirmation freezes against, and none of its samples enter a "
    "confirmation."
)

POLYNOMIAL_DESCRIPTION = (
    "How the current gf2 long product and the whole-consumer wide GF(2^m) "
    "field product compare with the pinned gf2x 1.3.0 build across operand "
    "widths, and where along that grid the comparison changes direction. "
    "Baseline arm: gf2 — the public clmul_wide_slice, which routes through "
    "routes through the crate's capability dispatch and overwrites a fresh "
    "destination as an external long-product library does, and "
    "Gf2mWide::mul_ref, which composes that dispatch with BarrettReducerWide. "
    "The accumulating clmul_wide_slice form is not measured here: timing a "
    "destination clear and an XOR accumulation against a library that performs "
    "neither would not be an equivalent operation. Candidate arm: gf2x_mul_r from the "
    "pinned build, composed with the same gf2 reducer where the cell is a "
    "field product, so the two arms differ in the product stage alone. A "
    "speedup of medians below one means gf2 is faster. The whole-consumer "
    "wide-field cells are the composed operation no earlier receipt measured: "
    "they carry the operand masking, the product and the field reduction "
    "inside the timed call. The raw independent carry-less batch has no "
    "external equivalent — gf2x exposes no batch entry point — so its cell "
    "composes one single-word gf2x call per product and is recorded as a "
    "composed arm bounding what a consumer reaching gf2x through its public "
    "API pays, not as an equivalent operation and not as a gf2x basecase rate. "
    "The study adopts no gf2x dependency and changes no gf2 multiplication. "
    "This addendum is the pilot: every cell is exploratory, it fixes the "
    "measurement resolution the confirmation freezes against, and none of its "
    "samples enter a confirmation."
)

CROSSOVER_EFFECT = {
    "worthwhile_speedup": 1.15,
    "rationale": (
        "The recommendation this family can support is a length threshold the "
        "batched paths do not have today: a selector field in the canonical "
        "tuning mechanism, its read site, its compile-time counterpart and the "
        "calibration that keeps it honest. Fifteen percent is the smallest "
        "gain on a short vector that repays that standing cost, because a "
        "consumer whose short calls are a minority of its work sees the "
        "threshold's benefit diluted by exactly that share. Below it the study "
        "recommends retaining the established batched path rather than adding "
        "a branch."
    ),
    "measurement_resolution": None,
    "resolution_evidence": None,
    "equivalence_margin": 1.05,
    "equivalence_rationale": (
        "Five percent is the slowdown at which a consumer would notice the "
        "batched path on a size where it is not winning; a cell inside that "
        "margin is recorded as not material rather than as a crossover."
    ),
    "material_gap_threshold": 1.25,
    "material_gap_rationale": (
        "A quarter of the runtime is the smallest gap between the two paths "
        "that would justify work on the losing path itself rather than a "
        "selector that avoids it; smaller gaps are reported and not chased."
    ),
}

POLYNOMIAL_EFFECT = {
    "worthwhile_speedup": 1.15,
    "rationale": (
        "No cell of this family adopts anything: it measures a comparator gap "
        "under the study's exclusion of a gf2x dependency and of any gf2 "
        "multiplication change. The threshold is declared because the schema "
        "requires one for every family, and it is set at the same fifteen "
        "percent the crossover family uses so a reader comparing the two "
        "families reads one scale."
    ),
    "measurement_resolution": None,
    "resolution_evidence": None,
    "equivalence_margin": 1.05,
    "equivalence_rationale": (
        "Five percent bounds the region in which neither library is usefully "
        "faster at a width; a cell inside it establishes parity at that width "
        "rather than a direction."
    ),
    "material_gap_threshold": 1.25,
    "material_gap_rationale": (
        "A gap of a quarter is the smallest one worth attributing to an "
        "algorithm or an instruction mix rather than to call overhead, so it "
        "is the threshold at which a comparator-gap cell asks for attribution."
    ),
}

CROSSOVER_BUDGET = {
    "max_new_unsafe_kernels": 0,
    "max_added_source_lines": 250,
    "maintenance_rationale": (
        "The study changes no production code. The budget bounds the change "
        "its recommendation would authorise elsewhere: one selector field in "
        "the canonical mechanism with its validation, its compile-time "
        "counterpart, the read site in the batched entry point and the rustdoc "
        "that states the new mechanism. A recommendation whose implementation "
        "would exceed that is reported for separate tracking instead."
    ),
}

POLYNOMIAL_BUDGET = {
    "max_new_unsafe_kernels": 0,
    "max_added_source_lines": 0,
    "maintenance_rationale": (
        "The family adopts nothing. gf2x is GPL-3.0-or-later in the measured "
        "configuration and gf2 is MIT, and the study's scope excludes both a "
        "dependency on it and any change to gf2 multiplication, so no source "
        "line is available to spend."
    ),
}


def addendum(family, description, effect, budget, ledger, purpose, frozen_utc, holdout):
    return {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": family,
            "issue": "53c5a8c0",
            "purpose": purpose,
            "description": description,
        },
        "frozen": {"frozen_utc": frozen_utc},
        "effect": effect,
        "complexity_budget": budget,
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": ledger,
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 2,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": holdout,
        "cells": [grid.addendum_cell(cell) for cell in grid.cells_of(family)],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--frozen-utc", required=True)
    parser.add_argument("--output-dir", default="dev/active/53c5a8c0")
    parser.add_argument(
        "--resolution-stage",
        help="write a second-round resolution pilot under this suffix, holding "
        "only the cells named by --cell",
    )
    parser.add_argument("--cell", action="append", default=[], dest="cells")
    args = parser.parse_args()
    if bool(args.resolution_stage) != bool(args.cells):
        raise SystemExit("a resolution stage needs both --resolution-stage and --cell")

    written = []
    for name, family, description, effect, budget, ledger, purpose, holdout in (
        (
            "addendum-v4-crossover-pilot.json",
            grid.CROSSOVER_FAMILY,
            CROSSOVER_DESCRIPTION,
            CROSSOVER_EFFECT,
            CROSSOVER_BUDGET,
            "dev/bench_results/53c5a8c0/gf2m-clmul-crossover-ledger.jsonl",
            # The pilot stage is an exploratory consumer grid, not a
            # calibration: a holdout confirms a selector, and the pilot has
            # calibrated none. The confirmation stage carries the
            # selector-calibration purpose together with the holdout cells the
            # committed declaration fixes.
            "consumer-family",
            {"required": False, "cells": []},
        ),
        (
            "addendum-v4-polynomial-pilot.json",
            grid.POLYNOMIAL_FAMILY,
            POLYNOMIAL_DESCRIPTION,
            POLYNOMIAL_EFFECT,
            POLYNOMIAL_BUDGET,
            "dev/bench_results/53c5a8c0/wide-polynomial-competitiveness-ledger.jsonl",
            "consumer-family",
            {"required": False, "cells": []},
        ),
    ):
        document = addendum(family, description, effect, budget, ledger, purpose,
                            args.frozen_utc, holdout)
        if args.resolution_stage:
            if family != grid.CROSSOVER_FAMILY:
                continue
            name = name.replace("-pilot.json", f"-{args.resolution_stage}.json")
            declared = {cell["cell_id"] for cell in document["cells"]}
            unknown = [cell for cell in args.cells if cell not in declared]
            if unknown:
                raise SystemExit(f"unknown cells: {unknown}")
            document = copy.deepcopy(document)
            document["cells"] = [
                cell for cell in document["cells"] if cell["cell_id"] in set(args.cells)
            ]
            document["family"]["description"] = (
                document["family"]["description"]
                + " This addendum is the family's second-round resolution pilot: "
                "it re-measures the cells the confirmation will carry at the "
                "protocol's maximum pilot pair count, so the resolution the "
                "confirmation freezes against is estimated on the sample size "
                "the confirmation itself uses. Its samples enter no "
                "confirmation."
            )
        path = os.path.join(args.output_dir, name)
        with open(path, "w") as handle:
            json.dump(document, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
        written.append(path)
    print("\n".join(written), file=sys.stderr)


if __name__ == "__main__":
    main()
