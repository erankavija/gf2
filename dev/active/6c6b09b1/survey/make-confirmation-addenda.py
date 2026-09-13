#!/usr/bin/env python3
"""Freeze the three confirmation addenda of the byte-field survey (jit:6c6b09b1).

Usage: make-confirmation-addenda.py FROZEN_UTC [--survey-analysis PATH]

Each family's confirmation is frozen from that family's committed pilot
receipt and from nothing else, so a rerun against the same committed inputs
writes the same bytes. What the freeze takes from the pilot:

* the measurement resolution, `survey-analysis resolution-freeze` over the
  pilot receipt: its widest relative bootstrap half-width at the corrected
  alpha its own frozen addendum and ledger imply, rounded up to six decimal
  places. P-03 recomputes that half-width and rejects a declared resolution
  below it;
* the content pin of the pilot receipt that observed it;
* the cells, copied by identifier from the pilot addendum with the role
  changed to `confirmatory`, so a confirmed cell measures the workload,
  size, seed, cache state, metric kind and arms its pilot measured.

The margins are the family's own, declared before any trial and unchanged
from the pilot: an equivalence margin of 1.15 and a material-gap threshold of
1.25. Both strictly exceed one plus every frozen resolution, which the
addendum validation requires.

Which pilot: the committed pilot receipts of this family under
`dev/bench_results/6c6b09b1/`, excluding any campaign started from the
addendum this run writes. The exclusion is what keeps the freeze reproducible
after its own confirmation is committed beside the pilot; the receipt label
already excludes a confirmation, and the addendum-path test also excludes a
pilot-labelled campaign relaunched from the confirmation addendum itself.
Exactly one pilot must remain, so a second pilot of the same family is an
error rather than a silent choice.

Cell count: P-20 gives each confirmatory cell at least twenty expected
bootstrap draws per tail. With `bootstrap_resamples` 10000 that needs a
corrected alpha of at least 0.004, and these families spend
alpha / (t (t+1)) / m = 0.025 / m: every reservation each ledger carries so
far is exploratory, so this confirmation is attempt t = 1 and m is its own
confirmatory cell count. Seven cells would fall to 0.003571 and report every
cell `not-confirmatory`, so each family confirms six cells and leaves the
rest of its pilot breadth as exploratory evidence. `CELLS` below names the
six per family and the paragraph beside it states what they cover.
"""

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ISSUE = "6c6b09b1"
SURVEY = Path(f"dev/active/{ISSUE}/survey")
RESULTS = Path(f"dev/bench_results/{ISSUE}")
DEFAULT_ANALYSIS = f"target/{ISSUE}-analysis/release/survey-analysis"

# The six confirmatory cells of each family, and the paragraph that states
# what the six cover and what stays exploratory. The full pilot cell set
# answers each question across more regimes than one attempt's alpha budget
# can confirm; these are the cells the downstream feasibility issue 19513245
# needs at confirmatory strength.
CELLS = {
    "region-axpy": (
        [
            "axpy-4k-element-vs-isal",
            "axpy-128k-element-vs-isal",
            "axpy-8m-element-vs-isal",
            "axpy-128k-wide-vs-isal",
            "axpy-128k-whole-element-vs-isal",
            "axpy-128k-whole-wide-vs-isal",
        ],
        "The six confirmed cells sweep the warm cache regimes 4 KiB, 128 KiB and 8 MiB for "
        "FieldVec<Gf2mElement> kernel-isolated against ISA-L, the fastest external region "
        "kernel this family's pilot measured, and take 128 KiB as the anchor at which both "
        "gf2 representations are confirmed kernel-isolated and as whole byte-region "
        "consumers. The pilot's streaming regime, its 4 KiB and 8 MiB Gf2mWide cells, its "
        "4 KiB whole-consumer cells and its external-against-external cells keep their "
        "exploratory evidence and are not confirmed here.",
    ),
    "matrix-product": (
        [
            "matmul-n256-element-vs-m4rie",
            "matmul-n256-wide-vs-m4rie",
            "matmul-n256-whole-element-vs-m4rie",
            "matmul-n256-whole-wide-vs-m4rie",
            "encode-k10r4-64k-element-vs-isal",
            "encode-k10r4-64k-wide-vs-isal",
        ],
        "The six confirmed cells take the square dimension 256 as the anchor shape and "
        "confirm both gf2 representations there, kernel-isolated and as whole byte-matrix "
        "consumers, against M4RIE; at the generator-encode shape they confirm both "
        "representations kernel-isolated against ISA-L, which the pilot measures as far "
        "ahead of M4RIE at that shape. The pilot's dimensions 64 and 512, its whole-consumer "
        "encode cells and its external-against-external cell keep their exploratory evidence "
        "and are not confirmed here.",
    ),
    "pairwise-control": (
        [
            "pairwise-4k-batch-vs-m4rie",
            "pairwise-128k-batch-vs-m4rie",
            "pairwise-128k-whole-batch-vs-m4rie",
            "pairwise-4k-batch-vs-gfcomplete",
            "pairwise-128k-batch-vs-gfcomplete",
            "pairwise-128k-whole-batch-vs-gfcomplete",
        ],
        "The six confirmed cells are the two fastest external per-byte multiplies this "
        "family's pilot measured, M4RIE and GF-Complete, each at 4 KiB and 128 KiB "
        "kernel-isolated and at 128 KiB as a whole byte-region consumer. ISA-L's per-byte "
        "multiply, the slowest of the three in the pilot, keeps its exploratory evidence and "
        "is not confirmed here.",
    ),
}


def load_pilot_design():
    """Loads the pilot generator, the single source of the family prose."""
    spec = importlib.util.spec_from_file_location("make_addenda", SURVEY / "make-addenda.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def pilot_receipt(family_id, addendum_path):
    """Finds this family's one committed pilot receipt.

    Skips a campaign started from `addendum_path`, so the freeze reads the
    same pilot before and after its own confirmation is committed.
    """
    found = []
    for receipt_path in sorted(RESULTS.glob("*/receipt.json")):
        receipt = json.loads(receipt_path.read_bytes())
        if receipt.get("label") != "pilot" or receipt.get("family_id") != family_id:
            continue
        if receipt.get("addendum", {}).get("path") == addendum_path:
            continue
        found.append(receipt_path)
    if len(found) != 1:
        raise SystemExit(
            f"{family_id}: expected one committed pilot receipt, found "
            + (", ".join(str(path) for path in found) or "none")
        )
    return found[0]


def frozen_resolution(analysis, receipt_path):
    """Reads the pilot's widest relative half-width, rounded up to freeze."""
    output = subprocess.run(
        [analysis, "resolution-freeze", str(receipt_path.parent)],
        capture_output=True,
        check=True,
        text=True,
    ).stdout.strip()
    return json.loads(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen_utc")
    parser.add_argument("--survey-analysis", default=DEFAULT_ANALYSIS)
    arguments = parser.parse_args()
    design = load_pilot_design()

    for short, (family_id, question, cells) in design.FAMILIES.items():
        path = f"dev/active/{ISSUE}/addendum-v4-{short}-confirmation.json"
        chosen, coverage = CELLS[short]
        pilot_path = pilot_receipt(family_id, path)
        pilot = json.loads(pilot_path.read_bytes())
        pilot_addendum = json.loads(Path(pilot_path.parent, pilot["addendum"]["snapshot"]).read_bytes())
        by_id = {cell["cell_id"]: cell for cell in pilot_addendum["cells"]}
        missing = [cell_id for cell_id in chosen if cell_id not in by_id]
        if missing:
            raise SystemExit(f"{family_id}: the pilot declares no cell {', '.join(missing)}")
        confirmatory = [dict(by_id[cell_id], role="confirmatory") for cell_id in chosen]

        effect = dict(design.EFFECT)
        effect["measurement_resolution"] = frozen_resolution(
            arguments.survey_analysis, pilot_path
        )
        effect["resolution_evidence"] = {
            "receipt": str(pilot_path),
            "sha256": hashlib.sha256(pilot_path.read_bytes()).hexdigest(),
        }
        addendum = {
            "schema": "zen3-benchmark-addendum-v4",
            "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
            "family": {
                "id": family_id,
                "issue": ISSUE,
                "purpose": "consumer-family",
                "description": (
                    f"Confirmation of issue {ISSUE}, protocol version 4, frozen from the "
                    f"pilot receipt {pilot_path.parent} before any confirmatory trial. "
                    + question
                    + coverage
                    + " "
                    + design.FIELD
                    + " "
                    + design.ARMS
                    + " "
                    + design.RNG
                    + " "
                    + design.SCOPE
                ),
            },
            "frozen": {"frozen_utc": arguments.frozen_utc},
            "effect": effect,
            "complexity_budget": design.BUDGET,
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": f"{RESULTS}/{design.LEDGER_PREFIX}-{short}-family-ledger.jsonl",
            },
            "search_budget": {
                "max_pilot_trials_per_cell": 2,
                "max_confirmatory_attempts_per_candidate": 1,
            },
            "holdout": {"required": False, "cells": []},
            "cells": confirmatory,
        }
        with open(path, "w") as output:
            json.dump(addendum, output, indent=2)
            output.write("\n")
        print(
            f"{path}: {len(confirmatory)} confirmatory cells, resolution "
            f"{effect['measurement_resolution']} from {pilot_path}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
