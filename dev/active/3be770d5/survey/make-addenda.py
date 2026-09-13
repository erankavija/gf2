#!/usr/bin/env python3
"""Write the three protocol-v3 pilot addenda of the steady-state survey (jit:3be770d5).

Each family freezes one question. The cell list, workload sizes, decoder
contract and quality tolerance come from the constants below; the code and
input identities come from the recorded bundle manifests, which the arms
verify by digest at run time. Refuses to overwrite an existing addendum: a
published pilot addendum is frozen.

Usage: make-addenda.py --inputs DIR
"""

import argparse
import datetime
import json
import pathlib

ISSUE = "3be770d5"
ACTIVE = pathlib.Path("dev/active") / ISSUE
RESULTS = pathlib.Path("dev/bench_results") / ISSUE
CATALOGUE = json.loads((pathlib.Path(__file__).resolve().parent / "arms.json").read_text())

CODES = {
    "dvb-t2-r12": {
        "identity": "dvb-t2-normal-rate-1-2-etsi-en-302-755",
        "workload": "dvb-t2-normal-rate-1-2-etsi-en-302-755 awgn-bpsk recorded llrs",
    },
    "nr-bg1-z384": {
        "identity": "nr-5g-bg1-lifting-384-mother-code-3gpp-ts-38-212",
        "workload": "nr-5g-bg1-lifting-384-mother-code-3gpp-ts-38-212 awgn-bpsk recorded llrs",
    },
}

# Frames each worker decodes per timed call. The matched families use the
# first eight recorded frames (both codeword classes); the fastest-compatible
# family needs whole sixteen-frame i16 INTER waves.
MATCHED_BATCH = 8
FASTEST_BATCH = 16

CORE_ARMS = {
    "w1": ("single-core", 1, "sustained-throughput"),
    "p6": ("physical-cores-6", 6, "multicore-throughput"),
    "p12": ("physical-cores-12", 12, "multicore-throughput"),
    "l24": ("logical-cpus-24", 24, "multicore-throughput"),
}

OPERATION = (
    "Steady-state operation version 1: every worker is a thread pinned to one resolved CPU and "
    "owns one decoder built before timing (gf2: a decoder over a clone of the recorded-AList "
    "code; AFF3CT: its own clone() of one decoder built from the same AList through the "
    "c077a88b shim). AList parsing and construction are reported as setup, outside the timed "
    "windows. A timed call makes every worker decode the first {batch} recorded frames of "
    "fixture bank zero, including conversion of the recorded f32 LLRs, the dispatch to the "
    "workers and extraction of the information-window decisions; per-worker work is identical, "
    "so a call measures saturation without a load-imbalance tail. Throughput is workers times "
    "{batch} frames per call. The c077a88b prepared 128-frame quality evidence is reused without "
    "resampling, and every timed execution checks each worker's per-frame bit errors against it "
    "and fails on any difference. Arms report observed per-worker affinity, CPUs and process "
    "thread counts; no nested pool is declared."
)

FAMILIES = {
    "ldpc-steady-matched-single-worker-v1": {
        "ledger": "v3-steady-matched-single-worker-family-ledger.jsonl",
        "arm_kind": "matched-algorithm",
        "arms": ["w1"],
        "candidates": ["aff3ct-flooding-nms-f32"],
        "batch": MATCHED_BATCH,
        "tolerance": 1.0,
        "question": (
            "Exploratory pilot of the per-core steady-state gap: gf2-coding against AFF3CT "
            "v4.7.0, both f32 flooding normalized min-sum at factor 0.75, iteration cap 50 and "
            "syndrome stopping, on bit-identical parity-check matrices and recorded LLRs, one "
            "worker each. It observes the measurement resolution that the confirmatory addendum "
            "of this family freezes; it decides nothing."
        ),
    },
    "ldpc-steady-matched-multicore-v1": {
        "ledger": "v3-steady-matched-multicore-family-ledger.jsonl",
        "arm_kind": "matched-algorithm",
        "arms": ["p6", "p12", "l24"],
        "candidates": ["aff3ct-flooding-nms-f32"],
        "batch": MATCHED_BATCH,
        "tolerance": 1.0,
        "question": (
            "Exploratory pilot of the steady-state gap under multicore saturation: the matched "
            "gf2-coding and AFF3CT v4.7.0 f32 flooding normalized-min-sum decoders (factor 0.75, "
            "cap 50, syndrome stopping) with six and twelve physical-core workers and 24 "
            "logical-CPU workers, one worker per resolved CPU. It observes the measurement "
            "resolution that the confirmatory addendum of this family freezes; it decides nothing."
        ),
    },
    "ldpc-steady-fastest-compatible-v1": {
        "ledger": "v3-steady-fastest-compatible-family-ledger.jsonl",
        "arm_kind": "fastest-quality-compatible",
        "arms": ["w1"],
        "candidates": [
            "aff3ct-layered-nms-f32",
            "aff3ct-layered-nms-f32-inter",
            "aff3ct-layered-nms-i16-inter",
        ],
        "batch": FASTEST_BATCH,
        "tolerance": 2.0,
        "question": (
            "Exploratory pilot of the steady-state gap to the fastest supported AFF3CT v4.7.0 "
            "modes: the gf2-coding f32 flooding normalized-min-sum decoder against AFF3CT "
            "horizontal-layered NMS f32 scalar, f32 INTER (eight-frame waves) and i16 INTER "
            "(sixteen-frame waves, LLR scale 16), one worker each, on bit-identical matrices "
            "and recorded LLRs. Precision, schedule and wave size differ and stay labelled. "
            "Under v3 the 128-frame corpus cannot certify quality admission, so this family "
            "plans no confirmation; its cells characterize the layered, quantized and "
            "inter-frame levers and select nothing."
        ),
    },
}


def cell(code_key, manifest, arm_key, candidate, spec):
    core_arm, workers, scaling = CORE_ARMS[arm_key]
    batch = spec["batch"]
    suffix = candidate.removeprefix("aff3ct-").removeprefix("layered-nms-")
    name = "matched" if spec["arm_kind"] == "matched-algorithm" else f"layered-{suffix}"
    return {
        "cell_id": f"{code_key}-{name}-{arm_key}",
        "objective": "comparator-gap",
        "role": "exploratory",
        "workload": {
            "identity": CODES[code_key]["workload"],
            "size": {
                "n": manifest["n"],
                "k": manifest["k"],
                "frames": manifest["frames"],
                "timed_batch": batch,
                "workers": workers,
                "timed_frames": batch * workers,
                "timed_fixture_banks": 1,
            },
            "seed": manifest["seed"],
        },
        "metric_kind": "whole-consumer",
        "scaling": scaling,
        "core_arm": core_arm,
        "workers": {"declared": workers, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "native", "candidate": "external"},
        "conversion_costs_included": True,
        "decoder": {
            "arm_kind": spec["arm_kind"],
            "code": {
                "identity": CODES[code_key]["identity"],
                "n": manifest["n"],
                "k": manifest["k"],
                "h_sha256": manifest["h_sha256"],
            },
            "input": {
                "llr_source": f"dev/active/c077a88b/survey bundle {CATALOGUE['codes'][code_key]['bundle']} (ldpc-make-inputs)",
                "llr_sha256": manifest["llrs_sha256"],
                "frames": manifest["frames"],
                "seed": manifest["seed"],
                "codeword_source": manifest["codeword_source"],
                "snr_db": manifest["esn0_db"],
            },
            "precision": "f32",
            "schedule": "flooding",
            "normalization": {"kind": "normalized-min-sum", "factor": 0.75},
            "iteration_cap": 50,
            "stopping": {"kind": "syndrome", "crc": None},
            "batching": {"batch_size": 1, "batch_fill_included": True},
            "quality_tolerance": {"fer_ratio_max": spec["tolerance"], "confidence": 0.95},
            "rate_matching": None,
        },
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--inputs", required=True, type=pathlib.Path)
    args = parser.parse_args()
    frozen = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    for family, spec in FAMILIES.items():
        cells = []
        for code_key, code in CATALOGUE["codes"].items():
            manifest = json.loads((args.inputs / code["bundle"] / "manifest.json").read_text())
            for arm_key in spec["arms"]:
                for candidate in spec["candidates"]:
                    cells.append(cell(code_key, manifest, arm_key, candidate, spec))
        addendum = {
            "schema": "zen3-benchmark-addendum-v3",
            "protocol": {"id": "zen3-benchmark-protocol", "version": 3},
            "family": {
                "id": family,
                "issue": ISSUE,
                "purpose": "decoder-family",
                "description": spec["question"] + " " + OPERATION.format(batch=spec["batch"]),
            },
            "frozen": {"frozen_utc": frozen},
            "effect": {
                "worthwhile_speedup": None,
                "rationale": "A pilot observes the resolution that later fixes this family's margins; declaring them here would defeat the observation.",
                "measurement_resolution": None,
                "resolution_evidence": None,
                "equivalence_margin": None,
                "equivalence_rationale": "A pilot decides nothing, so it declares no equivalence margin.",
                "material_gap_threshold": None,
                "material_gap_rationale": "A confirmatory addendum fixes the material-gap threshold from this pilot's observed resolution.",
            },
            "complexity_budget": {
                "max_new_unsafe_kernels": 0,
                "max_added_source_lines": None,
                "maintenance_rationale": "A profiling survey adopts nothing into production, so it budgets no new unsafe kernel and no production source.",
            },
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": str(RESULTS / spec["ledger"]),
            },
            "search_budget": {"max_pilot_trials_per_cell": 2, "max_confirmatory_attempts_per_candidate": 1},
            "holdout": {"required": False, "cells": []},
            "cells": cells,
        }
        path = ACTIVE / f"addendum-{family.removesuffix('-v1')}-pilot.json"
        with path.open("x", encoding="utf-8") as output:
            json.dump(addendum, output, indent=2)
            output.write("\n")
        ledger = RESULTS / spec["ledger"]
        ledger.open("xb").close()
        print(path, ledger)


if __name__ == "__main__":
    main()
