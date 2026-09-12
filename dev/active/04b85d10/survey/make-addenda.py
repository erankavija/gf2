#!/usr/bin/env python3
"""Writes the protocol-v3 family addenda of the bit-storage survey (jit:04b85d10).

One family per consumer group, one pilot addendum per family, and, once a
pilot is published, one confirmatory addendum per family whose resolution is
the pilot's widest relative half-width. Cell declarations live here once so
the pilot and its confirmation cannot disagree on a workload, size, seed, core
arm or cache state; the confirmation drops the identity-control cells and
promotes the rest to `confirmatory`.

Usage:
  make-addenda.py pilot <frozen-utc>
  make-addenda.py confirmation <family> <frozen-utc> <pilot-receipt-dir> <resolution> \
      [<worthwhile> <equivalence> "<why the margins moved>"]

The optional margin arguments raise a family's confirmatory margins above the
values its pilot carried when the pilot-measured resolution requires it; the
reason is recorded verbatim in the addendum rationale.
"""

import hashlib
import json
import pathlib
import sys

ISSUE = "04b85d10"
OUT = pathlib.Path("dev/active/04b85d10")
LEDGER = "dev/bench_results/04b85d10/v3-bit-storage-{family}-consumers-family-ledger.jsonl"


def cell(cell_id, objective, identity, size, seed, *, metric, scaling="single-core-latency",
         core_arm="single-core", cache_state="warm", conversion=False):
    return {
        "cell_id": cell_id,
        "objective": objective,
        "role": "exploratory",
        "workload": {"identity": identity, "size": size, "seed": seed},
        "metric_kind": metric,
        "scaling": scaling,
        "core_arm": core_arm,
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cache_state,
        "builds": {"baseline": "conservative-portable", "candidate": "conservative-portable"},
        "conversion_costs_included": conversion,
        "decoder": None,
    }


FAMILIES = {
    "logical": {
        "id": "bit-storage-logical-consumers",
        "question": (
            "Whether the current packed-row XOR dispatch and its eight-word SIMD "
            "cutover leave a material single-core gap against routes gf2-core "
            "already exposes (a hoisted resolved kernel, the detected SIMD backend), "
            "at the row widths dense elimination and row-operation consumers use, "
            "and what the pinned dense RREF and sparse LDPC syndrome consumers cost "
            "today. Every arm is current production code; `ops-dispatched` is the "
            "route `BitMatrix::row_xor` takes."
        ),
        "worthwhile": 1.05,
        "worthwhile_rationale": (
            "A dispatch-hoist or cutover change is a small selector edit whose benefit "
            "reaches a row-operation consumer nearly one-to-one, because elimination "
            "loops spend their time in the row kernel itself. Five percent of an "
            "L1-resident row XOR is the smallest gain such a change can claim to move "
            "in a consumer; the mid-range buffer story it feeds admits repeatable "
            "smaller gains under a frozen rule rather than a blanket hurdle."
        ),
        "equivalence": 1.05,
        "equivalence_rationale": (
            "A route at most five percent slower than the dispatched production route "
            "is not worse for a row consumer whose kernel share is below one; the "
            "margin equals the worthwhile threshold so a cell is either material, "
            "not material, or a regression."
        ),
        "cells": [
            cell("logical-row-xor-dispatch-64w-1core", "improvement", "row-xor",
                 {"rows": 64, "words": 64}, 101, metric="kernel-isolated"),
            cell("logical-row-xor-dispatch-8w-1core", "improvement", "row-xor",
                 {"rows": 64, "words": 8}, 101, metric="kernel-isolated"),
            cell("logical-row-xor-threshold-4w-1core", "improvement", "row-xor",
                 {"rows": 64, "words": 4}, 101, metric="kernel-isolated"),
            cell("logical-row-xor-dispatch-8192w-1core", "improvement", "row-xor",
                 {"rows": 64, "words": 8192}, 101, metric="kernel-isolated"),
            cell("logical-dense-rref-1024-control-1core", "non-regression", "dense-rref",
                 {"rows": 1024, "cols": 1024}, 102, metric="whole-consumer", conversion=True),
            cell("logical-ldpc-syndrome-64800-control-1core", "non-regression", "ldpc-syndrome",
                 {"n": 64800}, 104, metric="whole-consumer", conversion=True),
        ],
    },
    "count": {
        "id": "bit-storage-count-consumers",
        "question": (
            "Where population counts and the fused reductions built on them cost "
            "material time in current consumers: the scalar/SIMD dispatch at the "
            "four- and eight-word boundaries, the SIMD gain on an L2-resident buffer, "
            "and whether an any-nonzero spelling (`find_first_one().is_none()`) "
            "replaces the full count materially in an isolated DVB-T2 syndrome "
            "buffer and in the whole `LdpcCode::is_valid_codeword` consumer. The "
            "fused AND-popcount consumer, dense matvec, is recorded as a pinned "
            "identity control because production exposes no alternative route."
        ),
        "worthwhile": 1.05,
        "worthwhile_rationale": (
            "A population-count route or an any-nonzero spelling is a one-line "
            "consumer change; five percent of the count is the smallest gain such a "
            "change can claim in a syndrome or weight consumer, and the whole-consumer "
            "cell decides whether the count is a material share at all."
        ),
        "equivalence": 1.05,
        "equivalence_rationale": (
            "A spelling at most five percent slower than the current count is not "
            "worse for a consumer whose count share is below one; the margin equals "
            "the worthwhile threshold."
        ),
        "cells": [
            cell("count-popcount-dispatch-4w-1core", "improvement", "popcount",
                 {"words": 4}, 201, metric="kernel-isolated"),
            cell("count-popcount-threshold-8w-1core", "improvement", "popcount",
                 {"words": 8}, 201, metric="kernel-isolated"),
            cell("count-popcount-bandwidth-65536w-1core", "improvement", "popcount",
                 {"words": 65536}, 201, metric="kernel-isolated"),
            cell("count-zero-test-507w-1core", "improvement", "zero-test",
                 {"words": 507, "set_bit": 32448}, 202, metric="kernel-isolated"),
            cell("count-ldpc-check-64800-1core", "improvement", "ldpc-codeword-check",
                 {"n": 64800}, 203, metric="whole-consumer", conversion=True),
            cell("count-dense-matvec-1024x4096-control-1core", "non-regression", "dense-matvec",
                 {"rows": 1024, "cols": 4096}, 103, metric="whole-consumer", conversion=True),
        ],
    },
    "layout": {
        "id": "bit-storage-layout-consumers",
        # "Normal-frame" below is wrong: the degree-14 BCH cells measure the
        # mother code of the DVB-T2 short frame (normal frame: GF(2^16)). The
        # text stays as both frozen layout addenda carry it, so this generator
        # still reproduces them; findings.md states the correction.
        "question": (
            "Whether the current bit-layout transform routes leave a material "
            "whole-consumer gap against routes the library already registers: the "
            "detected AVX2 64x64 block transpose against the portable primitive, "
            "the packed binary BCH batch entry point's current family selection "
            "against its registered bitslice and carry-less-multiply fold families "
            "on the DVB-T2 normal-frame mother code, and the allocating batch entry "
            "point against the caller-buffer one. The dense 4096-square transpose "
            "and the DVB-T2 compatibility BCH encoder are recorded as pinned "
            "identity controls because production exposes no alternative route."
        ),
        "worthwhile": 1.10,
        "worthwhile_rationale": (
            "A transform-route or encoding-family change carries selector, workspace "
            "and conversion code; ten percent of the whole consumer, conversion and "
            "output included, is the smallest gain worth that maintenance, and the "
            "measured candidates are expected far above it or plainly below it."
        ),
        "equivalence": 1.05,
        "equivalence_rationale": (
            "A route at most five percent slower than the current whole consumer is "
            "not worse; the allocation cell is expected to sit inside this margin, "
            "which records it as not material rather than as a regression."
        ),
        "cells": [
            cell("layout-transpose-block-256-1core", "improvement", "transpose-64x64",
                 {"blocks": 256}, 301, metric="kernel-isolated"),
            cell("layout-transpose-block-4096-6core", "improvement", "transpose-64x64",
                 {"blocks": 4096}, 301, metric="kernel-isolated",
                 scaling="sustained-throughput", core_arm="physical-cores-6",
                 cache_state="streaming"),
            cell("layout-bch-encode-bitslice-m14-b256-1core", "improvement", "bch-encode-batch",
                 {"degree": 14, "batch": 256}, 303, metric="whole-consumer", conversion=True),
            cell("layout-bch-encode-fold-m14-b256-1core", "improvement", "bch-encode-batch",
                 {"degree": 14, "batch": 256}, 303, metric="whole-consumer", conversion=True),
            cell("layout-bch-encode-caller-buffer-m14-b256-1core", "improvement",
                 "bch-encode-batch-alloc", {"degree": 14, "batch": 256}, 304,
                 metric="whole-consumer", conversion=True),
            cell("layout-dense-transpose-4096-control-1core", "non-regression", "dense-transpose",
                 {"rows": 4096, "cols": 4096}, 302, metric="whole-consumer", conversion=True),
            cell("layout-dvb-bch-encode-7200-control-1core", "non-regression", "dvb-bch-encode",
                 {"n": 7200, "batch": 1}, 306, metric="whole-consumer", conversion=True),
        ],
    },
}

COMMON = {
    "schema": "zen3-benchmark-addendum-v3",
    "protocol": {"id": "zen3-benchmark-protocol", "version": 3},
}

BUDGET = {
    "max_new_unsafe_kernels": 0,
    "max_added_source_lines": 0,
    "maintenance_rationale": (
        "This issue is a profile: it selects experiments and adopts no "
        "implementation, so its budget for production change is zero unsafe "
        "kernels and zero added production lines. A passing improvement cell "
        "confirms a candidate's materiality for its downstream issue; it "
        "selects nothing here."
    ),
}


def family_block(spec, mode):
    return {
        "id": spec["id"],
        "issue": ISSUE,
        "purpose": "consumer-family",
        "description": (
            f"{'Exploratory pilot' if mode == 'pilot' else 'Confirmatory family'} of "
            f"issue {ISSUE}, protocol version 3. Question: {spec['question']} "
            + (
                "Every cell is exploratory: the pilot sizes the family's measurement "
                "resolution at the confirmatory sample size, records the pinned "
                "pre-change baseline latencies, and decides nothing."
                if mode == "pilot"
                else
                "Every cell is confirmatory on fresh samples; identity controls stay in "
                "the pilot. A pass records a material gap between two current production "
                "routes; the issue adopts nothing and names the downstream issue that tests "
                "the candidate."
            )
        ),
    }


def write(path, document):
    text = json.dumps(document, indent=2) + "\n"
    path.write_text(text, encoding="utf-8")
    print(f"{path} sha256 {hashlib.sha256(text.encode('utf-8')).hexdigest()}")


def pilot(frozen_utc):
    for key, spec in FAMILIES.items():
        document = dict(COMMON)
        document["family"] = family_block(spec, "pilot")
        document["frozen"] = {"frozen_utc": frozen_utc}
        document["effect"] = {
            "worthwhile_speedup": spec["worthwhile"],
            "rationale": spec["worthwhile_rationale"]
            + " The pilot carries the margin so its recorded decisions are computed "
            "under the rule the confirmation uses; pilot cells never pass.",
            "measurement_resolution": None,
            "resolution_evidence": None,
            "equivalence_margin": spec["equivalence"],
            "equivalence_rationale": spec["equivalence_rationale"],
            "material_gap_threshold": None,
            "material_gap_rationale": (
                "No cell compares gf2 against an external comparator, so no "
                "comparator-gap objective is exercised and no threshold applies."
            ),
        }
        document["complexity_budget"] = BUDGET
        document["family_wise"] = {
            "alpha": 0.05,
            "prior_confirmatory_trials": 0,
            "prior_trials": [],
            "ledger_path": LEDGER.format(family=key),
        }
        document["search_budget"] = {
            "max_pilot_trials_per_cell": 1,
            "max_confirmatory_attempts_per_candidate": 1,
        }
        document["holdout"] = {"required": False, "cells": []}
        document["cells"] = [dict(c) for c in spec["cells"]]
        write(OUT / f"addendum-bit-storage-{key}-v3-pilot.json", document)


def confirmation(key, frozen_utc, pilot_dir, resolution, worthwhile=None, equivalence=None, moved=""):
    spec = dict(FAMILIES[key])
    resolution = float(resolution)
    if worthwhile is not None:
        spec["worthwhile"] = float(worthwhile)
        spec["equivalence"] = float(equivalence)
        spec["worthwhile_rationale"] += " " + moved
        spec["equivalence_rationale"] += " " + moved
    pilot_receipt = pathlib.Path(pilot_dir) / "receipt.json"
    digest = hashlib.sha256(pilot_receipt.read_bytes()).hexdigest()
    for margin in (spec["worthwhile"], spec["equivalence"]):
        if not margin > 1 + resolution:
            raise SystemExit(
                f"{key}: margin {margin} does not strictly exceed 1 + resolution {resolution}"
            )
    document = dict(COMMON)
    document["family"] = family_block(spec, "confirmation")
    document["frozen"] = {"frozen_utc": frozen_utc}
    document["effect"] = {
        "worthwhile_speedup": spec["worthwhile"],
        "rationale": spec["worthwhile_rationale"]
        + f" It strictly exceeds one plus the same-family pilot resolution of {resolution}.",
        "measurement_resolution": resolution,
        "resolution_evidence": {"receipt": str(pilot_receipt), "sha256": digest},
        "equivalence_margin": spec["equivalence"],
        "equivalence_rationale": spec["equivalence_rationale"]
        + f" It strictly exceeds one plus the pilot resolution of {resolution}.",
        "material_gap_threshold": None,
        "material_gap_rationale": (
            "No cell compares gf2 against an external comparator, so no "
            "comparator-gap objective is exercised and no threshold applies."
        ),
    }
    document["complexity_budget"] = BUDGET
    document["family_wise"] = {
        "alpha": 0.05,
        "prior_confirmatory_trials": 0,
        "prior_trials": [],
        "ledger_path": LEDGER.format(family=key),
    }
    document["search_budget"] = {
        "max_pilot_trials_per_cell": 1,
        "max_confirmatory_attempts_per_candidate": 1,
    }
    document["holdout"] = {"required": False, "cells": []}
    cells = []
    for declared in spec["cells"]:
        if declared["cell_id"].split("-")[-2] == "control":
            continue
        promoted = dict(declared)
        promoted["role"] = "confirmatory"
        cells.append(promoted)
    document["cells"] = cells
    write(OUT / f"addendum-bit-storage-{key}-v3-confirmation.json", document)


def main():
    if len(sys.argv) >= 3 and sys.argv[1] == "pilot" and len(sys.argv) == 3:
        pilot(sys.argv[2])
    elif len(sys.argv) in (6, 9) and sys.argv[1] == "confirmation":
        confirmation(*sys.argv[2:])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
