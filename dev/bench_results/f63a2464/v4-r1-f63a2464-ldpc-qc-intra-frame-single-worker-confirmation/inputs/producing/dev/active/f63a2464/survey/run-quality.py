#!/usr/bin/env python3
"""Run the untimed candidate quality campaign (jit:f63a2464).

Bounded and resumable: the cell matrix is fixed here, each cell writes one
immutable result file, and a rerun omits every cell whose file already exists
instead of decoding it again. The canonical append-only execution log is opened
and its path printed before the first cell runs; console output is a view of
that file. Nothing here is timed, so it runs outside the benchmark window.

Usage (from the worktree root):
  run-quality.py OUT_DIR [--workers N] [--only SUBSTRING]
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

BINARY = pathlib.Path("target/ldpc-candidate-prototypes/release/ldpc-candidate-quality")
BUNDLES = pathlib.Path("target/ldpc-inputs")

# The quantized family's exploratory configurations: the channel scale of each
# alphabet, inside the per-family configuration budget the decision record
# freezes.
NARROW_SCALES = [1.0, 2.0, 4.0, 8.0]
WIDE_SCALES = [8.0, 16.0, 32.0, 64.0]

QUANTIZED_ARMS = [f"quantized-i8:{scale}" for scale in NARROW_SCALES] + [
    f"quantized-i16:{scale}" for scale in WIDE_SCALES
]


def cells():
    """Every declared (bundle, arm, cell) triple, in a fixed order."""
    dvb = "dvb-t2-r12-waterfall"
    nr = "nr-bg1-z384-mother"
    for arm in ["canonical-f32", "layered-f32", *QUANTIZED_ARMS]:
        yield {"bundle": dvb, "arm": arm, "cell": "recorded", "qc_base": None}
    for cell in ["recorded", "punctured", "filler"]:
        for arm in ["canonical-f32", "qc-f32", "layered-f32", *QUANTIZED_ARMS]:
            yield {"bundle": nr, "arm": arm, "cell": cell, "qc_base": "1:384"}


def cell_id(cell):
    arm = cell["arm"].replace(":", "-")
    return f"{cell['bundle']}__{arm}__{cell['cell']}"


class Log:
    """The canonical append-only execution log."""

    def __init__(self, path):
        self.path = path
        self.handle = open(path, "a", encoding="utf-8")

    def record(self, kind, **fields):
        entry = {"utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "kind": kind}
        entry.update(fields)
        self.handle.write(json.dumps(entry, sort_keys=True) + "\n")
        self.handle.flush()
        os.fsync(self.handle.fileno())
        print(f"{entry['kind']} {fields.get('cell', '')}", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out_dir", type=pathlib.Path)
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--only", default=None)
    args = parser.parse_args()

    repo = pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True
        ).stdout.strip()
    )
    if pathlib.Path.cwd() != repo:
        sys.exit("invoke from the worktree root")
    if not BINARY.exists():
        sys.exit(f"{BINARY} is not built")

    results = args.out_dir / "cells"
    results.mkdir(parents=True, exist_ok=True)
    log = Log(args.out_dir / "execution.log")
    print(f"GF2_QUALITY_EXECUTION_LOG={(args.out_dir / 'execution.log').as_posix()}", flush=True)

    selected = [
        cell for cell in cells() if args.only is None or args.only in cell_id(cell)
    ]
    log.record(
        "campaign-start",
        binary=BINARY.as_posix(),
        binary_sha256=subprocess.run(
            ["sha256sum", str(BINARY)], capture_output=True, text=True, check=True
        ).stdout.split()[0],
        cells=len(selected),
        workers=args.workers,
    )

    lock = __import__("threading").Lock()
    failures = []

    def run_cell(cell):
        identifier = cell_id(cell)
        out = results / f"{identifier}.json"
        if out.exists():
            with lock:
                log.record("cell-omitted", cell=identifier, path=out.as_posix())
            return
        command = [
            str(BINARY),
            "--bundle",
            str(BUNDLES / cell["bundle"]),
            "--arm",
            cell["arm"],
            "--cell",
            cell["cell"],
            "--out",
            str(out),
        ]
        if cell["qc_base"]:
            command += ["--qc-base", cell["qc_base"]]
        with lock:
            log.record("cell-start", cell=identifier, command=command)
        started = time.monotonic()
        finished = subprocess.run(command, capture_output=True, text=True)
        elapsed = round(time.monotonic() - started, 3)
        with lock:
            if finished.returncode == 0:
                log.record("cell-complete", cell=identifier, status="measured",
                           wall_seconds=elapsed)
            else:
                failures.append(identifier)
                log.record("cell-failed", cell=identifier, exit_code=finished.returncode,
                           stderr=finished.stderr.strip()[:500])

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        list(pool.map(run_cell, selected))

    log.record("complete" if not failures else "failed", failed=failures)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
