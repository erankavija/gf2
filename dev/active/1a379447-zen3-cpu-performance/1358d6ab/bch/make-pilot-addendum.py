#!/usr/bin/env python3
"""Write the frozen pilot addendum of family 1362381c-bch-integration (jit:1358d6ab).

Usage: make-pilot-addendum.py [--check]

Writes `pilot/addendum.json` beside this script. The confirmation stage derives
its own addendum from the committed pilot receipt with the repository's
canonical freezer; `CONFIRMATION_DESCRIPTION` is the family prose it passes.
`--check` writes nothing and exits non-zero when the committed addendum differs.
"""

import json
import sys
from pathlib import Path

from locate import HERE, ISSUE, ROOT, repository_files

FROZEN_UTC = "2026-10-26T22:42:13Z"
FAMILY = "1362381c-bch-integration"
OUTPUT = HERE / "pilot" / "addendum.json"
# The family's results sit beside the other issues' result directories.
RESULTS = Path(repository_files.live_file(ROOT, "run-codeword-assembly.sh")).parent.parent / ISSUE
LEDGER = (RESULTS / "bch-integration" / "ledger.jsonl").as_posix()

# Code segment of a cell identifier: workload identity and its code size.
CODES = {
    "m14": ("bch-encode-batch", {"degree": 14}),
    "t2s": ("dvb-bch-encode", {"n": 7200}),
    "t2n": ("dvb-bch-encode", {"n": 32400}),
}
# (code, batch, external engine, seed); the identifier is `<code>-b<batch>-<engine>`.
CELLS = [
    ("m14", 1, "aff3ct-lfsr-scalar", 13581),
    ("m14", 256, "aff3ct-lfsr-simd-inter", 13582),
    ("t2s", 1, "bchlib-table-remainder", 13583),
    ("t2s", 256, "bchlib-table-remainder", 13584),
    ("t2n", 1, "bchlib-table-remainder", 13585),
    ("t2n", 256, "bchlib-table-remainder", 13586),
]

COMMON = (
    "gf2's production binary BCH encode against the external encoders of the 4e732b56 survey, "
    "AFF3CT [Cassagne2019] and bchlib [Bchlib2026], on the code classes whose encoder the "
    "8e53235b word-wise codeword assembly changed: the GF(2^14) mother code of the DVB-T2 short "
    "frame (length 16383, designed distance 25) and the DVB-T2 rate-1/2 outer BCH codes of the "
    "short frame (7200, 7032) and the normal frame (32400, 32208) [Etsi2015]. "
    "The baseline arm is consumer-arm of the bit-storage consumer harness, a conservative-portable "
    "release build of the current tree, on the route the production dispatcher selects: "
    "BchCode::encode_batch_into on the mother code, and the DVB-T2 code's encode per message of the "
    "batch on the shortened codes. The candidate arm is bch-external-arm, whose environment "
    "selects one external encoder: AFF3CT's Encoder_BCH (one frame per LFSR call) or "
    "Encoder_BCH_inter (one SIMD wave of frames per call), or bchlib's bch_encode, the Linux "
    "kernel's table-driven remainder; the external sources are compiled with the survey's "
    "-O3 -march=native flags, so the candidate build is external. bchlib takes whole message "
    "bytes and does not serve the mother code, whose dimension 16215 is no whole number of "
    "bytes; on the shortened codes the survey records bchlib ahead of both AFF3CT encoders, so "
    "each cell pairs gf2 with the strongest external encoder that serves its code. AFF3CT's "
    "interleaved encoder serves whole waves only, so the one-message cell takes its scalar "
    "encoder. "
    "Both arms of a cell encode the same messages, BitVec::random_seeded(k, seed + i + 1) for "
    "message i of the cell's batch, and each external dispatch requires, before timing, its "
    "codewords mapped into gf2's user coordinates to equal gf2's bit for bit. A timed call is "
    "each arm's batch encode on messages already in its library's representation (packed "
    "words for gf2, one int per bit for AFF3CT, packed bytes for bchlib) into codewords in that "
    "representation; the external arm reports its conversions from and to packed BitVec as "
    "pack and unpack costs measured outside the timed call. The batch sizes are the one-message "
    "and 256-message entries of the fd9d5416 W1 table. Every cell is single-core and warm: "
    "neither external encoder exposes workers. A comparator-gap outcome improved records a "
    "material gap in the external encoder's favour. The family's fresh ledger admits six "
    "confirmatory cells on its first attempt under P-20, the cell count declared here."
)
FAMILY_DESCRIPTION = (
    "Exploratory pilot of the integrated comparison of " + COMMON +
    " This pilot observes the measurement resolution that the family's confirmation freezes; "
    "it decides nothing."
)
CONFIRMATION_DESCRIPTION = (
    "Confirmation of the integrated comparison of " + COMMON +
    " The measurement resolution is frozen from the family's pilot receipt with the margins "
    "this family fixed before its pilot; every cell is confirmatory and every outcome is "
    "reported as evaluated."
)

EQUIVALENCE_RATIONALE = (
    "Within ten percent of a batch encode, a consumer choosing between gf2 and an external BCH "
    "encoder gains too little to repay a second encoder dependency, so the family reads the "
    "arms as equivalent. Fixed before this pilot; the canonical freezer stops when pilot "
    "resolution reaches it and never widens it from results."
)
MATERIAL_GAP_RATIONALE = (
    "A gap of a quarter of a consumer's batch encode time outweighs the integration and "
    "maintenance cost of a second encoder dependency, so it changes which encoder a consumer "
    "deploys and needs attribution; it lies outside the equivalence margin. Fixed before this "
    "pilot; the canonical freezer stops when pilot resolution reaches it and never widens it "
    "from results."
)


def cell(code, batch, engine, seed):
    workload, size = CODES[code]
    return {
        "cell_id": f"{code}-b{batch}-{engine}",
        "objective": "comparator-gap",
        "role": "exploratory",
        "workload": {"identity": workload, "size": {**size, "batch": batch}, "seed": seed},
        "metric_kind": "kernel-isolated",
        "scaling": "single-core-latency" if batch == 1 else "sustained-throughput",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "conservative-portable", "candidate": "external"},
        "conversion_costs_included": False,
        "decoder": None,
    }


def addendum():
    return {
        "schema": "zen3-benchmark-addendum-v4",
        "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
        "family": {
            "id": FAMILY,
            "issue": ISSUE,
            "purpose": "consumer-family",
            "description": FAMILY_DESCRIPTION,
        },
        "frozen": {"frozen_utc": FROZEN_UTC},
        "effect": {
            "worthwhile_speedup": None,
            "rationale": "The family publishes the comparator gap and adopts nothing, so it "
                         "declares no worthwhile speedup.",
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.1,
            "equivalence_rationale": EQUIVALENCE_RATIONALE,
            "material_gap_threshold": 1.25,
            "material_gap_rationale": MATERIAL_GAP_RATIONALE,
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": None,
            "maintenance_rationale": "The family selects nothing and adds no production code.",
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": LEDGER,
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": [cell(*declared) for declared in CELLS],
    }


def main():
    check = sys.argv[1:] == ["--check"]
    if sys.argv[1:] and not check:
        raise SystemExit(__doc__)
    text = json.dumps(addendum(), indent=2) + "\n"
    if check:
        if not OUTPUT.is_file() or OUTPUT.read_text() != text:
            raise SystemExit(f"{OUTPUT.relative_to(ROOT)} differs from its generator")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(text)
    print(f"{OUTPUT.relative_to(ROOT)}: {len(CELLS)} exploratory cells")


if __name__ == "__main__":
    main()
