#!/usr/bin/env python3
"""Emits a `zen3-benchmark-plan-v1` runner plan for one LDPC survey family
(jit:c077a88b).

The addendum freezes what a cell is; the plan says which executables measure
it on this host. Arm executables and bundle directories are absolute host
paths, so the plan is generated beside its receipt rather than committed as a
fixed document; the receipt keeps the exact bytes and their digest.

Usage:
  make-plan.py --family <name> --label <pilot|confirmation> --addendum <path> \
      --arms-dir <dir> --bundles-dir <dir> --output <plan.json>
"""

import argparse
import json
import os

LOCK = "/tmp/gf2-ccx1.lock"
WRAPPER = "dev/scripts/ccx1-bench-flock.sh"
# One seed per family, fixed here so a resumed campaign reproduces its pair
# orders and bootstrap draws from the plan alone.
SEEDS = {
    "ldpc-matched-algorithm": 0x0C07_7A88_B1A7_C4ED,
    "ldpc-quality-compatible": 0x0C07_7A88_B9CA_11B7,
}

# Recorded-input bundle of each cell family member.
BUNDLES = {
    "dvb-t2-r12": "dvb-t2-r12-waterfall",
    "nr-bg1-z384": "nr-bg1-z384-mother",
}
CODES = {"dvb-t2-r12": "dvb-t2-r12", "nr-bg1-z384": "nr-bg1-r12"}

# Every arm the survey can place in a plan. `environment` is the only thing
# that distinguishes two arms built from the same executable, exactly as the
# protocol requires: the shared case never carries a backend selection.
ARMS = {
    "gf2-nms-f32": {
        "build": "native",
        "description": "gf2-coding LdpcDecoder, f32 flooding normalized min-sum, syndrome stopping",
        "executable": "gf2-ldpc-arm",
        "arguments": [],
        "environment": {},
        "rustflags": "-C target-cpu=native",
        "tuning_profile": None,
    },
    "aff3ct-flooding-nms-f32": {
        "build": "external",
        "description": "AFF3CT v4.7.0 Decoder_LDPC_BP_flooding<NMS>, f32, scalar",
        "executable": "aff3ct-ldpc-arm",
        "arguments": [],
        "environment": {
            "GF2_AFF3CT_TYPE": "flooding",
            "GF2_AFF3CT_IMPLEM": "NMS",
            "GF2_AFF3CT_SIMD": "",
            "GF2_AFF3CT_PRECISION": "f32",
            "GF2_AFF3CT_SYNDROME_DEPTH": "1",
        },
        "rustflags": "-C target-cpu=native",
        "tuning_profile": None,
    },
    "aff3ct-layered-nms-f32-inter": {
        "build": "external",
        "description": "AFF3CT v4.7.0 horizontal-layered ONMS inter, f32, INTER SIMD",
        "executable": "aff3ct-ldpc-arm",
        "arguments": [],
        "environment": {
            "GF2_AFF3CT_TYPE": "horizontal-layered",
            "GF2_AFF3CT_IMPLEM": "NMS",
            "GF2_AFF3CT_SIMD": "INTER",
            "GF2_AFF3CT_PRECISION": "f32",
            "GF2_AFF3CT_SYNDROME_DEPTH": "1",
        },
        "rustflags": "-C target-cpu=native",
        "tuning_profile": None,
    },
    "aff3ct-layered-nms-i16-inter": {
        "build": "external",
        "description": "AFF3CT v4.7.0 horizontal-layered ONMS inter, int16, INTER SIMD, LLR scale 16",
        "executable": "aff3ct-ldpc-arm",
        "arguments": [],
        "environment": {
            "GF2_AFF3CT_TYPE": "horizontal-layered",
            "GF2_AFF3CT_IMPLEM": "NMS",
            "GF2_AFF3CT_SIMD": "INTER",
            "GF2_AFF3CT_PRECISION": "i16",
            "GF2_AFF3CT_QUANT_SCALE": "16",
            "GF2_AFF3CT_SYNDROME_DEPTH": "1",
        },
        "rustflags": "-C target-cpu=native",
        "tuning_profile": None,
    },
}

# Cell order and case settings come from the frozen addendum, so the
# confirmatory manifest cannot silently acquire cells missing from its freeze.
ARMS["aff3ct-layered-nms-f32"] = dict(ARMS["aff3ct-layered-nms-f32-inter"])
ARMS["aff3ct-layered-nms-f32"]["environment"] = dict(ARMS["aff3ct-layered-nms-f32-inter"]["environment"], GF2_AFF3CT_SIMD="")
ARMS["aff3ct-layered-nms-f32"]["description"] = "AFF3CT v4.7.0 horizontal-layered NMS f32 scalar"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--family", required=True, choices=sorted(SEEDS))
    parser.add_argument("--label", required=True, choices=["pilot", "confirmation"])
    parser.add_argument("--addendum", required=True)
    parser.add_argument("--arms-dir", required=True)
    parser.add_argument("--bundles-dir", required=True)
    parser.add_argument("--quality-dir", required=True)
    parser.add_argument("--iteration-cap", type=int, default=50)
    parser.add_argument("--norm", type=float, default=0.75)
    parser.add_argument("--pilot-pairs", type=int, default=6)
    parser.add_argument("--max-cells-per-session", type=int, default=1)
    parser.add_argument("--campaign-id", default=None)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    addendum = json.load(open(args.addendum, encoding="utf-8"))
    cells = []
    arms = {}
    for declared in addendum["cells"]:
        cell_id = declared["cell_id"]
        decoder = declared["decoder"]
        code_key = "dvb-t2-r12" if cell_id.startswith("dvb") else "nr-bg1-z384"
        if "matched" in cell_id:
            candidate = "aff3ct-flooding-nms-f32"
        elif "i16" in cell_id:
            candidate = "aff3ct-layered-nms-i16-inter"
        elif "scalar" in cell_id:
            candidate = "aff3ct-layered-nms-f32"
        else:
            candidate = "aff3ct-layered-nms-f32-inter"
        names = []
        for name in ["gf2-nms-f32", candidate]:
            key = name + "-" + code_key
            arm = dict(ARMS[name])
            arm["executable"] = os.path.join(args.arms_dir, arm["executable"])
            arm["environment"] = dict(arm["environment"],
                GF2_LDPC_QUALITY=os.path.join(args.quality_dir, key + ".json"))
            if not os.path.isfile(arm["executable"]):
                raise SystemExit("missing executable " + arm["executable"])
            arms[key] = arm
            names.append(key)
        cells.append({
            "cell_id": cell_id, "baseline_arm": names[0], "candidate_arm": names[1],
            "case": {
                "bundle": os.path.join(args.bundles_dir, BUNDLES[code_key]),
                "code": CODES[code_key], "iteration_cap": decoder["iteration_cap"],
                "normalization_factor": decoder["normalization"]["factor"],
                "syndrome_stopping": True,
                "batch_size": declared["workload"]["size"]["timed_batch"],
                "quality_frames": decoder["input"]["frames"],
            },
            "pilot_pairs": args.pilot_pairs if args.label == "pilot" else None,
        })

    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id or "c077a88b-%s-%s" % (args.family, args.label),
        "issue": "c077a88b",
        "label": args.label,
        "campaign_seed": SEEDS[args.family],
        "addendum": args.addendum,
        "producing_manifest": "dev/active/c077a88b/survey/producing-inputs.json",
        "lock_path": LOCK,
        "wrapper": WRAPPER,
        "timing_override": None,
        "arms": arms,
        "cells": cells,
        "max_cells_per_session": args.max_cells_per_session,
    }
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(plan, handle, indent=2, sort_keys=False)
        handle.write("\n")
    print(args.output)


if __name__ == "__main__":
    main()
