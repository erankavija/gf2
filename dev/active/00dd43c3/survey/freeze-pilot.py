#!/usr/bin/env python3
"""Freeze the residual BMI2 pilot from the accepted workload profile."""

import argparse
import copy
import hashlib
import json
from pathlib import Path

ACTIVE = Path("dev/active/00dd43c3")
PROFILE = Path("dev/bench_results/c04dd4ac/residual-shift-profile")
PROFILE_RECEIPT = PROFILE / "receipt.json"
PROFILE_SUMMARY = PROFILE / "acceptance-summary.json"
PROFILE_ADDENDUM = PROFILE / "inputs/family-addendum.json"
FROZEN_UTC = "2026-09-26T07:29:22Z"


def encoded(value):
    return json.dumps(value, indent=2) + "\n"


def outputs():
    receipt_bytes = PROFILE_RECEIPT.read_bytes()
    summary = json.loads(PROFILE_SUMMARY.read_text())
    profile = json.loads(PROFILE_ADDENDUM.read_text())
    digest = hashlib.sha256(receipt_bytes).hexdigest()
    if summary["verdict"] != "accepted" or summary["receipt_sha256"] != digest:
        raise SystemExit("the workload profile is not an accepted pinned receipt")
    material = {cell["cell_id"] for cell in summary["cells"] if cell["decision"] == "improved"}
    cells = [copy.deepcopy(cell) for cell in profile["cells"] if cell["cell_id"] in material]
    if len(cells) != len(material) or {cell["cell_id"].split("-", 1)[0] for cell in cells} != {
        "left", "right"
    }:
        raise SystemExit("material profile cells do not cover both directions")
    for cell in cells:
        cell["objective"] = "improvement"
        cell["role"] = "exploratory"
        cell["workload"]["identity"] = cell["workload"]["identity"].replace(
            "word64-control", "scalar-vs-bmi2"
        )

    context = {
        "profile_receipt": {"path": str(PROFILE_RECEIPT), "sha256": digest},
        "profile_summary": {
            "path": str(PROFILE_SUMMARY),
            "sha256": hashlib.sha256(PROFILE_SUMMARY.read_bytes()).hexdigest(),
        },
        "material_cells": [cell["cell_id"] for cell in cells],
    }
    pilot = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": "bitvec-residual-bmi2-route",
            "issue": "00dd43c3",
            "purpose": "kernel-family",
            "description": (
                "Exploratory pilot of the shipped residual BitVec shift route. The baseline "
                "holds the public shift on its scalar funnel through the test-build switch; "
                "the candidate releases the switch for the BMI2-gated route. Both arms run "
                "the same executable, residual offset, seeded bits and cache state, and "
                "report their executed route. The profile receipt at "
                f"{PROFILE_RECEIPT} (SHA-256 {digest}) is context for selecting the material "
                "cells in both directions, not evidence inherited by this family. The public "
                "primitive has no downstream production caller, so cells measure isolated "
                "latency or throughput. Confirmation retains the largest set its ledger's "
                "P-20 tail-support budget admits, in this priority order: left and right "
                "resident, left and right streaming, left and right byte-residual, then "
                "left lane-crossing. The lane-crossing cell is the smallest material "
                "boundary; paired consequential sizes take priority. Frozen retention "
                "rule: retain the gated "
                "route only if acceptance verifies the confirmation, no confirmatory cell "
                "records fail or not-confirmatory, and at least one cell in each direction "
                "records pass. Otherwise remove it and keep the scalar funnel. Every "
                "not-material, regressed or inconclusive result remains published."
            ),
        },
        "frozen": {"frozen_utc": FROZEN_UTC},
        "effect": {
            "worthwhile_speedup": 1.20,
            "rationale": (
                "With no downstream production caller, a permanent BMI2 route must show "
                "a visible gain at the public primitive: twenty percent is the minimum "
                "return judged to repay the added dispatch and kernel surface."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.10,
            "equivalence_rationale": (
                "A change within ten percent is treated as equivalent at this isolated "
                "primitive; the confirmation freezer must put the margin above the pilot's "
                "observed resolution."
            ),
            "material_gap_threshold": None,
            "material_gap_rationale": (
                "Both arms run the same residual operation in the same executable, so "
                "this family has no workload-control gap to classify."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 1,
            "max_added_source_lines": 700,
            "maintenance_rationale": (
                "The one isolated BMI2 funnel kernel and its detected dispatch are the "
                "candidate surface. The ceiling covers that surface and its safety "
                "contracts; a wider vector route would require another proposal."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": "dev/bench_results/00dd43c3/residual-shift-family-ledger.jsonl",
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": cells,
    }
    return {
        ACTIVE / "pilot-addendum.json": encoded(pilot),
        ACTIVE / "profile-context.json": encoded(context),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for path, expected in outputs().items():
        if args.check:
            if path.read_text() != expected:
                raise SystemExit(f"{path} differs from its pinned profile projection")
        else:
            path.write_text(expected)
        print(path)


if __name__ == "__main__":
    main()
