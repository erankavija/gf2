#!/usr/bin/env python3
"""Record the untimed peak memory of the canonical and QC arms (jit:f63a2464).

For every cell of the two families that compare the QC prototype with the
canonical decoder, and for each of the two arms, this projects a throwaway plan
whose cell names that one arm in both positions and drives it with the shared
`benchmark-ab-runner smoke`: one untimed dispatch per position on the cell's
declared workload, workers and CPUs. The observation is the `ru_maxrss` that
`wait4` reports for the smoke process: the largest resident set among the
runner and the arm children it reaps. The same runner without an arm
(`benchmark-ab-runner check`) is recorded as the floor of that observation: a
spawned process starts from its launcher's resident set, so the floor bounds
what the launcher and the runner contribute.

Nothing is timed: the smoke takes no host lock, opens no ledger and writes no
receipt, and this script reads no clock.

Usage (from the worktree root):
  record-memory.py OUT_JSON --runner EXE --baseline-dir DIR --candidate-dir DIR
      --bundles-dir DIR --quality-dir DIR --scratch DIR [--samples N]
"""

import argparse
import hashlib
import json
import os
import pathlib
import statistics
import subprocess
import sys

SURVEY = pathlib.Path(__file__).resolve().parent
ROOT = pathlib.Path(subprocess.run(
    ["git", "-C", str(SURVEY), "rev-parse", "--show-toplevel"],
    check=True, capture_output=True, text=True,
).stdout.strip())
ACTIVE = SURVEY.parent.relative_to(ROOT)
# The families whose two arms share one build kind, so one arm may hold both
# positions of a cell under the runner's plan validation.
FAMILIES = ["intra-frame-single-worker", "intra-frame-multicore"]


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def relative(path):
    path = pathlib.Path(path).resolve()
    return str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)


def peak_kib(command):
    """`ru_maxrss` of `command` and the children it reaps, in KiB."""
    child = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    stderr = child.stderr.read()
    _, status, usage = os.wait4(child.pid, 0)
    child.returncode = os.waitstatus_to_exitcode(status)
    if child.returncode != 0:
        sys.exit(f"{' '.join(command)} exited {child.returncode}: {stderr.decode().strip()}")
    return usage.ru_maxrss


def spread(samples):
    return {
        "samples_kib": samples,
        "count": len(samples),
        "min_kib": min(samples),
        "median_kib": statistics.median(samples),
        "max_kib": max(samples),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out", type=pathlib.Path)
    for name in ("--runner", "--baseline-dir", "--candidate-dir", "--bundles-dir",
                 "--quality-dir", "--scratch"):
        parser.add_argument(name, required=True)
    parser.add_argument("--samples", type=int, default=5)
    args = parser.parse_args()
    if pathlib.Path.cwd() != ROOT:
        sys.exit("invoke from the worktree root")

    scratch = pathlib.Path(args.scratch)
    scratch.mkdir(parents=True, exist_ok=True)
    for stale in scratch.glob("*.plan.json"):
        stale.unlink()
    rows, executables, floors = [], {}, []
    for family in FAMILIES:
        addendum = ACTIVE / f"addendum-ldpc-qc-{family}-pilot.json"
        base = scratch / f"{family}.plan.json"
        subprocess.run(
            [sys.executable, "-B", str(SURVEY / "make-plan.py"),
             "--family", f"ldpc-qc-{family}-v1", "--label", "pilot",
             "--addendum", str(addendum),
             "--baseline-dir", os.path.abspath(args.baseline_dir),
             "--candidate-dir", os.path.abspath(args.candidate_dir),
             "--bundles-dir", os.path.abspath(args.bundles_dir),
             "--quality-dir", os.path.abspath(args.quality_dir),
             "--campaign-id", f"f63a2464-memory-{family}", "--output", str(base)],
            check=True,
        )
        plan = json.loads(base.read_text(encoding="utf-8"))
        floors += [peak_kib([args.runner, "check", str(base)]) for _ in range(args.samples)]
        declared = {
            cell["cell_id"]: cell
            for cell in json.loads((ROOT / addendum).read_text(encoding="utf-8"))["cells"]
        }
        for cell in plan["cells"]:
            for arm in (cell["baseline_arm"], cell["candidate_arm"]):
                single = dict(plan)
                single["arms"] = {arm: plan["arms"][arm]}
                single["cells"] = [dict(cell, baseline_arm=arm, candidate_arm=arm)]
                path = scratch / f"{family}-{cell['cell_id']}-{arm}.plan.json"
                path.write_text(json.dumps(single, indent=2) + "\n", encoding="utf-8")
                samples = [peak_kib([args.runner, "smoke", str(path)])
                           for _ in range(args.samples)]
                executable = plan["arms"][arm]["executable"]
                executables[arm] = {
                    "executable": relative(executable),
                    "sha256": digest(executable),
                }
                size = declared[cell["cell_id"]]["workload"]["size"]
                rows.append({
                    "family": f"ldpc-qc-{family}-v1",
                    "addendum": str(addendum),
                    "cell_id": cell["cell_id"],
                    "arm": arm,
                    "workers": size["workers"],
                    "frames_per_worker_call": cell["case"]["batch_size"],
                    "bundle": relative(cell["case"]["bundle"]),
                    **spread(samples),
                })

    record = {
        "schema": "ldpc-candidate-peak-memory-v1",
        "method": (
            "ru_maxrss reported by wait4 for one `benchmark-ab-runner smoke` process over a plan "
            "whose single cell names one arm in both positions: the largest resident set among "
            "the runner and the arm children it reaps, each arm child performing one untimed "
            "dispatch of the cell's declared workload. Linux reports the value in KiB. The floor "
            "is the same observation of `benchmark-ab-runner check`, which launches no arm; a "
            "spawned process starts from its launcher's resident set, so the floor bounds what "
            "the launcher and the runner contribute and an observation above it is an arm's."
        ),
        "commands": {
            "observation": f"{relative(args.runner)} smoke <single-arm plan>",
            "floor": f"{relative(args.runner)} check <family plan>",
        },
        "runner": {"executable": relative(args.runner), "sha256": digest(args.runner)},
        "arms": executables,
        "floor": spread(floors),
        "observations": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(args.out.as_posix())


if __name__ == "__main__":
    main()
