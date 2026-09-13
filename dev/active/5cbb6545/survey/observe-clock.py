#!/usr/bin/env python3
"""Observe the core clock each confirmed count route runs at (jit:5cbb6545).

The receipts time calls in nanoseconds and count no cycles, so a cycles-per-byte
row needs one observed conversion. For every cell of every receipt named on the
command line this script runs the committed arm child once per arm, on that
cell's own case from the receipt's plan, under `perf stat` for user-space cycles
and task-clock, and records the ratio: cycles per nanosecond on the CPU the
receipt resolved. The child's own window record travels with each observation,
so the share of the process the timed loop occupies is read from the record
rather than assumed.

The observation establishes a clock and no comparison. Every comparative claim
belongs to the receipts, whose pairs, ordering and family correction this
script does not reproduce; it runs one child per arm outside any counterbalanced
block. It takes the canonical exclusive host mutex for the same reason a
campaign does, so a sibling worker's build does not shift the clock it reads.

Usage: observe-clock.py <output.json> <receipt-dir>...  (from the repository root)
"""

import hashlib
import json
import pathlib
import subprocess
import sys
import time

SCHEMA = "count-clock-observation-v1"
#: Plan constants of this observation: the child's fresh-case sentinel, the
#: wrapper every timed session on this host runs under, and the counters read.
SENTINEL_VAR = "GF2_TUNING_FRESH_CASE"
SENTINEL_VALUE = "child-v2"
ARM_VAR = "GF2_COUNT_ARM"
WRAPPER = ["dev/scripts/ccx1-bench-flock.sh", "--full-host"]
COUNTERS = "cycles:u,task-clock:u"
RESULT_PREFIX = "GF2_TUNING_RESULT="


def sha256(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def revision():
    return subprocess.run(
        ["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()


def bytes_per_call(case):
    """Useful bytes one call of this case reads, from the case itself."""
    if case["op"] == "popcount":
        return case["words"] * 8
    if case["op"] == "and_popcnt":
        return case["words"] * 8 * 2
    if case["op"] == "matvec":
        row_bytes = (case["cols"] + 63) // 64 * 8
        return case["rows"] * (row_bytes + row_bytes)
    raise SystemExit(f"unknown case op {case['op']!r}")


def request(cell, arm, case, cache_state, role, settings, cpus, workers):
    """The canonical request the child accepts.

    The child re-encodes what it reads and refuses any spelling but its own, so
    the request's own fields keep the child's declaration order while the case
    it carries, which the child holds as a generic value, keeps the key order
    that encoder produces: sorted.
    """
    return json.dumps(
        {
            "schema": "zen3-benchmark-arm-request-v1",
            "cell_id": cell,
            "arm": arm,
            "role": role,
            "pair": 0,
            "case": dict(sorted(case.items())),
            "cache_state": cache_state,
            "windows": settings["windows_per_execution"],
            "window_target_ms": settings["window_target_ms"],
            "cpus": cpus,
            "workers_declared": workers,
        },
        separators=(",", ":"),
        sort_keys=False,
    )


def perf_counts(stderr):
    """The two counter values of one `perf stat -x,` run."""
    counts = {}
    for line in stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2] in ("cycles:u", "task-clock:u"):
            counts[fields[2]] = float(fields[0])
    if set(counts) != {"cycles:u", "task-clock:u"}:
        raise SystemExit(f"perf reported no counters: {stderr!r}")
    return counts


def observe(binary, payload, arm, cpus):
    command = (
        WRAPPER
        + ["taskset", "-c", ",".join(str(cpu) for cpu in cpus)]
        + ["perf", "stat", "-x,", "-e", COUNTERS, "--", str(binary)]
    )
    process = subprocess.run(
        command,
        input=payload,
        capture_output=True,
        text=True,
        env={
            "PATH": "/usr/bin:/bin",
            "HOME": str(pathlib.Path.home()),
            SENTINEL_VAR: SENTINEL_VALUE,
            ARM_VAR: arm,
        },
        check=True,
    )
    line = [l for l in process.stdout.splitlines() if l.startswith(RESULT_PREFIX)]
    if len(line) != 1:
        raise SystemExit(f"child wrote no single result line: {process.stdout!r}")
    return json.loads(line[0][len(RESULT_PREFIX) :]), perf_counts(process.stderr), command


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    output = pathlib.Path(sys.argv[1])
    receipts = [pathlib.Path(arg) for arg in sys.argv[2:]]
    binary = None
    records = []
    for directory in receipts:
        plan = json.loads((directory / "plan.json").read_text())
        receipt = json.loads((directory / "receipt.json").read_text())
        addendum = json.loads((directory / "inputs" / "family-addendum.json").read_text())
        settings = receipt["settings"]
        declared = {cell["cell_id"]: cell for cell in addendum["cells"]}
        measured = {cell["cell_id"]: cell for cell in receipt["cells"]}
        for cell in plan["cells"]:
            cell_id = cell["cell_id"]
            frozen = declared[cell_id]
            cpus = measured[cell_id]["resolved_cpus"]
            case = cell["case"]
            for role_name, arm in (
                ("baseline", cell["baseline_arm"]),
                ("candidate", cell["candidate_arm"]),
            ):
                path = pathlib.Path(plan["arms"][arm]["executable"])
                binary = path if binary is None else binary
                if path != binary:
                    raise SystemExit("the receipts name more than one arm executable")
                payload = request(
                    cell_id,
                    arm,
                    case,
                    frozen["cache_state"],
                    frozen["role"],
                    settings,
                    cpus,
                    frozen["workers"]["declared"],
                )
                result, counts, command = observe(binary, payload, arm, cpus)
                timed_ns = sum(window["elapsed_ns"] for window in result["windows"])
                calls = sum(window["calls"] for window in result["windows"])
                task_clock_ns = counts["task-clock:u"] * 1e6
                records.append(
                    {
                        "receipt": str(directory),
                        "campaign": receipt["campaign_id"],
                        "cell_id": cell_id,
                        "arm": arm,
                        "arm_role": role_name,
                        "selected_path": result["selected_path"],
                        "cpus": cpus,
                        "cpus_observed": result["cpus_observed"],
                        "bytes_per_call": bytes_per_call(case),
                        "calls": calls,
                        "timed_ns": timed_ns,
                        "cycles": counts["cycles:u"],
                        "task_clock_ns": task_clock_ns,
                        "cycles_per_ns": counts["cycles:u"] / task_clock_ns,
                        "timed_share": timed_ns / task_clock_ns,
                    }
                )
                print(
                    f"{cell_id} {arm}: {records[-1]['cycles_per_ns']:.3f} cycles/ns,"
                    f" timed share {records[-1]['timed_share']:.3f}",
                    file=sys.stderr,
                )
    record = {
        "schema": SCHEMA,
        "observed_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "revision": revision(),
        "arm_executable": str(binary),
        "arm_executable_sha256": sha256(binary),
        "command_template": " ".join(
            WRAPPER
            + ["taskset", "-c", "<cpus>", "perf", "stat", "-x,", "-e", COUNTERS,
               "--", "<arm executable>"]
        ),
        "note": (
            "One child per arm per cell, on that cell's own case from the receipt's "
            "plan, outside any counterbalanced block: this record establishes the "
            "clock a route runs at and makes no comparison between arms. The arms "
            "are those of the revision recorded here, which is the delivered tree; "
            "a route this issue reverted therefore reports the clock of the route "
            "the tree keeps, as each observation's selected_path states."
        ),
        "observations": records,
    }
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{len(records)} observations -> {output}", file=sys.stderr)


if __name__ == "__main__":
    main()
