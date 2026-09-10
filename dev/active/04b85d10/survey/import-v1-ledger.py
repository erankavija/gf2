#!/usr/bin/env python3
"""Genesis of the three protocol-v3 family ledgers (jit:04b85d10).

Protocol version 1 kept no family ledger. The one v1 campaign of this issue,
the exploratory pilot `pilot-04b85d10-20260908t085658z`, measured fifteen
exploratory cells under the single family `bit-storage-consumers-pilot` and
reserved zero comparisons. Version 3 asks one family question per consumer
group, so each v3 ledger opens with that pilot as its sequence-0 line: a
retrospective zero-comparison reservation that keeps the v1 lineage inside the
chain every v3 receipt pins. No confirmatory reservation exists to import.

The line format is the runner's `trial_ledger::Reservation`, and the values are
read from the committed v1 receipt rather than typed here. The script refuses
to overwrite a ledger that already has a line, so a chain in use is never
rewritten.

Usage: import-v1-ledger.py  (from the repository root)
"""

import hashlib
import json
import pathlib

V1_RECEIPT = pathlib.Path(
    "dev/bench_results/04b85d10/2026-09-08-04b85d10-consumers-pilot/receipt.json"
)
V1_ADDENDUM_SNAPSHOT = pathlib.Path(
    "dev/bench_results/04b85d10/2026-09-08-04b85d10-consumers-pilot/inputs/family-addendum.json"
)
FAMILIES = [
    "bit-storage-logical-consumers",
    "bit-storage-count-consumers",
    "bit-storage-layout-consumers",
]
LEDGER_DIR = pathlib.Path("dev/bench_results/04b85d10")
RECORD = pathlib.Path("dev/active/04b85d10/ledger-genesis.json")


def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    receipt = json.loads(V1_RECEIPT.read_text(encoding="utf-8"))
    addendum = json.loads(V1_ADDENDUM_SNAPSHOT.read_text(encoding="utf-8"))
    addendum_sha = sha256_file(V1_ADDENDUM_SNAPSHOT)
    if receipt["addendum"]["sha256"] != addendum_sha:
        raise SystemExit("the v1 receipt does not pin its own addendum snapshot")
    non_exploratory = [c for c in addendum["cells"] if c["role"] != "exploratory"]
    if non_exploratory:
        raise SystemExit("the v1 pilot reserved comparisons; this import handles none")

    entry = {
        "sequence": 0,
        "predecessor": "0" * 64,
        "family": None,
        "campaign": receipt["campaign_id"],
        "addendum_sha256": addendum_sha,
        "comparisons": 0,
        "protocol_version": addendum["protocol"]["version"],
        "candidates": [],
    }

    written = {}
    for family in FAMILIES:
        ledger = LEDGER_DIR / f"v3-{family}-family-ledger.jsonl"
        if ledger.exists() and ledger.read_bytes():
            raise SystemExit(f"{ledger} already holds a chain; refusing to rewrite it")
        line = dict(entry, family=family)
        text = json.dumps(line, separators=(",", ":")) + "\n"
        ledger.write_bytes(text.encode("utf-8"))
        written[family] = {
            "ledger_path": str(ledger),
            "line_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
            "entry": line,
        }

    record = {
        "schema": "bit-storage-v3-ledger-genesis-v1",
        "issue": "04b85d10",
        "purpose": (
            "Retrospective accounting of the protocol-v1 exploratory pilot in the "
            "three protocol-v3 family ledgers. Each line is a zero-comparison "
            "reservation, not a premeasurement reservation; no confirmatory "
            "attempt was ever reserved under version 1, so nothing is imported "
            "as spent."
        ),
        "source": {
            "receipt": str(V1_RECEIPT),
            "receipt_sha256": sha256_file(V1_RECEIPT),
            "addendum_snapshot": str(V1_ADDENDUM_SNAPSHOT),
            "addendum_sha256": addendum_sha,
            "v1_family": addendum["family"]["id"],
            "v1_cells": len(addendum["cells"]),
            "v1_non_exploratory_cells": len(non_exploratory),
        },
        "family_split": {
            "rationale": (
                "The v1 pilot pooled three consumer groups in one exploratory "
                "family. Each group is a distinct scientific question answered "
                "for a distinct downstream issue, so version 3 keeps one ledger "
                "per group. The split escapes no history: the pooled family "
                "reserved zero comparisons, and its pilot line opens every v3 "
                "chain."
            ),
            "bit-storage-logical-consumers": [
                "logical-row-xor-dispatch-1core",
                "logical-row-xor-threshold-1core",
                "logical-dense-rref-1core",
                "logical-ldpc-syndrome-1core",
            ],
            "bit-storage-count-consumers": [
                "count-popcount-dispatch-1core",
                "count-popcount-threshold-1core",
                "count-zero-test-syndrome-1core",
                "count-ldpc-check-1core",
            ],
            "bit-storage-layout-consumers": [
                "layout-transpose-block-1core",
                "layout-bch-encode-bitslice-1core",
                "layout-bch-encode-fold-1core",
                "layout-bch-encode-alloc-1core",
                "layout-transpose-block-6core",
                "layout-bch-encode-parallel-12core",
                "layout-bch-encode-parallel-24logical",
            ],
        },
        "ledgers": written,
    }
    RECORD.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    for family, info in written.items():
        print(f"{info['ledger_path']}: genesis line {info['line_sha256']}")


if __name__ == "__main__":
    main()
