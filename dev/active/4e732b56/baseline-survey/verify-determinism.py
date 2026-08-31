#!/usr/bin/env python3
"""Check seeded digest reproduction for a small survey rerun (jit:4e732b56)."""

from __future__ import annotations

import argparse
import csv
import pathlib
import sys


def rows(path: pathlib.Path, workload: str) -> dict[tuple[str, str, str, str, str], str]:
    """Return digest by observable cell/trial identity for one workload."""
    with path.open(newline="") as fh:
        result = {}
        for row in csv.DictReader(fh):
            if row["workload"] != workload or row["code"] not in {"B1", "B2"}:
                continue
            key = (row["workload"], row["algorithm"], row["code"], row["batch"], row["trial"])
            result[key] = row["digest"]
        return result


def compare(original: pathlib.Path, rerun: pathlib.Path, workload: str) -> tuple[int, list[str]]:
    old = rows(original, workload)
    new = rows(rerun, workload)
    lines = []
    failures = 0
    for key in sorted(set(old) | set(new)):
        old_digest = old.get(key, "MISSING")
        new_digest = new.get(key, "MISSING")
        match = old_digest == new_digest
        lines.append(
            f"{key[0]} {key[1]} {key[2]} batch={key[3]} trial={key[4]} "
            f"original={old_digest} rerun={new_digest} match={match}"
        )
        failures += not match
    return failures, lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--original", type=pathlib.Path, required=True)
    ap.add_argument("--w1-rerun", type=pathlib.Path, required=True)
    ap.add_argument("--w2-rerun", type=pathlib.Path, required=True)
    ap.add_argument("--w1-command", required=True)
    ap.add_argument("--w2-command", required=True)
    ap.add_argument("--w1-captured", required=True)
    ap.add_argument("--w2-captured", required=True)
    args = ap.parse_args()

    print("# seeded determinism rerun for survey digest agreement")
    print(f"# original CSV: {args.original}")
    print(f"# W1 rerun captured: {args.w1_captured}")
    print(f"# W1 command: {args.w1_command}")
    print(f"# W2 rerun captured: {args.w2_captured}")
    print(f"# W2 command: {args.w2_command}")
    print("# filter: AFF3CT B1/B2, all emitted W1 cells and W2 materialization cells")
    print("# seed: 0x00000000ae03bcd0")

    failures = 0
    for workload, rerun in (("W1", args.w1_rerun), ("W2", args.w2_rerun)):
        count, lines = compare(args.original, rerun, workload)
        failures += count
        print(f"## {workload} digest comparisons")
        for line in lines:
            print(line)
        print(f"{workload}: comparisons={len(lines)} mismatches={count}")

    if failures:
        print(f"determinism check failed: {failures} mismatches", file=sys.stderr)
        return 1
    print("all seeded rerun digests match the original CSVs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
