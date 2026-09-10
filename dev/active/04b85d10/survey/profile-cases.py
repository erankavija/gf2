#!/usr/bin/env python3
"""Case ladder of the bit-storage consumer profile (jit:04b85d10).

Emits one JSON object per line: the workload, the production route, the case
the driver forwards, the declared cache state and the wall-clock target of the
run. The ladder is derived here rather than written out so a size and its
justification stay in one place.

Sizes are justified against this host's cache hierarchy, observed at run time
and recorded in the run's host record: 32 KiB L1d and 512 KiB L2 per core,
32 MiB L3 per core complex. The BCH rows and batch ladder are those of
`dev/active/4e732b56/workload-selection.md` sections 2 and 3.
"""

import json
import sys

# Word counts spanning the hierarchy for a single packed buffer: the
# conservative SIMD dispatch boundary, one and four cache lines beyond it, an
# L1-resident row, the two DVB-T2 LDPC syndrome widths, an L2-resident buffer
# and an L3-resident one.
BUFFER_WORDS = [4, 8, 16, 64, 127, 507, 4096, 65536]

# Rows of one logical bank. Sixty-four rows keep the bank a whole number of
# 64x64 blocks and make the per-row cost of a size sweep comparable.
ROW_BANK_ROWS = 64
# Row widths whose 64-row banks land in L1 (2-32 KiB), L2 (256 KiB) and L3
# (4 MiB).
ROW_BANK_WORDS = [4, 8, 16, 64, 512, 8192]

# Dense square shapes: L1-resident, L2-resident, and two L3-resident ones.
DENSE_SHAPES = [(256, 256), (512, 512), (1024, 1024), (2048, 2048)]
# Dense matrix-vector shapes, including a wide one whose rows exceed L1.
MATVEC_SHAPES = [(256, 256), (1024, 1024), (1024, 4096), (4096, 4096)]
# Dense transpose shapes at one and sixteen mebibits.
TRANSPOSE_SHAPES = [(1024, 1024), (4096, 4096)]
# Block counts of the 64x64 transpose primitive: one block, an L1-resident
# run, an L2-resident run and an L3-resident run.
TRANSPOSE_BLOCKS = [1, 16, 256, 4096]

# DVB-T2 LDPC frame lengths, rate 1/2.
LDPC_LENGTHS = [16200, 64800]

# Mother-field degrees of the BCH rows: 8 is contract row B3, 14 and 16 are
# the mother codes of the DVB-T2 short and normal frames.
BCH_DEGREES = [8, 14, 16]
# The contract's batch ladder, extended by 64 and 1024 so the lane-group
# boundary of a bit-sliced family is bracketed.
BCH_BATCHES = [1, 16, 64, 256, 1024]
BCH_FAMILY_PATHS = [
    "current",
    "family-poly-remainder-scalar",
    "family-table-remainder",
    "family-bitslice-interleaved",
    "family-clmul-fold",
]

# DVB-T2 shortened BCH frame lengths reached through the other entry point.
DVB_BCH = [(7200, 16), (32400, 1)]


def case(family, workload, path, size, seed, cache_state="warm", target_ms=1000, env=None):
    return {
        "family": family,
        "workload": workload,
        "path": path,
        "cache_state": cache_state,
        "target_ms": target_ms,
        "env": env or {},
        "case": {"workload": workload, "size": size, "seed": seed},
    }


def cases():
    for words in ROW_BANK_WORDS:
        for path in ("ops-dispatched", "ops-resolved", "scalar-backend", "simd-backend"):
            yield case(
                "logical",
                "row-xor",
                path,
                {"rows": ROW_BANK_ROWS, "words": words},
                seed=101,
            )

    for rows, cols in DENSE_SHAPES:
        yield case("logical", "dense-rref", "current", {"rows": rows, "cols": cols}, seed=102)

    for rows, cols in MATVEC_SHAPES:
        yield case("logical", "dense-matvec", "current", {"rows": rows, "cols": cols}, seed=103)

    for length in LDPC_LENGTHS:
        yield case("logical", "ldpc-syndrome", "current", {"n": length}, seed=104)

    for words in BUFFER_WORDS:
        for path in ("ops-dispatched", "scalar-backend", "simd-backend"):
            yield case("count", "popcount", path, {"words": words}, seed=201)

    for words in [64, 127, 507, 4096]:
        # The all-zero buffer is the worst case for an early-exit search: it
        # scans everything. The first-bit-set buffer is its best case.
        for set_bit in (words * 64, 0):
            for path in ("count-ones", "find-first-one"):
                yield case(
                    "count",
                    "zero-test",
                    path,
                    {"words": words, "set_bit": set_bit},
                    seed=202,
                )

    for length in LDPC_LENGTHS:
        for path in ("count-ones", "find-first-one"):
            yield case("count", "ldpc-codeword-check", path, {"n": length}, seed=203)

    for blocks in TRANSPOSE_BLOCKS:
        for path in ("transpose-scalar", "transpose-detected"):
            yield case("layout", "transpose-64x64", path, {"blocks": blocks}, seed=301)

    for rows, cols in TRANSPOSE_SHAPES:
        yield case("layout", "dense-transpose", "current", {"rows": rows, "cols": cols}, seed=302)

    for degree in BCH_DEGREES:
        for batch in BCH_BATCHES:
            # The largest mother code at the largest batch encodes 67 Mibit per
            # call in the reference family; the ladder stops before it.
            if degree == 16 and batch > 256:
                continue
            for path in BCH_FAMILY_PATHS:
                yield case(
                    "layout",
                    "bch-encode-batch",
                    path,
                    {"degree": degree, "batch": batch},
                    seed=303,
                )

    for degree in (8, 14):
        for batch in (16, 256):
            yield case(
                "layout",
                "bch-encode-batch-alloc",
                "current",
                {"degree": degree, "batch": batch},
                seed=304,
            )

    for workers in (1, 6):
        for batch in (256, 1024):
            yield case(
                "layout",
                "bch-encode-batch-parallel",
                "current",
                {"degree": 14, "batch": batch, "workers": workers},
                seed=305,
                env={"RAYON_NUM_THREADS": str(workers)},
            )

    # One call per message of a batch, so the row reads directly as the
    # per-batch cost of the field-identity check a batch encode performs.
    for calls in (1, 16, 256, 1024):
        yield case(
            "layout",
            "field-id-hint",
            "current",
            {"calls": calls},
            seed=307,
        )

    for length, batch in DVB_BCH:
        yield case(
            "layout",
            "dvb-bch-encode",
            "current",
            {"n": length, "batch": batch},
            seed=306,
            target_ms=2000,
        )


def main():
    for entry in cases():
        sys.stdout.write(json.dumps(entry, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
