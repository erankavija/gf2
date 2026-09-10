"""Cell design of the two protocol-v3 families of jit:26465e6c.

The single source for arm identities, workloads and cell identifiers:
`make-addenda.py` writes the frozen addenda from it and `build-plan.py`
projects a runner plan from an addendum, checking every cell against it in
both directions.
"""

ISSUE = "26465e6c"
ARM_VAR = "GF2_POPCOUNT_ARM"
RESULTS = f"dev/bench_results/{ISSUE}"

# Arm name -> (build identity, description). The name is also the value of
# GF2_POPCOUNT_ARM the one survey binary dispatches on.
ARMS = {
    "production-dispatch": (
        "conservative-portable",
        "gf2 production dispatcher gf2_core::kernels::ops::popcount: portable scalar "
        "count_ones below 8 words, AVX2 nibble lookup from 8 words, chosen at run time",
    ),
    "nibble-lut": (
        "conservative-portable",
        "internal control: gf2's AVX2 nibble-lookup kernel popcnt_fn called directly, "
        "without the dispatcher's size threshold",
    ),
    "scalar-popcnt": (
        "conservative-portable",
        "internal control: scalar loop forced onto the POPCNT instruction by a "
        "runtime-checked target_feature function",
    ),
    "compiler-count-ones": (
        "conservative-portable",
        "internal control: compiler-optimized portable u64::count_ones loop, the "
        "lowering gf2's scalar backend runs",
    ),
    "libpopcnt": (
        "external",
        "libpopcnt v4.2 popcnt() compiled -O3 as its README recommends, with its own "
        "CPUID dispatch",
    ),
    "mula-avx2-harley-seal": (
        "external",
        "Mula sse-popcount popcnt_AVX2_harley_seal compiled with the upstream "
        "Makefile's AVX2 flags; requires 32-byte aligned input",
    ),
    "and-fused": (
        "conservative-portable",
        "gf2 fused AVX2 AND-popcount kernel and_popcnt_fn from the "
        "gf2_kernels_simd::detect() bundle, resolved once as BitMatrix::matvec_simd uses it",
    ),
    "and-scalar-control": (
        "conservative-portable",
        "internal control: single-pass portable (a & b).count_ones() loop",
    ),
    "and-two-pass": (
        "conservative-portable",
        "gf2-core public route: copy into a fresh temporary, kernels::ops::and_inplace, "
        "then kernels::ops::popcount, inside every call",
    ),
}

# Workload key -> (words, word_offset, pattern, cache_state, seed, why).
POPCOUNT_WORKLOADS = {
    "w4": (4, 0, "random", "warm", 301,
           "32 B, one vector: below gf2's 8-word SIMD threshold; libpopcnt runs scalar POPCNT"),
    "w8": (8, 0, "random", "warm", 302,
           "64 B: gf2's observed SIMD threshold (cmp $0x7); libpopcnt still scalar POPCNT"),
    "w12": (12, 0, "random", "warm", 303,
            "96 B: libpopcnt's observed AVX2 threshold (cmp $0x5f)"),
    "w60": (60, 0, "random", "warm", 304,
            "480 B, 15 whole vectors: Mula's carry-save loop does not engage and no byte tail remains"),
    "w64": (64, 0, "random", "warm", 305,
            "512 B, 16 vectors: Mula's carry-save loop engages (trip mask $0xfffffffffffffff0)"),
    "w128": (128, 0, "random", "warm", 306,
             "1 KiB: libpopcnt's observed Harley-Seal threshold (cmp $0x3ff)"),
    "w256": (256, 0, "random", "warm", 307,
             "2 KiB, L1-resident, vector aligned: the alignment control"),
    "w256-off24": (256, 3, "random", "warm", 307,
                   "2 KiB starting 24 bytes past a vector boundary; Mula's aligned loads cannot accept it"),
    "w64-ones": (64, 0, "all_one", "warm", 308,
                 "512 B, every bit set"),
    "w64-zeros": (64, 0, "all_zero", "warm", 309,
                  "512 B, every bit clear"),
    "w16384": (16384, 0, "random", "warm", 310,
               "128 KiB: L2-resident (512 KiB L2 per core)"),
    "w1m-streaming": (1 << 20, 0, "random", "streaming", 311,
                      "8 MiB per bank, 64 MiB rotated over eight banks: twice the 32 MiB L3 of one CCD"),
}
POPCOUNT_BASELINE = "production-dispatch"
POPCOUNT_CANDIDATES = [
    "nibble-lut",
    "scalar-popcnt",
    "compiler-count-ones",
    "libpopcnt",
    "mula-avx2-harley-seal",
]

# Workload key -> (words per operand, cache_state, seed, why).
AND_WORKLOADS = {
    "w4": (4, "warm", 401,
           "32 B per operand: below gf2-core's 8-word threshold, so the public route runs scalar"),
    "w4096": (4096, "warm", 402,
              "32 KiB per operand, 64 KiB pair: L2-resident"),
    "w512k-streaming": (1 << 19, "streaming", 403,
                        "4 MiB per operand, 64 MiB rotated over eight banks of pairs"),
}
AND_BASELINE = "and-fused"
# Candidate -> (metric kind, whole_consumer flag in the case).
AND_CANDIDATES = {
    "and-scalar-control": ("kernel-isolated", False),
    "and-two-pass": ("whole-consumer", True),
}
AND_RHS_SEED_OFFSET = 1000


def popcount_cells():
    """(cell_id, workload key, candidate) in addendum order."""
    cells = []
    for key, (_, offset, _, _, _, _) in POPCOUNT_WORKLOADS.items():
        for candidate in POPCOUNT_CANDIDATES:
            if candidate == "mula-avx2-harley-seal" and offset != 0:
                continue
            cells.append((f"popcount-{key}-vs-{candidate}", key, candidate))
    return cells


def and_cells():
    cells = []
    for candidate in AND_CANDIDATES:
        for key in AND_WORKLOADS:
            cells.append((f"and-popcnt-{key}-vs-{candidate.removeprefix('and-')}", key, candidate))
    return cells


FAMILIES = {
    "popcount": {
        "id": "popcount-baselines",
        "ledger": f"{RESULTS}/v3-popcount-family-ledger.jsonl",
        "baseline": POPCOUNT_BASELINE,
        "cells": popcount_cells,
    },
    "and-popcnt": {
        "id": "and-popcnt-baselines",
        "ledger": f"{RESULTS}/v3-and-popcnt-family-ledger.jsonl",
        "baseline": AND_BASELINE,
        "cells": and_cells,
    },
}


def addendum_path(family, mode):
    return f"dev/active/{ISSUE}/addendum-{family}-v3-{mode}.json"


def case_for(family, key, candidate):
    """The opaque case both arms of a cell receive."""
    if family == "popcount":
        words, offset, pattern, _, seed, _ = POPCOUNT_WORKLOADS[key]
        return {"op": "popcount", "pattern": pattern, "seed": seed,
                "word_offset": offset, "words": words}
    words, _, seed, _ = AND_WORKLOADS[key]
    return {"op": "and_popcnt", "pattern": "random", "seed_lhs": seed,
            "seed_rhs": seed + AND_RHS_SEED_OFFSET, "word_offset": 0,
            "whole_consumer": AND_CANDIDATES[candidate][1], "words": words}
