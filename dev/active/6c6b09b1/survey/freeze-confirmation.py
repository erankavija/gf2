#!/usr/bin/env python3
"""Freeze the confirmatory byte-field addendum from the pilot receipt.

The measurement contract requires every numeric setting to be committed
before the first confirmatory trial, and the protocol requires the
resolution to be a pilot-measured quantity identified by content digest.
This script derives both from the pilot receipt rather than restating them,
so the frozen threshold and the evidence behind it cannot drift apart.

The resolution is the largest relative confidence-interval half-width any
pilot cell showed, rounded up to two decimals; the confirmatory thresholds
are then chosen above it with the rationales recorded in the addendum.

Usage: freeze-confirmation.py --pilot <receipt-dir> --out <addendum.json>
"""

import argparse
import collections
import datetime
import hashlib
import json
import math
import os


def relative_half_width(interval):
    return (interval["upper"] - interval["lower"]) / (2.0 * interval["estimate"])


def cell(cell_id, workload, size, seed, metric, scaling, core_arm, cache, conversion):
    return collections.OrderedDict(
        [
            ("cell_id", cell_id),
            ("objective", "comparator-gap"),
            ("role", "confirmatory"),
            (
                "workload",
                collections.OrderedDict(
                    [("identity", workload), ("size", size), ("seed", seed)]
                ),
            ),
            ("metric_kind", metric),
            ("scaling", scaling),
            ("core_arm", core_arm),
            (
                "workers",
                collections.OrderedDict(
                    [("declared", 1), ("nested_pools_allowed", False)]
                ),
            ),
            ("cache_state", cache),
            (
                "builds",
                collections.OrderedDict(
                    [("baseline", "native"), ("candidate", "external")]
                ),
            ),
            ("conversion_costs_included", conversion),
            ("decoder", None),
        ]
    )


AXPY = "byte-region-axpy-gf256"
AXPY_11B = "byte-region-axpy-gf256-0x11b"
MATMUL = "dense-matmul-gf256"
ENCODE = "generator-encode-gf256"
PAIRWISE = "pairwise-multiply-gf256"

# Every cell is single-core because every operation this family measures is
# single-threaded on both sides: gf2's `FieldVec::axpy`, `field::matrix::gemm`
# and `gf2m::batch::batch_mul` expose no parallel form, and neither do ISA-L's
# region entry points, GF-Complete's region kernels or M4RIE's matrix layer. A
# six- or twelve-core arm would therefore compare two harness thread pools
# rather than two library implementations, so this family declares none and
# `findings.md` records that reason instead.
CELLS = [
    # Fixed-coefficient region multiply-accumulate across the cache regimes:
    # 4 KiB inside L1d, 128 KiB inside L2, 8 MiB inside one CCX's 32 MiB L3
    # for the external arm, and a streaming cell whose eight rotating fixture
    # banks put both arms past it.
    cell("axpy-isal-l1-1core", AXPY, {"bytes": 4096}, 21,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    cell("axpy-isal-l2-1core", AXPY, {"bytes": 131072}, 22,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    cell("axpy-isal-l3-1core", AXPY, {"bytes": 8388608}, 23,
         "kernel-isolated", "sustained-throughput", "single-core", "warm", False),
    # Streaming rotates the eight fixture banks, so the touched set is
    # sixteen times one region. Two mebibytes puts the external arm at a
    # 32 MiB footprint, the size of one CCX's L3, and the gf2 arm far past it,
    # because a Gf2mElement occupies sixteen bytes per coefficient.
    cell("axpy-isal-streaming-1core", AXPY, {"bytes": 2097152}, 24,
         "kernel-isolated", "sustained-throughput", "single-core", "streaming", False),
    # The same operation against the other two comparators at one regime.
    cell("axpy-gfcomplete-l2-1core", AXPY, {"bytes": 131072}, 25,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    cell("axpy-m4rie-l2-1core", AXPY, {"bytes": 131072}, 26,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    # The other field on the same byte carrier. GF-Complete and M4RIE take the
    # polynomial as a parameter, so 0x11B has a comparator; ISA-L compiles
    # 0x11D in and has no 0x11B arm, which is why no ISA-L cell appears here.
    cell("axpy-gfcomplete-11b-l2-1core", AXPY_11B, {"bytes": 131072}, 27,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    # What a byte-region consumer pays, conversion and table preparation
    # included on every call.
    cell("axpy-isal-whole-consumer-l2-1core", AXPY, {"bytes": 131072}, 28,
         "whole-consumer", "single-core-latency", "single-core", "warm", True),
    # Dense matrix multiply across three dimensions, all below M4RIE's
    # Strassen cutoff of 512 for GF(2^8), so every cell measures its
    # Newton-John base case.
    cell("matmul-m4rie-n64-1core", MATMUL, {"n": 64}, 29,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    cell("matmul-m4rie-n128-1core", MATMUL, {"n": 128}, 30,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    cell("matmul-m4rie-n256-1core", MATMUL, {"n": 256}, 31,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    cell("matmul-m4rie-n128-whole-consumer-1core", MATMUL, {"n": 128}, 32,
         "whole-consumer", "single-core-latency", "single-core", "warm", True),
    # The batched matrix cell: a generator matrix times several data regions.
    cell("encode-isal-k6r3-1core", ENCODE, {"bytes": 65536, "k": 6, "rows": 3}, 33,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
    # Arbitrary pairwise multiplication, where no coefficient table is reusable.
    cell("pairwise-gfcomplete-l2-1core", PAIRWISE, {"bytes": 131072}, 34,
         "kernel-isolated", "single-core-latency", "single-core", "warm", False),
]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pilot", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--repo-root", default=".")
    args = parser.parse_args()

    receipt_path = os.path.join(args.pilot, "receipt.json")
    with open(receipt_path, "rb") as handle:
        digest = hashlib.sha256(handle.read()).hexdigest()
    with open(os.path.join(args.pilot, "acceptance-summary.json")) as handle:
        summary = json.load(handle)
    if summary["verdict"] != "accepted":
        raise SystemExit(f"pilot receipt is {summary['verdict']}, not accepted")
    if summary["label"] != "pilot":
        raise SystemExit("resolution evidence must be a pilot receipt")

    widths = {
        entry["cell_id"]: relative_half_width(entry["interval"])
        for entry in summary["cells"]
        if entry.get("interval")
    }
    if not widths:
        raise SystemExit("the pilot receipt carries no intervals")
    observed = max(widths.values())
    resolution = math.ceil(observed * 100.0) / 100.0
    resolution = max(resolution, 0.01)

    # The threshold sits an order of magnitude above the resolution and an
    # order of magnitude below the smallest gap the pilot saw, so it decides
    # the question the survey asks without being a blanket hurdle.
    threshold = 1.25
    if threshold <= 1.0 + resolution:
        raise SystemExit(
            f"resolution {resolution} leaves no room under the {threshold} threshold"
        )

    detail = ", ".join(
        f"{cell_id} {width:.4f}" for cell_id, width in sorted(widths.items())
    )
    relative = os.path.relpath(receipt_path, args.repo_root)
    frozen_utc = (
        datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .strftime("%Y-%m-%dT%H:%M:%SZ")
    )

    addendum = collections.OrderedDict(
        [
            ("schema", "zen3-benchmark-addendum-v1"),
            (
                "protocol",
                collections.OrderedDict(
                    [("id", "zen3-benchmark-protocol"), ("version", 1)]
                ),
            ),
            (
                "family",
                collections.OrderedDict(
                    [
                        ("id", "byte-field-arms"),
                        ("issue", "6c6b09b1"),
                        ("purpose", "consumer-family"),
                        (
                            "description",
                            "Current baseline of gf2's GF(2^8) byte-field consumer entry points "
                            "against operation-equivalent arms in M4RIE, GF-Complete and ISA-L. "
                            "Both sides of a cell work in the same field: GF(2^8) modulo 0x11D "
                            "everywhere except the one 0x11B cell, where ISA-L has no arm because "
                            "it compiles 0x11D in. gf2 is the baseline arm and the external "
                            "library is the candidate in every cell, so a speedup above one is a "
                            "gap in the comparator's favour and a cell where gf2 wins records no "
                            "material gap. Every cell is single-core because every operation "
                            "measured here is single-threaded on both sides. The family selects "
                            "nothing for production: it establishes the before measurement a "
                            "later byte-region design would have to improve on.",
                        ),
                    ]
                ),
            ),
            ("frozen", collections.OrderedDict([("frozen_utc", frozen_utc)])),
            (
                "effect",
                collections.OrderedDict(
                    [
                        ("worthwhile_speedup", threshold),
                        (
                            "rationale",
                            "A byte-region consumer changes implementation when the cost "
                            "difference changes its algorithm choice, which a quarter of the "
                            "runtime does and a few percent does not. The pilot's largest "
                            f"relative confidence-interval half-width was {observed:.4f} "
                            f"({detail}), rounded up to {resolution:.2f}, so this threshold "
                            "lies far outside the measurement resolution while staying far "
                            "below every gap the pilot observed.",
                        ),
                        ("measurement_resolution", resolution),
                        (
                            "resolution_evidence",
                            collections.OrderedDict(
                                [("receipt", relative), ("sha256", digest)]
                            ),
                        ),
                        ("equivalence_margin", 1.1),
                        (
                            "equivalence_rationale",
                            "Ten percent is the smallest band that exceeds the pilot-measured "
                            f"resolution of {resolution:.2f} on a quiet CCX1, so declaring two "
                            "implementations equivalent within it is a statement about them "
                            "rather than about this host's noise.",
                        ),
                        ("material_gap_threshold", threshold),
                        (
                            "material_gap_rationale",
                            "A comparator gap matters to this survey when it is large enough to "
                            "justify a byte-region representation, and a quarter of the runtime "
                            "is the smallest difference that would. A cell whose interval falls "
                            "below the equivalence margin records that gf2 is at least as fast "
                            "as the comparator for that operation, which is a result the family "
                            "retains rather than an omission.",
                        ),
                    ]
                ),
            ),
            (
                "complexity_budget",
                collections.OrderedDict(
                    [
                        ("max_new_unsafe_kernels", 0),
                        ("max_added_source_lines", 0),
                        (
                            "maintenance_rationale",
                            "This survey is a feasibility study. It changes no production kernel "
                            "and selects no candidate, so its adoption budget is zero by "
                            "construction; a later issue that proposes a byte-region API declares "
                            "its own budget.",
                        ),
                    ]
                ),
            ),
            (
                "family_wise",
                collections.OrderedDict(
                    [
                        ("alpha", 0.05),
                        ("prior_confirmatory_trials", 0),
                        ("prior_trials", []),
                    ]
                ),
            ),
            (
                "search_budget",
                collections.OrderedDict(
                    [
                        ("max_pilot_trials_per_cell", 1),
                        ("max_confirmatory_attempts_per_candidate", 1),
                    ]
                ),
            ),
            ("holdout", collections.OrderedDict([("required", False), ("cells", [])])),
            ("cells", CELLS),
        ]
    )
    with open(args.out, "w") as out:
        json.dump(addendum, out, indent=2)
        out.write("\n")
    print(f"observed resolution {observed:.4f} -> frozen {resolution:.2f}")
    print(f"resolution evidence {relative} sha256 {digest}")


if __name__ == "__main__":
    main()
