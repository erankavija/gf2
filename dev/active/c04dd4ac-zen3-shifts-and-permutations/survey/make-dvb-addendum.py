#!/usr/bin/env python3
"""Freeze the protocol-v4 DVB-T2 interleaver profile addendum (jit:9fb40c83).

Usage: make-dvb-addendum.py FROZEN_UTC [OUTPUT]

The matrix is declared here once and projected into the campaign plan by
`make-dvb-plan.py`. Every cell is exploratory and decides no adoption.
"""

import json
import pathlib
import re
import sys

ISSUE = "9fb40c83"
FAMILY = "dvb-t2-interleave-consumer-profile"
DEFAULT_OUTPUT = pathlib.Path(
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/dvb-profile-addendum.json"
)
LEDGER = (
    "dev/active/c04dd4ac-zen3-shifts-and-permutations/"
    "dvb-profile-trial-ledger.jsonl"
)

MODCODS = [
    ("qam16-r12-normal", 64_800, 32_400, 90, 8, 8_100, 2101),
    ("qam64-r12-normal", 64_800, 32_400, 90, 12, 5_400, 2102),
    ("qam16-r12-short", 16_200, 7_200, 25, 8, 2_025, 2103),
    ("qam64-r12-short", 16_200, 7_200, 25, 12, 1_350, 2104),
]


def cell(modcod, n, k, q, nc, nr, seed, boundary, cache):
    isolated = boundary == "isolated"
    suffix = "isolated-null" if isolated else "sim-stage-gap"
    return {
        "cell_id": f"dvb-t2-{modcod}-{cache}-{suffix}",
        "objective": "comparator-gap",
        "role": "exploratory",
        "workload": {
            "identity": f"dvb-t2-bit-interleave-{modcod}-{boundary}",
            "size": {
                "n_ldpc": n,
                "k_ldpc": k,
                "q_ldpc": q,
                "nc": nc,
                "nr": nr,
                "frames": 1,
            },
            "seed": seed,
        },
        "metric_kind": "kernel-isolated" if isolated else "whole-consumer",
        "scaling": "sustained-throughput" if cache == "streaming" else "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache,
        "builds": {
            "baseline": "native",
            "candidate": "native" if isolated else "external",
        },
        "conversion_costs_included": not isolated,
        "decoder": None,
    }


def main():
    if len(sys.argv) not in (2, 3):
        raise SystemExit(__doc__.strip())
    frozen = sys.argv[1]
    if not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", frozen):
        raise SystemExit("FROZEN_UTC must be a whole-second UTC timestamp")
    output = pathlib.Path(sys.argv[2]) if len(sys.argv) == 3 else DEFAULT_OUTPUT
    cells = []
    for row in MODCODS:
        cells.append(cell(*row, "isolated", "warm"))
        cells.append(cell(*row, "sim-stage", "warm"))
    # One repeated Normal-frame path rotates through the canonical eight fixture
    # banks, so the campaign observes sustained rather than cache-resident work.
    cells.append(cell(*MODCODS[0], "isolated", "streaming"))
    cells.append(cell(*MODCODS[0], "sim-stage", "streaming"))

    document = {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": FAMILY,
            "issue": ISSUE,
            "purpose": "consumer-family",
            "description": (
                "Exploratory attribution of the production DVB-T2 ETSI EN 302 755 v1.4.1 "
                "section 6.1.3 packed-bit scatter [Etsi2015] inside its real gf2 consumers. "
                "The isolated null cells launch the same current DvbT2BitInterleaver::interleave "
                "path as both arms; they estimate its absolute cost and same-binary noise without "
                "selecting a candidate. The whole-consumer cells start and end at the one-frame "
                "gf2-sim BitPackedBatch boundary: the baseline is BitInterleave::process and the "
                "candidate is xdsopl/LDPC PCTITL at commit "
                "32357d8ad55a6a302c34e093759f0454e45cca56 [Xdsopl2026], with unpack, "
                "the destructive-input copy, output allocation and pack inside every timed call. "
                "A speedup above one in those cells means xdsopl is ahead. The matrix crosses "
                "Normal and Short FECFRAMEs with 16-QAM and 64-QAM under warm cache state; the "
                "Normal 16-QAM path is repeated while rotating eight fixture banks for streaming "
                "coverage. All cells are single-core because the production interleaver and stage "
                "are serial. Seeds expand with the protocol's pinned SplitMix64 implementation. "
                "The accepted protocol-v3 re-measurement remains exploratory historical context; "
                "earlier warm receipts remain withdrawn and confirm no gap here. This family asks "
                "the distinct attribution question, adopts nothing and opens no confirmation. "
                "At most two branch-free or packed-word forms may be nominated from the combined "
                "receipt and dynamic profile; nomination does not authorize implementation."
            ),
        },
        "frozen": {"frozen_utc": frozen},
        "effect": {
            "worthwhile_speedup": None,
            "rationale": (
                "Every cell is a comparator-gap or same-path null cell and this issue adopts no "
                "implementation, so no worthwhile-improvement threshold applies."
            ),
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.10,
            "equivalence_rationale": (
                "Ten percent bounds descriptive parity for the null control and external gap; "
                "exploratory cells record it but never pass an adoption rule."
            ),
            "material_gap_threshold": 1.20,
            "material_gap_rationale": (
                "A twenty-percent whole-stage gap is the smallest residual worth a bounded "
                "branch-free or packed-word feasibility investigation after conversion, copy and "
                "stage-wrapper costs are attributed. Smaller gaps retain the current scatter."
            ),
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 1,
            "max_added_source_lines": 400,
            "maintenance_rationale": (
                "A nomination is bounded to no more than two concrete forms and four hundred "
                "production lines including shared conformance tests and documentation. At most "
                "one form may require an isolated SIMD kernel; every form still requires the "
                "planning-time Rust 1.95 compile, assembly, oracle and runtime-gated scalar "
                "fallback record before the bracket can authorize implementation."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": LEDGER,
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": cells,
    }
    output.write_text(json.dumps(document, indent=2) + "\n")
    print(f"{output}: {len(cells)} exploratory cells")


if __name__ == "__main__":
    main()
