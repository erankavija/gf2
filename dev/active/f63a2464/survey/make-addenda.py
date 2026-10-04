#!/usr/bin/env python3
"""Emit this issue's exploratory pilot addenda (jit:f63a2464).

One addendum per family, each declaring only what a pilot declares: its cells,
its ledger and its search budget. A pilot observes the measurement resolution a
confirmatory addendum is later frozen from by the canonical freezer, so every
margin field is null here and the rationale says why.

The shared numeric settings come from the protocol, never from this file. The
code identity, input identity and decoder contract of every cell come from the
recorded bundle's own manifest, so a changed bundle cannot silently keep the
frozen text.

Usage (from the worktree root):
  make-addenda.py --frozen-utc YYYY-MM-DDTHH:MM:SSZ --bundles-dir DIR
      --results-dir DIR

The results directory is the root-relative directory of the family ledgers.
"""

import argparse
import json
import pathlib
import subprocess
import sys

SURVEY = pathlib.Path(__file__).resolve().parent
CATALOGUE = json.loads((SURVEY / "arms.json").read_text(encoding="utf-8"))
ROOT = pathlib.Path(subprocess.run(
    ["git", "-C", str(SURVEY), "rev-parse", "--show-toplevel"],
    check=True, capture_output=True, text=True,
).stdout.strip())

ISSUE = "f63a2464"
ITERATION_CAP = 50
NORMALIZATION_FACTOR = 0.75
TIMED_BATCH = 8
CODE_IDENTITY = "nr-5g-bg1-lifting-384-mother-code-3gpp-ts-38-212"
WORKLOAD_IDENTITY = f"{CODE_IDENTITY} awgn-bpsk recorded llrs"

COMMON_DESCRIPTION = (
    "The steady-state operation of `3be770d5`, unchanged: every worker is a thread pinned to "
    "one resolved CPU and owns one decoder built before timing. AList parsing and construction "
    "are reported as setup, outside the timed windows. A timed call makes every worker decode "
    "its declared batch of recorded frames of fixture bank zero, including conversion of the "
    "recorded f32 LLRs, the dispatch to the workers and extraction of the information-window "
    "decisions. The candidate is the QC-aware intra-frame prototype of `f63a2464`, which "
    "declares the canonical numerical contract unchanged: f32 flooding normalized min-sum at "
    "factor 0.75, iteration cap 50, syndrome stopping, one frame per decoder invocation, "
    "vectorized across the lifted positions of one frame's circulant blocks rather than across "
    "frames, so it fills no batch. The `c077a88b` prepared 128-frame quality evidence is reused "
    "without resampling and every timed execution checks each worker's per-frame bit errors "
    "against it, so a candidate that parted from the contract fails the run. The DVB-T2 "
    "workload declares no cell in this family: its check rows carry no equal-degree circulant "
    "block partition, which `f63a2464`'s numerical-contract review records with its evidence. "
    "Arms report observed per-worker affinity, CPUs and process thread counts; no nested pool is "
    "declared. It observes the measurement resolution that this family's confirmatory addendum "
    "freezes; it decides nothing."
)

FAMILIES = {
    "ldpc-qc-intra-frame-single-worker-v1": {
        "purpose": "decoder-family",
        "description": (
            "Exploratory pilot. The canonical gf2 decoder against the QC-aware intra-frame "
            "candidate on the NR BG1 lifting-384 mother code, on a bit-identical parity-check "
            "matrix and bit-identical recorded LLRs under one numerical contract. This family "
            "asks the per-core question: one worker, no sharing, at two granularities — "
            "sustained throughput over a batch of frames and the latency of a single frame, "
            "which is the axis inter-frame batching cannot improve. " + COMMON_DESCRIPTION
        ),
        "ledger": "v4-qc-intra-frame-single-worker-family-ledger.jsonl",
        "arm_kind": "matched-algorithm",
        "builds": {"baseline": "native", "candidate": "native"},
        "cells": [
            ("nr-bg1-z384-qc-w1", "improvement", "sustained-throughput", "single-core", 1, TIMED_BATCH),
            ("nr-bg1-z384-qc-latency", "improvement", "single-core-latency", "single-core", 1, 1),
        ],
    },
    "ldpc-qc-intra-frame-multicore-v1": {
        "purpose": "decoder-family",
        "description": (
            "Exploratory pilot. The same two arms decoding independent frames on several "
            "workers, which characterizes how the candidate shares the memory system rather "
            "than deciding adoption. " + COMMON_DESCRIPTION
        ),
        "ledger": "v4-qc-intra-frame-multicore-family-ledger.jsonl",
        "arm_kind": "matched-algorithm",
        "builds": {"baseline": "native", "candidate": "native"},
        "cells": [
            ("nr-bg1-z384-qc-w6", "improvement", "multicore-throughput", "physical-cores-6", 6, TIMED_BATCH),
            ("nr-bg1-z384-qc-w12", "improvement", "multicore-throughput", "physical-cores-12", 12, TIMED_BATCH),
            ("nr-bg1-z384-qc-w24", "improvement", "multicore-throughput", "logical-cpus-24", 24, TIMED_BATCH),
        ],
    },
    "ldpc-qc-comparator-single-worker-v1": {
        "purpose": "decoder-family",
        "description": (
            "Exploratory pilot. The QC-aware intra-frame candidate against the matched external "
            "arm, AFF3CT v4.7.0's scalar f32 flooding normalized-min-sum decoder [Cassagne2019], "
            "under the same contract on the same recorded frames. It estimates a comparator gap "
            "and selects nothing. The fastest quality-compatible external arm is a separate "
            "question whose shortlist `c077a88b` closed empty: no surveyed candidate satisfied "
            "the paired quality-admission rule on this corpus, and that outcome is preserved "
            "rather than replaced by a matched arm. " + COMMON_DESCRIPTION
        ),
        "ledger": "v4-qc-comparator-single-worker-family-ledger.jsonl",
        "arm_kind": "matched-algorithm",
        "builds": {"baseline": "native", "candidate": "external"},
        "cells": [
            ("nr-bg1-z384-qc-comparator-w1", "comparator-gap", "sustained-throughput", "single-core", 1, TIMED_BATCH),
            ("nr-bg1-z384-qc-comparator-latency", "comparator-gap", "single-core-latency", "single-core", 1, 1),
        ],
    },
}


def cell(manifest, arm_kind, builds, spec):
    cell_id, objective, scaling, core_arm, workers, batch = spec
    return {
        "cell_id": cell_id,
        "objective": objective,
        "role": "exploratory",
        "workload": {
            "identity": WORKLOAD_IDENTITY,
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
        "builds": dict(builds),
        "conversion_costs_included": True,
        "decoder": {
            "arm_kind": arm_kind,
            "code": {
                "identity": CODE_IDENTITY,
                "n": manifest["n"],
                "k": manifest["k"],
                "h_sha256": manifest["h_sha256"],
            },
            "input": {
                "llr_source": "dev/active/c077a88b/survey bundle nr-bg1-z384-mother (ldpc-make-inputs)",
                "llr_sha256": manifest["llrs_sha256"],
                "frames": manifest["frames"],
                "seed": manifest["seed"],
                "codeword_source": manifest["codeword_source"],
                "snr_db": manifest["esn0_db"],
            },
            "precision": "f32",
            "schedule": "flooding",
            "normalization": {"kind": "normalized-min-sum", "factor": NORMALIZATION_FACTOR},
            "iteration_cap": ITERATION_CAP,
            "stopping": {"kind": "syndrome", "crc": None},
            "batching": {"batch_size": 1, "batch_fill_included": True},
            "quality_tolerance": {"fer_ratio_max": 1.0, "confidence": 0.95},
            "rate_matching": None,
        },
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--frozen-utc", required=True)
    parser.add_argument("--bundles-dir", default="target/ldpc-inputs")
    parser.add_argument("--results-dir", required=True, type=pathlib.Path)
    args = parser.parse_args()

    bundle = pathlib.Path(args.bundles_dir) / CATALOGUE["codes"]["nr-bg1-z384"]["bundle"]
    manifest = json.loads((bundle / "manifest.json").read_text(encoding="utf-8"))

    for family, declared in FAMILIES.items():
        if family not in CATALOGUE["families"]:
            sys.exit(f"{family} is not in the arms catalogue")
        addendum = {
            "schema": "zen3-benchmark-addendum-v4",
            "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
            "family": {
                "id": family,
                "issue": ISSUE,
                "purpose": declared["purpose"],
                "description": declared["description"],
            },
            "frozen": {"frozen_utc": args.frozen_utc},
            "effect": {
                "worthwhile_speedup": None,
                "rationale": (
                    "A pilot observes the resolution that later fixes this family's margins; "
                    "declaring them here would defeat the observation."
                ),
                "measurement_resolution": None,
                "resolution_evidence": None,
                "equivalence_margin": None,
                "equivalence_rationale": "A pilot decides nothing, so it declares no equivalence margin.",
                "material_gap_threshold": None,
                "material_gap_rationale": (
                    "A confirmatory addendum fixes the material-gap threshold from this pilot's "
                    "observed resolution."
                ),
            },
            "complexity_budget": {
                "max_new_unsafe_kernels": 1,
                "max_added_source_lines": 1200,
                "maintenance_rationale": (
                    "The candidate is one block layout, one flooding decoder over it and one "
                    "AVX2 check kernel with its scalar reference, which is the single unsafe "
                    "kernel the budget allows. The source budget is the size of those modules "
                    "and their behavioural suite. Nothing in the production decoder changes "
                    "under this issue."
                ),
            },
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": str(args.results_dir / declared["ledger"]),
            },
            "search_budget": {
                "max_pilot_trials_per_cell": 2,
                "max_confirmatory_attempts_per_candidate": 1,
            },
            "holdout": {"required": False, "cells": []},
            "cells": [
                cell(manifest, declared["arm_kind"], declared["builds"], spec)
                for spec in declared["cells"]
            ],
        }
        out = SURVEY.parent.relative_to(ROOT) / f"addendum-{family[:-3]}-pilot.json"
        out.write_text(json.dumps(addendum, indent=2) + "\n", encoding="utf-8")
        print(out.as_posix())


if __name__ == "__main__":
    main()
