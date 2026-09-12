#!/usr/bin/env python3
"""Project a zen3-benchmark-plan-v1 from a frozen c7113c5a family addendum.

Usage:
  make-plan.py --addendum <json> --label pilot|confirmation --campaign-id <id>
               --campaign-seed <n> --ext <staging> --lock <path>
               --max-cells-per-session <n> [--pilot-pairs <n>] --output <plan>

Every numeric setting comes from the addendum and the protocol's frozen shared
settings; this script adds only the arm wiring. A cell identifier names its
build level (`-conservative-`, `-tuned-`, otherwise native) and whether it
measures the public long-product API (`-public-api-`); the baseline is always
the gf2 arm and the candidate is always the gf2x arm of the same level, so a
speedup above one favours gf2x. The arm build identities are checked against
the addendum's declared builds.
"""

import argparse
import json

PRODUCING_MANIFEST = "dev/active/c7113c5a/survey/producing-inputs.json"
WRAPPER = "dev/scripts/ccx1-bench-flock.sh --full-host"
GF2X = "gf2x 1.3.0 (tag gf2x-1.3.0, commit 27ba588f03bf6e1e74763903bab25e6e8bb6d0f0, GPL-3.0-or-later)"

LEVELS = {
    "conservative": {"build": "conservative-portable", "rustflags": None, "cflags": "-O2"},
    "tuned": {"build": "tuned-portable", "rustflags": "-C target-cpu=x86-64-v3", "cflags": "-O3 -march=x86-64-v3"},
    "native": {"build": "native", "rustflags": "-C target-cpu=native", "cflags": "-O3 -march=native"},
}


def gf2_arm(ext, level, public_api):
    spec = LEVELS[level]
    what = ("the public clmul_wide_slice long-product API (GF2_POLY_PATH=schoolbook)"
            if public_api else
            "dispatched wide kernels, the scalar schoolbook beyond them, the detected raw-batch lane "
            "and FieldVec::simd_dot_product")
    return {
        "build": spec["build"],
        "description": (f"gf2 {level} build of poly-baseline-arms: {what}; gf2-core and gf2-kernels-simd "
                        f"by path, RUSTFLAGS={spec['rustflags'] or '(none)'}"),
        "executable": f"{ext}/arms-{level}/release/gf2-poly-arm",
        "arguments": [],
        "environment": {"GF2_POLY_PATH": "schoolbook"} if public_api else {},
        "rustflags": spec["rustflags"],
        "tuning_profile": None,
    }


def gf2x_arm(ext, level):
    spec = LEVELS[level]
    return {
        "build": "external",
        "description": (f"{GF2X}, gf2x_mul_r built with CFLAGS={spec['cflags']} (configure appends "
                        f"-msse2 -msse3 -mssse3 -msse4.1 -mpclmul; hwdir x86_64_pclmul); Rust shell "
                        f"RUSTFLAGS={spec['rustflags'] or '(none)'}; the arm asserts it mapped "
                        f"{ext}/prefix-{level}/lib/libgf2x.so and reports that file's SHA-256"),
        "executable": f"{ext}/arms-{level}/release/gf2x-poly-arm",
        "arguments": [],
        "environment": {},
        "rustflags": spec["rustflags"],
        "tuning_profile": None,
    }


def wiring(cell_id):
    level = "native"
    for candidate in ("conservative", "tuned"):
        if f"-{candidate}-" in cell_id:
            level = candidate
    public_api = "-public-api-" in cell_id
    baseline = f"gf2-{level}" + ("-public-api" if public_api else "")
    return level, public_api, baseline, f"gf2x-{level}"


def case(workload):
    identity, size, seed = workload["identity"], workload["size"], workload["seed"]
    inner = size.get("inner", 1)
    if "words" in size:
        return {"kind": "poly-mul", "words": size["words"], "inner": inner, "seed": seed}
    if "clmul-batch" in identity:
        return {"kind": "clmul-batch", "count": size["count"], "inner": inner, "seed": seed}
    if "gf2m-dot" in identity:
        return {"kind": "gf2m-dot", "count": size["count"], "inner": inner, "seed": seed}
    raise SystemExit(f"workload {identity!r} has no case projection")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--addendum", required=True)
    parser.add_argument("--label", required=True, choices=["pilot", "confirmation"])
    parser.add_argument("--campaign-id", required=True)
    parser.add_argument("--campaign-seed", required=True, type=int)
    parser.add_argument("--ext", required=True)
    parser.add_argument("--lock", required=True)
    parser.add_argument("--max-cells-per-session", required=True, type=int)
    parser.add_argument("--pilot-pairs", type=int)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    with open(args.addendum) as handle:
        addendum = json.load(handle)
    arms, cells = {}, []
    for declared in addendum["cells"]:
        level, public_api, baseline, candidate = wiring(declared["cell_id"])
        arms.setdefault(baseline, gf2_arm(args.ext, level, public_api))
        arms.setdefault(candidate, gf2x_arm(args.ext, level))
        for name, build in ((baseline, declared["builds"]["baseline"]),
                            (candidate, declared["builds"]["candidate"])):
            if arms[name]["build"] != build:
                raise SystemExit(f"{declared['cell_id']}: arm {name} is {arms[name]['build']}, addendum declares {build}")
        exploratory = declared["role"] == "exploratory"
        cells.append({
            "cell_id": declared["cell_id"],
            "baseline_arm": baseline,
            "candidate_arm": candidate,
            "case": case(declared["workload"]),
            "pilot_pairs": args.pilot_pairs if exploratory else None,
        })
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": args.campaign_id,
        "issue": addendum["family"]["issue"],
        "label": args.label,
        "campaign_seed": args.campaign_seed,
        "addendum": args.addendum,
        "producing_manifest": PRODUCING_MANIFEST,
        "lock_path": args.lock,
        "wrapper": WRAPPER,
        "timing_override": None,
        "arms": dict(sorted(arms.items())),
        "cells": cells,
        "max_cells_per_session": args.max_cells_per_session,
    }
    with open(args.output, "w") as handle:
        json.dump(plan, handle, indent=2)
        handle.write("\n")
    print(f"plan: {len(cells)} cells, {len(arms)} arms -> {args.output}")


if __name__ == "__main__":
    main()
