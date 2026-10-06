#!/usr/bin/env python3
"""Write this issue's pilot family addenda (jit:07ca8585).

Every numeric setting a cell carries is either a workload identity read from
the committed input bundle manifest, or a decoder setting the `3be770d5`
matched families already froze for the same recorded frames. A pilot declares
no margin: the confirmation addenda are derived from the accepted pilot
receipts by the canonical freezer, not written here.

An addendum already committed is not rewritten: `--only` names the families to
write, so adding a family leaves the frozen text of the others alone.

Usage: make-addenda.py --bundles-dir DIR --out-dir DIR [--frozen-utc STAMP]
    [--only FAMILY]...
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

# Flooding rounds the isolated check-node preparation runs before it freezes the
# variable-to-check array; see `survey/arms/src/prepare.rs`.
WARMUP_ROUNDS = 8
# The committed matched-ness receipt of the isolated arms, which carries the
# checksum of each code's prepared array and of the pass's output.
PARITY = pathlib.Path("dev/bench_results/07ca8585/preparation/checknode-parity.jsonl")
# The prepared quality corpus the fixed-iteration cells decode against.
FIXED_QUALITY = pathlib.Path("dev/bench_results/07ca8585/preparation/quality-fixed")

CORE_ARMS = {
    "w1": ("single-core", 1, "sustained-throughput"),
    "p6": ("physical-cores-6", 6, "multicore-throughput"),
    "p12": ("physical-cores-12", 12, "multicore-throughput"),
    "l24": ("logical-cpus-24", 24, "multicore-throughput"),
}

KERNEL_OPERATION = (
    "Kernel-isolated operation version 1: each worker is a thread pinned to one resolved CPU and "
    "owns its own copy of the prepared variable-to-check message array of the first 8 recorded "
    "frames and its own output array of the same shape. Both are built before timing, from the "
    "frozen recorded LLRs and the recorded graph, by one function both arms call: every edge "
    "starts at its variable's recorded LLR and 8 flooding rounds follow, each one check-node pass "
    "and one variable-node pass, which is the state a flooding decode reaches. AList parsing, the "
    "layout build, the preparation and, for the AFF3CT arm, the permutation into its variable-major "
    "edge order are reported as setup, outside the timed windows. A timed call makes every worker "
    "run one flooding check-node pass over each of the 8 prepared frames, and nothing else: no "
    "variable update, no syndrome, no conversion and no allocation. Throughput is workers times 8 "
    "frames per call. The cell declares the checksum of the prepared array and of the output, both "
    "in the canonical check-major edge order; every worker of either arm reproduces both or the arm "
    "fails. An isolated pass decodes no frame, so the cell declares no decoder and the runner asks "
    "for no per-frame quality; what stands in its place is the committed matched-ness receipt "
    "`dev/bench_results/07ca8585/preparation/checknode-parity.jsonl`, which records that the two "
    "passes write bit-identical outputs over these workloads and that AFF3CT's transpose is the "
    "canonical edge map. Arms report observed per-worker affinity, CPUs and process thread counts; "
    "no nested pool is declared."
)

CHECKNODE = (
    "The changed gf2 check-node update against AFF3CT v4.7.0's own check-node update rule, "
    "isolated from the rest of a decode, which is REQ-10's kernel granularity. Both arms read the "
    "same prepared messages on the same edges of the same recorded graph, each in the edge layout "
    "its own production decoder holds them in, and write an output array of the same size; the gf2 "
    "arm runs `min_sum_check_row` over the canonical `EdgeLayout` check runs and the AFF3CT arm "
    "runs `tools::Update_rule_NMS<float>` over the scan order of "
    "`Decoder_LDPC_BP_flooding::_decode_single_ite`, at normalization factor 0.75. This family "
    "publishes the REQ-10 kernel comparison and selects nothing: adoption is decided by the "
    "before/after families, which keep their own ledgers. "
)

FIXED_ITERATION = (
    "The changed gf2 decoder against AFF3CT v4.7.0's flooding normalized min-sum at a fixed "
    "iteration count, which is REQ-10's full-iteration granularity: syndrome stopping is off on "
    "both arms, so each performs exactly the declared cap of 50 flooding iterations on every frame "
    "and the comparison is of equal iteration counts rather than of equal decisions reached at "
    "different counts. The reused `c077a88b` quality corpus was produced under syndrome stopping, "
    "which the harness refuses for these settings, so these cells decode against the corpus "
    "`dev/bench_results/07ca8585/preparation/quality-fixed/` records for exactly these settings: "
    "untimed, committed, produced by `survey/arms/src/bin/ldpc-fixed-quality.rs` from the same "
    "frozen frames, one decode of every recorded frame per arm. This family publishes the REQ-10 "
    "full-iteration comparison and selects nothing. "
)

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
    "ldpc-update-checknode-v1": {
        "arms": ("w1",),
        "objective": "comparator-gap",
        # An isolated check-node pass decodes no frame, so the family is a kernel
        # family: a decoder family's cells must each carry a decoder declaration.
        "purpose": "kernel-family",
        "ledger": "dev/bench_results/07ca8585/v4-update-checknode-family-ledger.jsonl",
        "metric_kind": "kernel-isolated",
        "scaling": "single-core-latency",
        "description": CHECKNODE + "One worker. " + KERNEL_OPERATION,
    },
    "ldpc-update-fixed-iteration-v1": {
        "arms": ("w1",),
        "objective": "comparator-gap",
        "purpose": "decoder-family",
        "ledger": "dev/bench_results/07ca8585/v4-update-fixed-iteration-family-ledger.jsonl",
        "stopping": "fixed",
        "description": FIXED_ITERATION + "One worker. " + OPERATION,
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


def kernel_cell(family, code_key, arm_key, manifest, parity):
    """One isolated check-node cell: a kernel with no decoder to declare."""
    core_arm, workers, _ = CORE_ARMS[arm_key]
    code = CODES[code_key]
    receipt = parity[manifest["code"]]
    return {
        "cell_id": f"{code_key}-checknode-{arm_key}",
        "objective": FAMILIES[family]["objective"],
        "role": "exploratory",
        "workload": {
            "identity": (
                f"{code['workload']}; prepared variable-to-check messages after "
                f"{WARMUP_ROUNDS} flooding rounds, input {receipt['input_checksum']}, "
                f"output {receipt['gf2_output_checksum']}"
            ),
            "size": {
                "n": manifest["n"],
                "k": manifest["k"],
                "frames": manifest["frames"],
                "timed_batch": TIMED_BATCH,
                "workers": workers,
                "timed_frames": TIMED_BATCH * workers,
                "timed_fixture_banks": 1,
                "checks": receipt["checks"],
                "edges": receipt["edges"],
                "max_check_degree": receipt["max_check_degree"],
                "warmup_rounds": WARMUP_ROUNDS,
            },
            "seed": manifest["seed"],
        },
        "metric_kind": FAMILIES[family]["metric_kind"],
        "scaling": FAMILIES[family]["scaling"],
        "core_arm": core_arm,
        "workers": {"declared": workers, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "native", "candidate": "external"},
        # The timed call reads the prepared array as it stands and converts
        # nothing; the preparation and the permutation are setup.
        "conversion_costs_included": False,
        # An isolated check-node pass decodes no frame, so there is no decoder
        # declaration to make and no per-frame quality to check it against.
        "decoder": None,
    }


def cell(family, code_key, arm_key, manifest):
    core_arm, workers, scaling = CORE_ARMS[arm_key]
    code = CODES[code_key]
    stopping = FAMILIES[family].get("stopping", "syndrome")
    granularity = "fixed-iteration" if stopping == "fixed" else "update"
    return {
        "cell_id": f"{code_key}-{granularity}-{arm_key}",
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
            "candidate": "external" if "comparator" in family or stopping == "fixed" else "native",
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
            "stopping": {"kind": stopping, "crc": None},
            "batching": {"batch_size": 1, "batch_fill_included": True},
            "quality_tolerance": {"fer_ratio_max": 1.0, "confidence": 0.95},
            "rate_matching": None,
        },
    }


def parity_receipt():
    """The matched-ness receipt of the isolated arms, keyed by code name."""
    rows = [json.loads(line) for line in PARITY.read_text().splitlines() if line.strip()]
    for row in rows:
        if not row["outputs_bit_identical"]:
            raise SystemExit(f"the isolated arms disagree on {row['code']}")
    return {row["code"]: row for row in rows}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bundles-dir", required=True)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--frozen-utc")
    parser.add_argument("--only", action="append", choices=sorted(FAMILIES), default=None)
    args = parser.parse_args()
    selected = set(args.only) if args.only else set(FAMILIES)

    bundles = pathlib.Path(args.bundles_dir)
    manifests = {
        key: json.loads((bundles / code["bundle"] / "manifest.json").read_text(encoding="utf-8"))
        for key, code in CODES.items()
    }
    frozen = args.frozen_utc or datetime.datetime.now(datetime.timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )

    parity = parity_receipt()
    out = pathlib.Path(args.out_dir)
    for family, spec in FAMILIES.items():
        if family not in selected:
            continue
        kernel = spec.get("metric_kind") == "kernel-isolated"
        cells = [
            kernel_cell(family, code_key, arm_key, manifests[code_key], parity)
            if kernel
            else cell(family, code_key, arm_key, manifests[code_key])
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
                "rationale": "A pilot supplies the observed resolution used to fix this family's "
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
