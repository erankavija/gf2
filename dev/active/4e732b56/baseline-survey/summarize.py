#!/usr/bin/env python3
"""Aggregate the survey's raw per-trial CSVs into the tables the findings
document reports (jit:4e732b56).

Reads every ``*.csv`` a run wrote and prints, per cell, the trial count and the
min / median / max of the observed throughput. The findings document quotes
these tables rather than restating individual rows, so a re-run regenerates the
numbers instead of inviting a hand edit.

Usage:
    ./summarize.py <run-dir> [--workload W1|W2] [--markdown]
"""

from __future__ import annotations

import argparse
import csv
import pathlib
import statistics
import sys

# The two harness schemas differ: the encoder harnesses carry a `t` column and
# report per-frame nanoseconds under `info_mbit_per_s`, while the M4RI harness
# reports whole-call nanoseconds under `bits_per_s_scaled`. `load` reads
# whichever pair a file actually has, so neither schema is hard-coded here.


def load(run_dir: pathlib.Path) -> list[dict]:
    rows: list[dict] = []
    for path in sorted(run_dir.glob("*.csv")):
        with path.open() as fh:
            reader = csv.DictReader(fh)
            if reader.fieldnames is None:
                continue
            fields = set(reader.fieldnames)
            if not {"lib", "workload", "algorithm", "code"} <= fields:
                print(f"# skipping {path.name}: not a survey CSV", file=sys.stderr)
                continue
            for row in reader:
                if None in row.values():
                    continue
                row["_source"] = path.name
                row["_rate"] = float(row.get("info_mbit_per_s") or row["bits_per_s_scaled"])
                row["_ns"] = float(row.get("ns_per_frame") or row["ns_total"])
                rows.append(row)
    return rows


def summarize(rows: list[dict], workload: str | None) -> list[dict]:
    cells: dict[tuple, list[dict]] = {}
    for row in rows:
        if workload and row["workload"] != workload:
            continue
        key = (row["workload"], row["code"], row["lib"], row["version"], row["algorithm"], int(row["batch"]))
        cells.setdefault(key, []).append(row)

    out = []
    for key, group in sorted(cells.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][5], kv[0][2], kv[0][4])):
        rates = sorted(r["_rate"] for r in group)
        nss = sorted(r["_ns"] for r in group)
        digests = {r["digest"] for r in group}
        out.append(
            {
                "workload": key[0], "code": key[1], "lib": key[2], "version": key[3],
                "algorithm": key[4], "batch": key[5], "trials": len(group),
                "rate_min": rates[0], "rate_med": statistics.median(rates), "rate_max": rates[-1],
                "ns_med": statistics.median(nss),
                "spread_pct": (rates[-1] - rates[0]) / statistics.median(rates) * 100.0,
                # Rows the harness could not measure inside its cell budget are
                # emitted with an `-projected` algorithm and trial -1. They are
                # estimates and are never merged into a measured cell.
                "projected": key[4].endswith("-projected"),
                "digest": digests.pop() if len(digests) == 1 else "VARIES",
            }
        )
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("run_dir", type=pathlib.Path)
    ap.add_argument("--workload", choices=["W1", "W2"])
    ap.add_argument("--markdown", action="store_true")
    args = ap.parse_args()

    rows = load(args.run_dir)
    if not rows:
        print("no survey rows found", file=sys.stderr)
        return 1
    cells = summarize(rows, args.workload)

    if args.markdown:
        print("| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |")
        print("|---|---|---|---|---|---|---|---|---|---|")
        for c in cells:
            if c["projected"]:
                print(
                    f"| {c['workload']} | {c['code']} | {c['batch']} | {c['lib']} {c['version']} "
                    f"| `{c['algorithm']}` | *estimate* | {c['rate_med']:.1f} | — | — | — |"
                )
            else:
                print(
                    f"| {c['workload']} | {c['code']} | {c['batch']} | {c['lib']} {c['version']} | `{c['algorithm']}` "
                    f"| {c['trials']} | {c['rate_med']:.1f} | {c['rate_min']:.1f} | {c['rate_max']:.1f} "
                    f"| {c['spread_pct']:.1f}% |"
                )
    else:
        for c in cells:
            if c["projected"]:
                print(
                    f"{c['workload']:3} {c['code']:4} batch={c['batch']:<5} {c['lib']:7} {c['algorithm']:28} "
                    f"ESTIMATE  {c['rate_med']:10.2f} Mbit/s (not measured)"
                )
            else:
                print(
                    f"{c['workload']:3} {c['code']:4} batch={c['batch']:<5} {c['lib']:7} {c['algorithm']:28} "
                    f"n={c['trials']} median={c['rate_med']:10.2f} Mbit/s "
                    f"[{c['rate_min']:.2f}, {c['rate_max']:.2f}] spread={c['spread_pct']:.1f}%"
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
