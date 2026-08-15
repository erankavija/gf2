#!/usr/bin/env python3
"""Aggregate the paired profiled run's per-dispatch counters per kernel.

Reads every ``pmc-*/*_counter_collection.csv.zst`` under the raw retention
directory (the per-dispatch rocprofv3 counter output of run
20260815T181923Z, zstd-compressed; see provenance.txt for the retention
location and the exact producing commands) and writes
``counters-aggregate.csv`` beside this script: one row per
(pass, kernel, counter) carrying the dispatch count and the counter's
mean/min/max over all dispatches, plus the kernel's per-dispatch resource
fields (VGPR_Count, Accum_VGPR_Count, SGPR_Count, LDS_Block_Size,
Scratch_Size, Workgroup_Size), which must be constant across the pass's
dispatches of that kernel — a varying resource field aborts the run rather
than averaging silently.

Usage, from the repository root:

    python3 dev/studies/6c7fcb38/profiled-20260815T181923Z/aggregate-counters.py \
        <raw-retention-dir>
"""

from __future__ import annotations

import csv
import io
import sys
from collections import defaultdict
from pathlib import Path

from compression import zstd  # Python >= 3.14

HERE = Path(__file__).resolve().parent
OUT = HERE / "counters-aggregate.csv"
RESOURCE_FIELDS = [
    "VGPR_Count",
    "Accum_VGPR_Count",
    "SGPR_Count",
    "LDS_Block_Size",
    "Scratch_Size",
    "Workgroup_Size",
]


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    raw_root = Path(sys.argv[1])
    files = sorted(raw_root.glob("pmc-*/*_counter_collection.csv.zst"))
    if not files:
        print(f"no counter files under {raw_root}", file=sys.stderr)
        return 1

    rows_out = []
    for path in files:
        pass_name = path.parent.name
        with zstd.open(path, "rt", encoding="utf-8", newline="") as fh:
            reader = csv.DictReader(fh)
            values = defaultdict(list)
            resources: dict[str, dict[str, str]] = {}
            for row in reader:
                kernel = row["Kernel_Name"]
                values[(kernel, row["Counter_Name"])].append(
                    float(row["Counter_Value"])
                )
                res = {f: row[f] for f in RESOURCE_FIELDS}
                seen = resources.setdefault(kernel, res)
                if seen != res:
                    print(
                        f"{path}: resource fields vary across dispatches of "
                        f"{kernel!r}: {seen} != {res}",
                        file=sys.stderr,
                    )
                    return 1
        for (kernel, counter), vals in sorted(values.items()):
            rows_out.append(
                {
                    "pass": pass_name,
                    "kernel": kernel,
                    "counter": counter,
                    "dispatches": len(vals),
                    "mean": f"{sum(vals) / len(vals):.6f}",
                    "min": f"{min(vals):.6f}",
                    "max": f"{max(vals):.6f}",
                    **resources[kernel],
                }
            )

    with OUT.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=["pass", "kernel", "counter", "dispatches", "mean", "min", "max"]
            + RESOURCE_FIELDS,
        )
        writer.writeheader()
        writer.writerows(rows_out)
    print(f"wrote {OUT} ({len(rows_out)} rows from {len(files)} passes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
