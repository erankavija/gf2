#!/usr/bin/env python3
"""Emit the runner plan for one byte-field campaign (jit:6c6b09b1).

The plan names one arm per (library, arithmetic variant) pair and one cell
per entry in the frozen addendum. Cell cases are built from the addendum's
own workload sizes, so the plan cannot disagree with the frozen declaration
about what a cell measures.

An arm is a fresh process per execution; the runner digests each executable
at run time, so the environment below is the only thing that distinguishes
two arms sharing one binary.
"""

import argparse
import collections
import json


# The two full reduction polynomials this survey measures. 0x11D is the field
# gf2-core's `Gf2mField::gf256()` uses and the only one ISA-L implements, so it
# carries every cell with an ISA-L arm. 0x11B is the AES polynomial, a
# different field on the same byte carrier; M4RIE and GF-Complete take it as a
# parameter and ISA-L has no arm there at all.
POLY_11D = 0x11D
POLY_11B = 0x11B

# Each cell id encodes operation, comparator and regime; the table maps it to
# the arm, the case fields the two arms decode, and the field the cell works
# in. Nothing here may disagree with the frozen addendum: the workload sizes
# and seeds come from the addendum itself.
CELLS = {
    "axpy-isal-l1-1core": ("ext-isal", "axpy", "kernel-isolated", POLY_11D),
    "axpy-isal-l2-1core": ("ext-isal", "axpy", "kernel-isolated", POLY_11D),
    "axpy-isal-l3-1core": ("ext-isal", "axpy", "kernel-isolated", POLY_11D),
    "axpy-isal-streaming-1core": ("ext-isal", "axpy", "kernel-isolated", POLY_11D),
    "axpy-isal-whole-consumer-l2-1core": ("ext-isal", "axpy", "whole-consumer", POLY_11D),
    "axpy-gfcomplete-l2-1core": ("ext-gfcomplete", "axpy", "kernel-isolated", POLY_11D),
    "axpy-gfcomplete-11b-l2-1core": ("ext-gfcomplete", "axpy", "kernel-isolated", POLY_11B),
    "axpy-m4rie-l2-1core": ("ext-m4rie", "axpy", "kernel-isolated", POLY_11D),
    "matmul-m4rie-n64-1core": ("ext-m4rie", "matmul", "kernel-isolated", POLY_11D),
    "matmul-m4rie-n128-1core": ("ext-m4rie", "matmul", "kernel-isolated", POLY_11D),
    "matmul-m4rie-n256-1core": ("ext-m4rie", "matmul", "kernel-isolated", POLY_11D),
    "matmul-m4rie-n128-whole-consumer-1core": ("ext-m4rie", "matmul", "whole-consumer", POLY_11D),
    "encode-isal-k6r3-1core": ("ext-isal", "encode", "kernel-isolated", POLY_11D),
    "pairwise-gfcomplete-l2-1core": ("ext-gfcomplete", "pairwise", "kernel-isolated", POLY_11D),
}


def case_for(declared, operation, metric, poly):
    size = declared["workload"]["size"]
    return collections.OrderedDict(
        sorted(
            {
                "operation": operation,
                "bytes": size.get("bytes", 0),
                "n": size.get("n", 0),
                "k": size.get("k", 0),
                "rows": size.get("rows", 0),
                "poly": poly,
                "metric": metric,
                "seed": declared["workload"]["seed"],
                "workers": declared["workers"]["declared"],
            }.items()
        )
    )


def main():
    parser = argparse.ArgumentParser()
    for flag in (
        "plan",
        "campaign",
        "issue",
        "label",
        "mode",
        "addendum",
        "lock",
        "gf2-arm",
        "ext-arm",
        "rustflags",
    ):
        parser.add_argument(f"--{flag}", required=True)
    args = parser.parse_args()

    with open(args.addendum) as handle:
        addendum = json.load(handle)

    def external(backend, variant, description):
        return collections.OrderedDict(
            [
                ("build", "external"),
                ("description", description),
                ("executable", getattr(args, "ext_arm")),
                ("arguments", []),
                (
                    "environment",
                    collections.OrderedDict(
                        [
                            ("GF2_SURVEY_BACKEND", backend),
                            ("GF2_SURVEY_VARIANT", variant),
                        ]
                    ),
                ),
                ("rustflags", None),
                ("tuning_profile", None),
            ]
        )

    arms = collections.OrderedDict(
        [
            (
                "gf2",
                collections.OrderedDict(
                    [
                        ("build", "native"),
                        (
                            "description",
                            "gf2-core consumer entry points over GF(2^8), field per cell",
                        ),
                        ("executable", getattr(args, "gf2_arm")),
                        ("arguments", []),
                        ("environment", collections.OrderedDict()),
                        ("rustflags", args.rustflags),
                        ("tuning_profile", None),
                    ]
                ),
            ),
            (
                "ext-isal",
                external(
                    "isa-l",
                    "default",
                    "ISA-L gf_vect_mad and ec_encode_data, runtime-dispatched kernels",
                ),
            ),
            (
                "ext-gfcomplete",
                external(
                    "gf-complete",
                    "default",
                    "GF-Complete w=8 default region kernel and elementwise multiply",
                ),
            ),
            (
                "ext-m4rie",
                external(
                    "m4rie",
                    "default",
                    "M4RIE mzed_add_multiple_of_row and mzed_mul, Newton-John tables",
                ),
            ),
        ]
    )

    cells = []
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        if cell_id not in CELLS:
            raise SystemExit(f"cell {cell_id} has no arm mapping in make-plan.py")
        arm, operation, metric, poly = CELLS[cell_id]
        entry = collections.OrderedDict(
            [
                ("cell_id", cell_id),
                ("baseline_arm", "gf2"),
                ("candidate_arm", arm),
                ("case", case_for(declared, operation, metric, poly)),
                ("pilot_pairs", 6 if declared["role"] == "exploratory" else None),
            ]
        )
        cells.append(entry)

    # Only the arms the cells actually name may appear, so the receipt's arm
    # descriptors describe exactly what ran.
    used = {"gf2"} | {cell["candidate_arm"] for cell in cells}
    arms = collections.OrderedDict(
        (name, arm) for name, arm in arms.items() if name in used
    )

    plan = collections.OrderedDict(
        [
            ("schema", "zen3-benchmark-plan-v1"),
            ("campaign_id", args.campaign),
            ("issue", args.issue),
            ("label", args.label),
            ("campaign_seed", 20260907),
            ("addendum", args.addendum),
            ("lock_path", args.lock),
            ("wrapper", "dev/scripts/ccx1-bench-flock.sh"),
            ("timing_override", None),
            ("arms", arms),
            ("cells", cells),
            # Eight workers share this host and the exclusive mutex, so a
            # session measures a few cells and pauses, and the next session
            # resumes from the checkpoint store. The chunk size answers the
            # cost that dominates on this host: a measured pilot cell takes
            # about eight seconds while one mutex acquisition waited about ten
            # minutes, so many small sessions cost the queue far more than they
            # save the host. The three-cell pilot pauses after two; the
            # fourteen-cell confirmation pauses after seven, a hold of roughly
            # a minute across two acquisitions.
            ("max_cells_per_session", 2 if args.mode == "pilot" else 7),
        ]
    )
    with open(args.plan, "w") as out:
        json.dump(plan, out, indent=2)
        out.write("\n")


if __name__ == "__main__":
    main()
