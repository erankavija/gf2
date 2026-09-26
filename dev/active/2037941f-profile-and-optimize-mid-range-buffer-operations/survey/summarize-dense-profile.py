#!/usr/bin/env python3
"""Render dense baseline profile counters with intervals over repeated passes."""

import json
from pathlib import Path
import re
import statistics
import sys


def counters(path):
    values = {}
    for line in path.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        fields = line.split(",")
        if len(fields) < 3:
            continue
        value, event = fields[0], fields[2].split(":")[0]
        if not value.startswith("<"):
            values[event] = float(value)
    return values


def result(path):
    text = path.read_text().strip()
    prefix = "GF2_TUNING_RESULT="
    if not text.startswith(prefix):
        raise ValueError(f"{path} has no arm result")
    return json.loads(text[len(prefix) :])


REPORT_ROW = re.compile(
    r"^\s*[0-9.]+%\s+([0-9,]+)\s+(\S+)\s+\[[^]]+\]\s+(.+?)\s*$"
)


def samples(path):
    counts = {}
    for line in path.read_text().splitlines():
        match = REPORT_ROW.match(line)
        if match:
            key = f"{match.group(2)}: {match.group(3)}"
            counts[key] = counts.get(key, 0) + int(match.group(1).replace(",", ""))
    if not counts:
        raise ValueError(f"{path}: no sampled symbols")
    return counts


def interval(values):
    if len(values) != 9:
        raise ValueError(f"expected nine profile repetitions, got {len(values)}")
    ordered = sorted(values)
    return f"{statistics.median(ordered):.2f} [{ordered[1]:.2f}, {ordered[7]:.2f}]"


def main(root):
    log = (root / "execution.log").read_text()
    done = {}
    for line in log.splitlines():
        if line.startswith("cell ") and " done " in line:
            fields = line.split()
            done[fields[1]] = int(fields[-1].removeprefix("attempt="))
    if not done:
        raise ValueError("the profile has no completed cell")
    print("# Dense baseline profile attribution")
    print()
    print(
        "Each bracket is the 96.1% order-statistic interval for the median of "
        "nine full process passes. Counters include setup, calibration, and "
        "output release, divided by the arm's observed timed call count; "
        "the campaign receipt remains the authority for operation latency."
    )
    print()
    print("| Frozen cell | cycles/call | instructions/call | L1 misses/call | cache misses/call |")
    print("|---|---:|---:|---:|---:|")
    symbol_tables = {}
    for cell, number in done.items():
        attempt = root / cell / f"attempt-{number}"
        series = {
            key: []
            for key in ("cycles", "instructions", "L1-dcache-load-misses", "cache-misses")
        }
        for rep in range(1, 10):
            for kind in ("issue", "memory"):
                arm = result(attempt / f"{kind}-{rep}.result")
                calls = sum(window["calls"] for window in arm["windows"])
                if calls <= 0:
                    raise ValueError(f"{cell} {kind}-{rep}: zero calls")
                for event, count in counters(attempt / f"{kind}-{rep}.csv").items():
                    if event in series:
                        series[event].append(count / calls)
        rendered = [interval(series[event]) if series[event] else "unavailable"
                    for event in series]
        print(f"| `{cell}` | " + " | ".join(rendered) + " |")
        records = [samples(attempt / f"cycles-report-{rep}.txt") for rep in range(1, 10)]
        totals = [sum(record.values()) for record in records]
        symbols = sorted({name for record in records for name in record})
        symbol_tables[cell] = (totals, [
            (
                name,
                [record.get(name, 0) for record in records],
                [100 * record.get(name, 0) / total
                 for record, total in zip(records, totals)],
            )
            for name in symbols
        ])
    print()
    print("## Sampled cycle attribution")
    print()
    print(
        "Each symbol share and count is the median and order-statistic interval "
        "over nine independent cycle-sampling passes. The table retains symbols "
        "whose median sampled share reaches 1%; every report and annotation "
        "remains beside the table."
    )
    print()
    print("| Frozen cell | symbol | share (%) | samples/pass | total samples/pass |")
    print("|---|---|---:|---:|---:|")
    for cell, (totals, rows) in symbol_tables.items():
        for name, counts, shares in sorted(
            rows, key=lambda row: (-statistics.median(row[2]), row[0])
        ):
            if statistics.median(shares) < 1:
                continue
            print(
                f"| `{cell}` | `{name}` | {interval(shares)} | "
                f"{interval(counts)} | {interval(totals)} |"
            )
    print()
    print(
        "Raw counter passes, sampled symbol reports, commands, host facts, and "
        "annotated instruction samples are retained beside this table."
    )


if __name__ == "__main__":
    main(Path(sys.argv[1]))
