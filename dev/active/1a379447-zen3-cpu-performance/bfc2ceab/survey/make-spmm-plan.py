#!/usr/bin/env python3
"""Project the runner plan of one sparse product check-cost campaign (jit:bfc2ceab).

Usage: make-spmm-plan.py --addendum A --label L --campaign-id ID --campaign-seed S
                         --lock PATH --executable BIN --producing-manifest M
                         --max-cells-per-session N [--pilot-pairs P] --output PLAN

The projection itself is the shared `campaign_plan.py`, which `locate.py`
beside this file puts on the import path; this file declares what is this
family's own: its two arms, which are one executable under two environments,
and the case a cell decodes.
"""

import locate  # noqa: F401
import campaign_plan  # noqa: E402

FORM_VAR = "GF2_SPMM_FORM"
CONSUMER = "sparse-matmat"


def arms_for(declared, executable):
    if declared["metric_kind"] != "whole-consumer":
        raise SystemExit(f"cell {declared['cell_id']}: every cell is whole-consumer")
    build = campaign_plan.BUILD
    return [
        ("unchecked-rows", campaign_plan.arm(
            build,
            f"spmm-check-arm, {build} release build: the dispatcher's row loop over the "
            "kernel crate's unchecked row kernel, with its lane conversions",
            executable,
            {FORM_VAR: "unchecked"},
        )),
        ("checked-product", campaign_plan.arm(
            build,
            f"spmm-check-arm, {build} release build: `SparseFieldMatrix::matmat`, whose "
            "kernel entry checks the column indices once per product",
            executable,
            {FORM_VAR: "checked"},
        )),
    ]


def case_for(declared):
    identity = declared["workload"]["identity"]
    if not identity.startswith(CONSUMER + "-"):
        raise SystemExit(f"cell {declared['cell_id']}: workload {identity!r} is no sparse product")
    return {"consumer": CONSUMER} | declared["workload"]["size"] | {
        "seed": declared["workload"]["seed"]}


if __name__ == "__main__":
    campaign_plan.project(arms_for, case_for)
