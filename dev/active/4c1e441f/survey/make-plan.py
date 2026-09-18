#!/usr/bin/env python3
"""Project the runner plan of one GF(2^8) dense-product campaign (jit:4c1e441f).

Usage: make-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                    --lock PATH --executable BIN --producing-manifest M
                    --max-cells-per-session N [--pilot-pairs P] --output PLAN

The projection itself is `dev/scripts/campaign_plan.py`, shared with every other
lane-comparison family; this file declares what is this family's own: the two
lanes, the two element representations, the consumer entry point and the case a
dense-product cell decodes.
"""

import os
import subprocess
import sys

sys.path.insert(0, os.path.join(subprocess.run(
    ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
).stdout.strip(), "dev/scripts"))
import campaign_plan  # noqa: E402

POLY = 0x11D

WORKLOAD_PREFIX = "gf256-0x11d-square-product"

ENTRY_POINT = "the shipped gf2-core dense FieldMatrix product"

# Baseline lane first, candidate lane second.
LANES = {
    "scalar": "holds every GF(2^8) call on the route the library takes without the cached table "
              "through the shipped lane switch, which is a batched dot product per output cell "
              "for the runtime-context element and the carry-less-product whole-gemm kernel for "
              "the compile-time-configured value",
    "table": "leaves the shipped lane switch clear, so gf256_table_dispatch selects the cached "
             "product table and one region multiply-accumulate per left-hand coefficient",
}

REPRESENTATIONS = {
    "element": "FieldMatrix<Gf2mElement> over Gf2mField::gf256()",
    "wide": "FieldMatrix<Gf2mWide<1,Gf256x11d>>",
}


def case_for(declared):
    identity = declared["workload"]["identity"]
    if not identity.startswith(WORKLOAD_PREFIX):
        raise SystemExit(
            f"cell {declared['cell_id']}: this family measures field::matrix::gemm; the workload "
            f"identity is {identity!r}")
    return {
        "operation": "matmul",
        "bytes": 0,
        "n": declared["workload"]["size"]["n"],
        "k": 0,
        "rows": 0,
        "poly": POLY,
        "metric": declared["metric_kind"],
        "seed": declared["workload"]["seed"],
        "workers": declared["workers"]["declared"],
    }


if __name__ == "__main__":
    campaign_plan.main(LANES, REPRESENTATIONS, ENTRY_POINT, case_for)
