#!/usr/bin/env python3
"""Render the survey's benchmark receipt from a run directory (jit:4e732b56).

Every figure, file name, and provenance line the receipt carries is read out of
the run directory at render time. Nothing is embedded here, so re-rendering a
re-run produces a receipt describing that run rather than this one.

Usage:
    ./make-receipt.py <run-dir> > <run-dir>/<date>-<id>-survey-receipt.md
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from summarize import load, summarize  # noqa: E402


def host_field(host_files: list[pathlib.Path], pattern: str) -> str:
    """First line matching `pattern` across the run's host records."""
    rx = re.compile(pattern)
    for path in host_files:
        for line in path.read_text(errors="replace").splitlines():
            if rx.search(line):
                return line.strip()
    return "(not recorded)"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("run_dir", type=pathlib.Path)
    args = ap.parse_args()
    run = args.run_dir

    host_files = sorted(run.glob("*host.txt"))
    if not host_files:
        print(f"no host record under {run}", file=sys.stderr)
        return 1

    rows = load(run)
    if not rows:
        print(f"no measurement rows under {run}", file=sys.stderr)
        return 1

    revision = host_field(host_files, r"^# gf2 revision:").split(":", 1)[-1].strip()
    generated = [host_field([p], r"^# generated:").split(":", 1)[-1].strip() for p in host_files]

    print("# Receipt: external-baseline survey for BCH encoding and generator-matrix materialization")
    print()
    print("Rendered by `baseline-survey/make-receipt.py` from the run directory; every")
    print("figure below is read out of the committed CSVs at render time.")
    print()
    print("| Field | Value |")
    print("|---|---|")
    print("| Issue | `4e732b56` |")
    print(f"| Run timestamps (UTC) | {', '.join(generated)} |")
    print(f"| gf2 revision | `{revision}` |")
    print(f"| Host | {host_field(host_files, r'Model name:').split(':', 1)[-1].strip()} |")
    print(f"| Cores pinned | CCX1 via `dev/scripts/ccx1-bench-flock.sh` (`taskset -c 6-11`, `nice -n -5`) |")
    print(f"| Governor | {host_field(host_files, r'scaling_governor').split(':')[-1].strip()} |")
    print(f"| Kernel | {host_field(host_files, r'^Linux ')} |")
    print(f"| C compiler | {host_field(host_files, r'^gcc ')} |")
    print(f"| C++ compiler | {host_field(host_files, r'^g\+\+ ')} |")
    print(f"| Rust | {host_field(host_files, r'^rustc ')} |")
    print()
    print("## Baseline pins")
    print()
    print("| Pin | Value |")
    print("|---|---|")
    for pat in [r"^aff3ct tag:", r"^aff3ct commit:", r"^bchlib tag:", r"^bchlib commit:",
                r"^m4ri version:", r"^m4ri tarball sha256:", r"^itpp release:", r"^itpp soname:"]:
        line = host_field(host_files, pat)
        if ":" in line:
            name, _, value = line.partition(":")
            print(f"| {name.strip()} | `{value.strip()}` |")
    print()
    print("## Reference build configuration")
    print()
    for pat in [r"^aff3ct library:", r"^aff3ct defines:", r"^m4ri library:"]:
        print(f"* `{host_field(host_files, pat)}`")
    print()
    print("## Measured cells")
    print()
    print("Throughput is information bits per second for W1 and matrix bits per second")
    print("for W2. `Trials` is the number of independent trials the cell's wall budget")
    print("allowed; a cell marked *estimate* was projected from a measured per-unit cost")
    print("and was never run at that size.")
    print()
    cells = summarize(rows, None)
    print("| Workload | Row | Batch | Library | Algorithm | Trials | Median | Min | Max | Spread |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    for c in cells:
        if c["projected"]:
            print(f"| {c['workload']} | {c['code']} | {c['batch']} | {c['lib']} {c['version']} "
                  f"| `{c['algorithm']}` | *estimate* | {c['rate_med']:.2f} | — | — | — |")
        else:
            print(f"| {c['workload']} | {c['code']} | {c['batch']} | {c['lib']} {c['version']} "
                  f"| `{c['algorithm']}` | {c['trials']} | {c['rate_med']:.2f} | {c['rate_min']:.2f} "
                  f"| {c['rate_max']:.2f} | {c['spread_pct']:.1f}% |")
    print()
    print("## Files")
    print()
    print("| File | SHA-256 | Bytes |")
    print("|---|---|---|")
    for path in sorted(run.iterdir()):
        if not path.is_file() or path.name.endswith("-receipt.md"):
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        print(f"| `{path.name}` | `{digest[:16]}…` | {path.stat().st_size} |")
    print()
    print("## Reproduction")
    print()
    print("```")
    print("dev/active/4e732b56/baseline-survey/fetch-build.sh")
    print(f"dev/active/4e732b56/baseline-survey/run-survey.sh {run} <codes> <prefix>")
    print(f"dev/active/4e732b56/baseline-survey/make-receipt.py {run}")
    print("```")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
