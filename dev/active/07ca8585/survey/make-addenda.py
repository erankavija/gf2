#!/usr/bin/env python3
"""Write this issue's pilot family addenda (jit:07ca8585).

Every numeric setting a cell carries is either a workload identity read from
the committed input bundle manifest, or a decoder setting the `3be770d5`
matched families already froze for the same recorded frames. A pilot declares
no margin: the confirmation addenda are derived from the accepted pilot
receipts by the canonical freezer, not written here.

Usage: make-addenda.py --bundles-dir DIR --out-dir DIR [--frozen-utc STAMP]
"""

import argparse
import json
import pathlib
import datetime

CODES = {
    "dvb-t2-r12": {
        "bundle": "dvb-t2-r12-waterfall",
        "identity": "dvb-t2-normal-rate-1-2-etsi-en-302-755",
        "workload": "dvb-t2-normal-rate-1-2-etsi-en-302-755 awgn-bpsk recorded llrs",
        "llr_source": "dev/active/c077a88b/survey bundle dvb-t2-r12-waterfall (ldpc-make-inputs)",
    },
    "nr-bg1-z384": {
        "bundle": "nr-bg1-z384-mother",
        "identity": "nr-5g-bg1-lifting-384-mother-code-3gpp-ts-38-212",
        "workload": "nr-5g-bg1-lifting-384-mother-code-3gpp-ts-38-212 awgn-bpsk recorded llrs",
        "llr_source": "dev/active/c077a88b/survey bundle nr-bg1-z384-mother (ldpc-make-inputs)",
    },
}

TIMED_BATCH = 8
ITERATION_CAP = 50
NORMALIZATION_FACTOR = 0.75

CORE_ARMS = {
    "w1": ("single-core", 1, "sustained-throughput"),
    "p6": ("physical-cores-6", 6, "multicore-throughput"),
    "p12": ("physical-cores-12", 12, "multicore-throughput"),
    "l24": ("logical-cpus-24", 24, "multicore-throughput"),
}

OPERATION = (
    "Steady-state operation version 1, the operation `3be770d5` froze, unchanged: every worker is "
    "a thread pinned to one resolved CPU and owns one decoder built before timing over a clone of "
    "the recorded-AList code. AList parsing and construction are reported as setup, outside the "
    "timed windows. A timed call makes every worker decode the first 8 recorded frames of fixture "
    "bank zero, including conversion of the recorded f32 LLRs, the dispatch to the workers and "
    "extraction of the information-window decisions; per-worker work is identical, so a call "
    "measures saturation without a load-imbalance tail. Throughput is workers times 8 frames per "
    "call. The c077a88b prepared 128-frame quality evidence is reused without resampling, and "
    "every timed execution checks each worker's per-frame bit errors against it and fails on any "
    "difference. Arms report observed per-worker affinity, CPUs and process thread counts; no "
    "nested pool is declared."
)

BEFORE_AFTER = (
    "Before and after the 07ca8585 change to the CPU min-sum check-node update, on bit-identical "
    "parity-check matrices and recorded LLRs under one numerical contract: f32 flooding normalized "
    "min-sum at factor 0.75, iteration cap 50, syndrome stopping, one frame per decoder "
    "invocation. The baseline arm is the pinned pre-change implementation and the candidate arm is "
    "the changed one; both are the same harness, built from the two generations of gf2-coding, and "
    "each is pinned by the digest of its executable. "
)

COMPARATOR = (
    "The changed gf2 decoder against AFF3CT v4.7.0's flooding normalized min-sum, the matched "
    "comparison `3be770d5` measured before the change, on the same recorded frames under the same "
    "schedule, precision, normalization, iteration cap and stopping rule. This family publishes "
    "the REQ-10 comparison and selects nothing: adoption is decided by the before/after families, "
    "which keep their own ledgers. "
)

FAMILIES = {
    "ldpc-update-single-worker-v1": {
        "arms": ("w1",),
        "objective": "improvement",
        "purpose": "decoder-family",
        "ledger": "dev/bench_results/07ca8585/v4-update-single-worker-family-ledger.jsonl",
        "description": BEFORE_AFTER
        + "This family asks the per-core question: one worker, no sharing. "
        + OPERATION,
    },
    "ldpc-update-multicore-v1": {
        "arms": ("p6", "p12", "l24"),
        "objective": "improvement",
        "purpose": "decoder-family",
        "ledger": "dev/bench_results/07ca8585/v4-update-multicore-family-ledger.jsonl",
        "description": BEFORE_AFTER
        + "This family asks the saturation question: six and twelve physical cores and "
        "twenty-four logical CPUs. " + OPERATION,
    },
    "ldpc-update-comparator-single-worker-v1": {
        "arms": ("w1",),
        "objective": "comparator-gap",
        "purpose": "decoder-family",
        "ledger": "dev/bench_results/07ca8585/v4-update-comparator-single-worker-family-ledger.jsonl",
        "description": COMPARATOR + "One worker. " + OPERATION,
    },
    "ldpc-update-comparator-multicore-v1": {
        "arms": ("p6", "p12", "l24"),
        "objective": "comparator-gap",
        "purpose": "decoder-family",
        "ledger": "dev/bench_results/07ca8585/v4-update-comparator-multicore-family-ledger.jsonl",
        "description": COMPARATOR
        + "Six and twelve physical cores and twenty-four logical CPUs. "
        + OPERATION,
    },
}


def cell(family, code_key, arm_key, manifest):
    core_arm, workers, scaling = CORE_ARMS[arm_key]
    code = CODES[code_key]
    return {
        "cell_id": f"{code_key}-update-{arm_key}",
        "objective": FAMILIES[family]["objective"],
        "role": "exploratory",
        "workload": {
            "identity": code["workload"],
            "size": {
                "n": manifest["n"],
                "k": manifest["k"],
                "frames": manifest["frames"],
                "timed_batch": TIMED_BATCH,
                "workers": workers,
                "timed_frames": TIMED_BATCH * workers,
                "timed_fixture_banks": 1,
            },
            "seed": manifest["seed"],
        },
        "metric_kind": "whole-consumer",
        "scaling": scaling,
        "core_arm": core_arm,
        "workers": {"declared": workers, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {
            "baseline": "native",
            "candidate": "external" if "comparator" in family else "native",
        },
        "conversion_costs_included": True,
        "decoder": {
            "arm_kind": "matched-algorithm",
            "code": {
                "identity": code["identity"],
                "n": manifest["n"],
                "k": manifest["k"],
                "h_sha256": manifest["h_sha256"],
            },
            "input": {
                "llr_source": code["llr_source"],
                "llr_sha256": manifest["llrs_sha256"],
                "frames": manifest["frames"],
                "seed": manifest["seed"],
                "codeword_source": manifest["codeword_source"],
                "snr_db": manifest["esn0_db"],
            },
            "precision": "f32",
            "schedule": "flooding",
            "normalization": {
                "kind": "normalized-min-sum",
                "factor": NORMALIZATION_FACTOR,
            },
            "iteration_cap": ITERATION_CAP,
            "stopping": {"kind": "syndrome", "crc": None},
            "batching": {"batch_size": 1, "batch_fill_included": True},
            "quality_tolerance": {"fer_ratio_max": 1.0, "confidence": 0.95},
            "rate_matching": None,
        },
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bundles-dir", required=True)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--frozen-utc")
    args = parser.parse_args()

    bundles = pathlib.Path(args.bundles_dir)
    manifests = {
        key: json.loads((bundles / code["bundle"] / "manifest.json").read_text(encoding="utf-8"))
        for key, code in CODES.items()
    }
    frozen = args.frozen_utc or datetime.datetime.now(datetime.timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )

    out = pathlib.Path(args.out_dir)
    for family, spec in FAMILIES.items():
        cells = [
            cell(family, code_key, arm_key, manifests[code_key])
            for code_key in CODES
            for arm_key in spec["arms"]
        ]
        addendum = {
            "schema": "zen3-benchmark-addendum-v4",
            "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
            "family": {
                "id": family,
                "issue": "07ca8585",
                "purpose": spec["purpose"],
                "description": "Exploratory pilot. " + spec["description"]
                + " It observes the measurement resolution that this family's confirmatory "
                "addendum freezes; it decides nothing.",
            },
            "frozen": {"frozen_utc": frozen},
            "effect": {
                "worthwhile_speedup": None,
                "rationale": "A pilot observes the resolution that later fixes this family's "
                "margins; declaring them here would defeat the observation.",
                "measurement_resolution": None,
                "resolution_evidence": None,
                "equivalence_margin": None,
                "equivalence_rationale": "A pilot decides nothing, so it declares no equivalence "
                "margin.",
                "material_gap_threshold": None,
                "material_gap_rationale": "A confirmatory addendum fixes the material-gap "
                "threshold from this pilot's observed resolution.",
            },
            "complexity_budget": {
                "max_new_unsafe_kernels": 0,
                "max_added_source_lines": 900,
                "maintenance_rationale": "The change is one edge-layout type and one scalar "
                "reduction in the owning library layer, with their tests. It adds no unsafe "
                "kernel, so the budget for one is zero, and the source budget is the size of "
                "those two modules and the decoder rewrite they replace.",
            },
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": spec["ledger"],
            },
            "search_budget": {
                "max_pilot_trials_per_cell": 2,
                "max_confirmatory_attempts_per_candidate": 1,
            },
            "holdout": {"required": False, "cells": []},
            "cells": cells,
        }
        path = out / f"addendum-{family[: -len('-v1')]}-pilot.json"
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(addendum, handle, indent=2)
            handle.write("\n")
        print(path)


if __name__ == "__main__":
    main()
