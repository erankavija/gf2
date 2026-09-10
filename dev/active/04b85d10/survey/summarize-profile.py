#!/usr/bin/env python3
"""Renders the sweep records of the bit-storage consumer profile as Markdown.

Reads the JSON lines `consumer-profile` wrote and projects them into one table
per consumer family. Every figure in the table comes from the record it sits
beside; nothing is carried in from another run.

Usage: summarize-profile.py <profile.jsonl>
"""

import json
import sys
from collections import OrderedDict

FAMILY_OF = {
    "row-xor": "logical",
    "dense-rref": "logical",
    "dense-matvec": "logical",
    "ldpc-syndrome": "logical",
    "popcount": "count",
    "zero-test": "count",
    "ldpc-codeword-check": "count",
    "transpose-64x64": "layout",
    "dense-transpose": "layout",
    "bch-encode-batch": "layout",
    "bch-encode-batch-alloc": "layout",
    "bch-encode-batch-parallel": "layout",
    "dvb-bch-encode": "layout",
    "field-id-hint": "layout",
}

FAMILY_TITLE = OrderedDict(
    [
        ("logical", "Logical row, parity and dense-matrix consumers"),
        ("count", "Count and fused-reduction consumers"),
        ("layout", "Transpose, bitslice and BCH encoding consumers"),
    ]
)


def size_text(size):
    return " ".join(f"{key}={value}" for key, value in sorted(size.items()))


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: summarize-profile.py <profile.jsonl>")
    records = []
    with open(sys.argv[1], encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                records.append(json.loads(line))

    print("# Current-code consumer profile sweep")
    print()
    print(
        "Projection of `profile.jsonl`, one row per measured route. "
        f"The run recorded {len(records)} rows."
    )
    print()
    print(
        "`ns/call` is the wall-clock mean over the calls the row reports, a "
        "single-run figure with no interval: the sweep locates the expensive "
        "routes and the protocol receipts carry the intervals."
    )

    for family, title in FAMILY_TITLE.items():
        rows = [
            record
            for record in records
            if FAMILY_OF.get(record["workload"]) == family
        ]
        if not rows:
            continue
        print()
        print(f"## {title}")
        print()
        print(
            "| Workload | Size | Route | Observed path | Calls | ns/call | "
            "Allocations/call | Bytes/call |"
        )
        print("|---|---|---|---|---:|---:|---:|---:|")
        for record in rows:
            timed = record["timed"]
            print(
                "| {workload} | {size} | {path} | {selected} | {calls} | "
                "{ns:.1f} | {allocations:.2f} | {bytes:.0f} |".format(
                    workload=record["workload"],
                    size=size_text(record["size"]),
                    path=record["path"],
                    selected=record["selected_path"],
                    calls=record["calls"],
                    ns=record["ns_per_call"],
                    allocations=timed["allocations_per_call"],
                    bytes=timed["allocated_bytes_per_call"],
                )
            )

    conversions = [record for record in records if record.get("conversion")]
    if conversions:
        print()
        print("## Reported setup and conversion costs")
        print()
        print(
            "Whole-consumer routes report the phases the receipt schema "
            "carries. `setup` is one-shot preparation the timed body does not "
            "repeat; the remaining columns are per timed call. A zero states "
            "that the route performs no such work."
        )
        print()
        print(
            "| Workload | Size | Route | setup ns | pack ns | unpack ns | "
            "batch fill ns | dispatch ns |"
        )
        print("|---|---|---|---:|---:|---:|---:|---:|")
        for record in conversions:
            conversion = record["conversion"]
            print(
                "| {workload} | {size} | {path} | {setup} | {pack} | "
                "{unpack} | {fill} | {dispatch} |".format(
                    workload=record["workload"],
                    size=size_text(record["size"]),
                    path=record["path"],
                    setup=conversion["setup_ns"],
                    pack=conversion["pack_ns"],
                    unpack=conversion["unpack_ns"],
                    fill=conversion["batch_fill_ns"],
                    dispatch=conversion["dispatch_ns"],
                )
            )


if __name__ == "__main__":
    main()
