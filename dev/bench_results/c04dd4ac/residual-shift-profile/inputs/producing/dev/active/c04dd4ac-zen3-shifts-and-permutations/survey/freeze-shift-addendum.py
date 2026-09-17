#!/usr/bin/env python3
"""Generate or check the frozen protocol-v4 residual-shift addendum."""

import argparse
import json
from pathlib import Path

ISSUE = "85fc5ff4"
ACTIVE = Path("dev/active/c04dd4ac-zen3-shifts-and-permutations")
OUTPUT = ACTIVE / "shift-profile-addendum.json"
FROZEN_UTC = "2026-09-15T01:15:22Z"

# Each pair differs only in direction. The word-aligned path is deliberately a
# workload control, not a semantic replacement and not a production candidate.
WORKLOADS = [
    ("small", 65, 1, "warm", "single-core-latency"),
    ("lane-crossing", 257, 7, "warm", "single-core-latency"),
    ("byte-residual", 4097, 8, "warm", "single-core-latency"),
    ("resident", 65537, 63, "warm", "single-core-latency"),
    ("streaming", 33554439, 65, "streaming", "sustained-throughput"),
]


def cells():
    result = []
    seed = 8500
    for direction in ("left", "right"):
        for name, length, residual, cache, scaling in WORKLOADS:
            seed += 1
            result.append(
                {
                    "cell_id": f"{direction}-{name}-r{residual}-w64",
                    "objective": "comparator-gap",
                    "role": "exploratory",
                    "workload": {
                        "identity": (
                            f"bitvec-{direction}-{name}-len{length}-"
                            f"residual{residual}-word64-control"
                        ),
                        "size": {
                            "length_bits": length,
                            "residual_offset": residual,
                            "control_offset": 64,
                        },
                        "seed": seed,
                    },
                    "metric_kind": "kernel-isolated",
                    "scaling": scaling,
                    "core_arm": "single-core",
                    "workers": {"declared": 1, "nested_pools_allowed": False},
                    "cache_state": cache,
                    "builds": {
                        "baseline": "conservative-portable",
                        "candidate": "conservative-portable",
                    },
                    "conversion_costs_included": False,
                    "decoder": None,
                }
            )
    return result


def addendum():
    return {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": "bitvec-residual-shift-profile",
            "issue": ISSUE,
            "purpose": "kernel-family",
            "description": (
                "The public BitVec zero-fill left and right shift family as an isolated "
                "workload because the complete production-source sweep finds no downstream "
                "caller. Each exploratory cell times the current residual scalar-carry path "
                "against the existing word-aligned dispatch path at the same vector length, "
                "direction, seeded representation and cache state. The offsets intentionally "
                "differ: the word path is a non-equivalent workload control, so ratios describe "
                "residual-path cost and cannot establish semantic substitution or production "
                "adoption. The matrix covers small, lane-crossing, byte-residual, resident and "
                "streaming sizes; correctness is established separately against the same "
                "independent zero-fill oracle for both paths."
            ),
        },
        "frozen": {"frozen_utc": FROZEN_UTC},
        "effect": {
            "worthwhile_speedup": None,
            "rationale": (
                "No adoptable candidate exists in this profile. Any production speedup rule "
                "is fixed only after a material result triggers the bracket's Rust 1.95 "
                "feasibility and re-review barrier."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.03,
            "equivalence_rationale": (
                "Three percent separates descriptive parity from a visible workload-control "
                "difference; exploratory results do not turn this margin into an adoption test."
            ),
            "material_gap_threshold": 1.20,
            "material_gap_rationale": (
                "A residual path must consume at least one fifth more time than the word-aligned "
                "control at a consequential size before this no-consumer family can justify "
                "planning-time feasibility work for at most two concrete forms."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 0,
            "maintenance_rationale": (
                "This profile authorizes no production source or unsafe kernel. A material "
                "nomination requires an amended, re-reviewed bracket before another leaf can "
                "receive a non-zero implementation budget."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": str(ACTIVE / "shift-profile-trial-ledger.jsonl"),
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": cells(),
    }


def encoded():
    return json.dumps(addendum(), indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = encoded()
    if args.check:
        observed = OUTPUT.read_text()
        if observed != expected:
            raise SystemExit(f"{OUTPUT} differs from its frozen generator")
        print(f"{OUTPUT}: frozen matrix matches ({len(cells())} cells)")
    else:
        OUTPUT.write_text(expected)
        print(f"{OUTPUT}: froze {len(cells())} exploratory cells")


if __name__ == "__main__":
    main()
