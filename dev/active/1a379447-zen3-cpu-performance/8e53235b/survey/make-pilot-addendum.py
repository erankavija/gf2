#!/usr/bin/env python3
"""Write the frozen pilot addendum of the codeword-assembly family (jit:8e53235b).

Usage: make-pilot-addendum.py, with no arguments

Writes `addendum-v4-codeword-assembly-pilot.json` in the directory above this
script. The file is a frozen declaration; the confirmation stage derives its
own addendum from the committed pilot receipt with the repository's canonical
freezer.
"""

import json
import sys

from locate import HERE, ISSUE, ROOT, repository_files

FROZEN_UTC = "2026-10-10T22:15:00Z"
FAMILY = "8e53235b-bch-codeword-assembly"
OUTPUT = HERE.parent / "addendum-v4-codeword-assembly-pilot.json"
LEDGER_NAME = "codeword-assembly-family-ledger.jsonl"
WORKLOAD = "bch-encode-batch"

# (cell, objective, size, seed)
CELLS = [
    ("assembly-bch-encode-current-m14-b256-1core", "improvement",
     {"degree": 14, "batch": 256}, 8101),
    ("assembly-bch-encode-current-m16-b256-1core", "non-regression",
     {"degree": 16, "batch": 256}, 8102),
    ("assembly-bch-encode-fold-m14-b256-1core", "non-regression",
     {"degree": 14, "batch": 256}, 8103),
    ("assembly-bch-encode-bitslice-m14-b256-1core", "non-regression",
     {"degree": 14, "batch": 256}, 8104),
    ("assembly-bch-encode-current-m8-b1-1core", "non-regression",
     {"degree": 8, "batch": 1}, 8105),
]

FAMILY_DESCRIPTION = (
    f"Exploratory pilot of issue {ISSUE}, protocol version 4: this addendum is the pilot and "
    "every cell is exploratory. Question: does placing each systematic codeword of a packed "
    "binary BCH batch encode by a word copy of the message and a shifted placement of the "
    "parity, in place of one bit write per coordinate, make the whole batch consumer faster? "
    "Both arms are the one executable consumer-arm of the bit-storage consumer harness, built "
    "once from one tree; the environment selects the route. Both arms call "
    "BchCode::encode_batch_into or, in a family-pinned cell, encode_batch_family_into, on the "
    "same messages with a workspace and an output the caller prepared, so message packing, the "
    "reduction, the codeword assembly and the output are inside the timed call. The baseline "
    "workspace keeps CodewordAssembly::BitSerial, the assembly every encode selects, which is "
    "the pinned pre-change consumer; the candidate workspace selects "
    "CodewordAssembly::WordWise. The pair differs in that selection alone. The harness's "
    "consumer-verify requires every family under both assemblies to write the reference "
    "family's bits. The declared cells reach the mother code of the DVB-T2 short frame, "
    "GF(2^14) of mother-code length 16383 and designed distance 25; the mother code of the "
    "DVB-T2 normal frame, GF(2^16) of mother-code length 65535 and designed distance 25; and "
    "the degree-8 mother code of corpus row B3, GF(2^8) of mother-code length 255 and designed "
    "distance 9. Cells: the deciding cell is the 256-message batch on the short-frame mother "
    "code through the family the production dispatcher selects, the consumer the 04b85d10 "
    "profile ranks, with the improvement objective. Four non-regression cells guard it: the "
    "same batch on the normal-frame mother code, where the codeword is four times as long; "
    "the short-frame batch with the carry-less-multiply fold family pinned and with the "
    "bit-sliced interleaved family pinned, the registered families a profile can admit in "
    "place of the reference; and one message on the degree-8 mother code, "
    "where the assembly has the least to place and a fixed per-call cost shows most. Every "
    "cell is single-core and warm: the assembly runs on the calling thread and the batch's "
    "buffers are cache-resident between calls. Frozen confirmation selection: a first "
    "confirmatory attempt spends alpha/2 = 0.025, and P-20's tail-support bound, "
    "10000 * (0.025 / m) / 2 >= 20, admits m = 6 confirmatory comparisons, so the "
    "confirmation retains all five cells. Frozen resolution rule: the canonical freezer's, "
    "the widest relative bootstrap half-width over every pilot cell, rounded up to two "
    "decimals. Frozen decision rule: the word-wise assembly is adopted as the one assembly of "
    "the packed kernel when the confirmation's receipt is accepted and every confirmatory "
    "cell records pass, which is improvement on the deciding cell and non-regression on each "
    "guard; on any other recorded outcome the bit-serial assembly is retained and the "
    "candidate is removed. A pilot whose deciding cell places the candidate behind the "
    "baseline is a recorded no-win without a confirmation. The rule reads the acceptance "
    "summary's recorded outcomes and nothing else."
)

WORTHWHILE_RATIONALE = (
    "The bar of the bit-storage layout consumer family of issue 04b85d10, held for a change "
    "measured on the same whole consumer: ten percent of the whole batch encode, packing and "
    "output included, is the smallest gain worth a new bit-run primitive in the bit-storage "
    "crate and a second statement of the layout mapping in the packed kernel."
)

EQUIVALENCE_RATIONALE = (
    "A guard cell records the candidate as not worse when it is at most five percent slower "
    "than the bit-serial assembly on the whole consumer, the equivalence margin of the "
    "bit-storage layout consumer family of issue 04b85d10 for the same consumer. The "
    "confirmation freezes a pilot-derived resolution strictly below this margin."
)

MATERIAL_GAP_RATIONALE = (
    "No cell is a comparator-gap cell: every cell compares two assemblies of one gf2 encode, "
    "so no material-gap threshold applies."
)

MAINTENANCE_RATIONALE = (
    "The adopted state is one safe bit-run write on BitVec in gf2-core and one assembly "
    "function in the packed BCH kernel of gf2-coding that replaces the bit-serial one, within "
    "120 added nonblank production lines, with no unsafe kernel, no selector, no dependency "
    "and no second entry point. The CodewordAssembly selection and its workspace setter exist "
    "for the measurement and either outcome removes them."
)


def cell(cell_id, objective, size, seed):
    return {
        "cell_id": cell_id,
        "objective": objective,
        "role": "exploratory",
        "workload": {"identity": WORKLOAD, "size": size, "seed": seed},
        "metric_kind": "whole-consumer",
        "scaling": "single-core-latency",
        "core_arm": "single-core",
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": "warm",
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": True,
        "decoder": None,
    }


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    addendum = {
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
            "worthwhile_speedup": 1.1,
            "rationale": WORTHWHILE_RATIONALE,
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": 1.05,
            "equivalence_rationale": EQUIVALENCE_RATIONALE,
            "material_gap_threshold": None,
            "material_gap_rationale": MATERIAL_GAP_RATIONALE,
        },
        "complexity_budget": {
            "max_new_unsafe_kernels": 0,
            "max_added_source_lines": 120,
            "maintenance_rationale": MAINTENANCE_RATIONALE,
        },
        "family_wise": {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": repository_files.live_file(ROOT, LEDGER_NAME),
        },
        "search_budget": {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        },
        "holdout": {"required": False, "cells": []},
        "cells": [cell(*declared) for declared in CELLS],
    }
    OUTPUT.write_text(json.dumps(addendum, indent=2) + "\n")
    print(f"{OUTPUT.relative_to(ROOT)}: {len(addendum['cells'])} exploratory cells")


if __name__ == "__main__":
    main()
