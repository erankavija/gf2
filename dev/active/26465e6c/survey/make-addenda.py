#!/usr/bin/env python3
"""Writes the protocol-v3 family addenda of jit:26465e6c.

  make-addenda.py pilot <popcount|and-popcnt>
  make-addenda.py confirmation <popcount|and-popcnt> <pilot-receipt-dir>

`pilot` freezes an all-exploratory addendum from `families.py`. `confirmation`
freezes the same cells as confirmatory and derives the resolution from the
named accepted v3 pilot: the widest relative half-width of the pilot's
intervals in its acceptance summary (the corrected alpha P-03 recomputes it
at), rounded up to a hundredth. Each declared margin keeps its
consumer-benefit value unless that value does not strictly exceed one plus the
resolution; then it rises to the next hundredth that does, and its rationale
says so. Margins are compared as decimals, never as binary floats. Both modes
refuse to overwrite an existing addendum.

Run from the repository root.
"""

import datetime
import hashlib
import json
import pathlib
import sys
from decimal import ROUND_CEILING, Decimal

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import families  # noqa: E402

HUNDREDTH = Decimal("0.01")
MATERIAL_GAP = Decimal("1.10")
EQUIVALENCE = Decimal("1.05")

DESCRIPTIONS = {
    "popcount": (
        "Question: on the Ryzen 9 5900X, is gf2's production population-count dispatcher "
        "(gf2_core::kernels::ops::popcount) the fastest measured arm for each workload class, "
        "against libpopcnt v4.2, Mula's AVX2 Harley-Seal reference and the labelled internal "
        "controls (gf2's nibble-lookup kernel called directly, a forced scalar POPCNT loop, the "
        "portable count_ones loop)? The dispatcher is the baseline of every cell and each "
        "alternative its candidate, so a speedup above 1 means the alternative is faster. "
        "Workloads bracket the thresholds read from the emitted instructions (gf2 at 8 words, "
        "libpopcnt at 96 B and 1 KiB, Mula's 16-vector carry-save loop at 512 B), a window 24 "
        "bytes past a vector boundary with its aligned control, all-ones and all-zero patterns, "
        "an L2-resident size and a streaming size. Every arm is a single-threaded leaf kernel, "
        "so every cell is single-core. Cells are warm or streaming: the protocol's cold state "
        "is a fixed-call first-use series that nanosecond calls cannot resolve above timer "
        "overhead. Mula's reference is absent from the misaligned workload because it loads "
        "through const __m256i*. This family continues the question whose protocol-v1 "
        "confirmation popcount-26465e6c-20260908 the ledger imports."
    ),
    "and-popcnt": (
        "Question: how does gf2's fused AVX2 AND-popcount kernel (and_popcnt_fn, which "
        "BitMatrix::matvec_simd calls per row) compare with a single-pass portable control and "
        "with the two-pass public gf2-core route a consumer takes today (copy into a temporary, "
        "kernels::ops::and_inplace, kernels::ops::popcount) at a small, an L2-resident and a "
        "streaming size? The fused kernel is the baseline of every cell, so a speedup below 1 "
        "means the fused kernel is faster. Two-pass cells are whole-consumer: the temporary "
        "buffer and the extra pass lie inside every timed call. Neither pinned external library "
        "exposes a fused AND or XOR reduction, so no external arm exists. Every arm is "
        "single-threaded, so every cell is single-core. The protocol-v1 confirmation of this "
        "question was frozen but never run."
    ),
}


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def cell(family, cell_id, key, candidate, role):
    if family == "popcount":
        words, offset, pattern, cache, seed, why = families.POPCOUNT_WORKLOADS[key]
        size = {"words": words, "word_offset": offset}
        identity = f"popcount-{pattern.replace('_', '')}-{key}: {why}"
        metric = "kernel-isolated"
        conversion = False
    else:
        words, cache, seed, why = families.AND_WORKLOADS[key]
        size = {"words": words, "word_offset": 0}
        metric, conversion = families.AND_CANDIDATES[candidate]
        identity = f"and-popcnt-random-{key}: {why}"
    baseline = families.FAMILIES[family]["baseline"]
    return {
        "cell_id": cell_id,
        "objective": "comparator-gap",
        "role": role,
        "workload": {"identity": identity, "size": size, "seed": seed},
        "metric_kind": metric,
        "scaling": "sustained-throughput",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache,
        "builds": {
            "baseline": families.ARMS[baseline][0],
            "candidate": families.ARMS[candidate][0],
        },
        "conversion_costs_included": conversion,
        "decoder": None,
    }


def effect(family, resolution=None, evidence=None, margins=None):
    material, material_note, equivalence, equivalence_note = margins or (
        MATERIAL_GAP, "", EQUIVALENCE, "")
    subject = "gf2's route" if family == "popcount" else "the fused kernel"
    return {
        "worthwhile_speedup": None,
        "rationale": "Unused: every cell is comparator-gap and decides against the material-gap threshold.",
        "measurement_resolution": None if resolution is None else float(resolution),
        "resolution_evidence": evidence,
        "equivalence_margin": float(equivalence),
        "equivalence_rationale": (
            f"An alternative within 5% of {subject} is not a materially different "
            "implementation for a consumer of these leaf kernels, so an interval whose lower "
            "bound clears 1/margin records not-worse rather than a gap." + equivalence_note
        ),
        "material_gap_threshold": float(material),
        "material_gap_rationale": (
            f"An alternative must run at least 10% faster than {subject} before replacing "
            f"{subject}, or moving a dispatch threshold toward the alternative, could repay "
            "its maintenance and dependency cost for a consumer; smaller gaps leave the "
            "established route in place." + material_note
        ),
    }


def addendum(family, mode, cells, effect_block):
    spec = families.FAMILIES[family]
    return {
        "schema": "zen3-benchmark-addendum-v3",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 3},
        "family": {
            "id": spec["id"],
            "issue": families.ISSUE,
            "purpose": "kernel-family",
            "description": f"Protocol-v3 {mode} of jit:{families.ISSUE}. " + DESCRIPTIONS[family],
        },
        "frozen": {"frozen_utc": utc_now()},
        "effect": effect_block,
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 0,
            "maintenance_rationale": (
                "A baseline survey: it authorizes no production change. Adoption belongs to the "
                "optimization issue that consumes this baseline, with its own before/after evidence."
            ),
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": spec["ledger"],
        },
        "search_budget": {"max_pilot_trials_per_cell": 2, "max_confirmatory_attempts_per_candidate": 1},
        "holdout": {"required": False, "cells": []},
        "cells": cells,
    }


def widest_half_width(summary):
    widest = Decimal(0)
    for entry in summary["cells"]:
        interval = entry.get("interval")
        if interval is None:
            continue
        estimate = Decimal(repr(interval["estimate"]))
        lower = Decimal(repr(interval["lower"]))
        upper = Decimal(repr(interval["upper"]))
        widest = max(widest, max(abs(estimate - lower), abs(upper - estimate)) / estimate)
    return widest


def admissible(declared, resolution, name):
    floor = 1 + resolution
    if declared > floor:
        return declared, ""
    raised = (floor + HUNDREDTH).quantize(HUNDREDTH, rounding=ROUND_CEILING)
    while raised <= floor:
        raised += HUNDREDTH
    return raised, (
        f" The consumer-benefit value {declared} does not strictly exceed one plus the "
        f"pilot resolution {resolution}, so the operative {name} is {raised}."
    )


def write_new(path, document):
    target = pathlib.Path(path)
    if target.exists():
        sys.exit(f"{path} exists; a frozen addendum is never rewritten")
    target.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    print(f"{path}: sha256 {hashlib.sha256(target.read_bytes()).hexdigest()}")


def main():
    if len(sys.argv) < 3 or sys.argv[2] not in families.FAMILIES:
        sys.exit(__doc__)
    mode, family = sys.argv[1], sys.argv[2]
    spec = families.FAMILIES[family]
    if mode == "pilot" and len(sys.argv) == 3:
        cells = [cell(family, cid, key, cand, "exploratory") for cid, key, cand in spec["cells"]()]
        write_new(families.addendum_path(family, "pilot"),
                  addendum(family, "pilot", cells, effect(family)))
    elif mode == "confirmation" and len(sys.argv) == 4:
        pilot = pathlib.Path(sys.argv[3])
        receipt = pilot / "receipt.json"
        digest = hashlib.sha256(receipt.read_bytes()).hexdigest()
        summary = json.loads((pilot / "acceptance-summary.json").read_text(encoding="utf-8"))
        if summary["receipt_sha256"] != digest or summary["verdict"] != "accepted":
            sys.exit("the pilot summary does not accept this receipt")
        if summary["label"] != "pilot" or summary["family"]["family_id"] != spec["id"]:
            sys.exit("resolution evidence must be this family's pilot")
        widest = widest_half_width(summary)
        resolution = widest.quantize(HUNDREDTH, rounding=ROUND_CEILING)
        if resolution <= widest:
            resolution += HUNDREDTH
        material, material_note = admissible(MATERIAL_GAP, resolution, "material-gap threshold")
        equivalence, equivalence_note = admissible(EQUIVALENCE, resolution, "equivalence margin")
        cells = [cell(family, cid, key, cand, "confirmatory") for cid, key, cand in spec["cells"]()]
        evidence = {"receipt": str(receipt), "sha256": digest}
        write_new(
            families.addendum_path(family, "confirmation"),
            addendum(family, "confirmation", cells,
                     effect(family, resolution, evidence,
                            (material, material_note, equivalence, equivalence_note))),
        )
        print(f"widest pilot relative half-width {widest}; declared resolution {resolution}; "
              f"material gap {material}; equivalence {equivalence}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
