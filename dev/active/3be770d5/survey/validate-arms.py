#!/usr/bin/env python3
"""Exercise every arm of every pilot cell end to end without timing (jit:3be770d5).

For each cell of the three pilot addenda, sends both arm executables the
canonical child request with role `validation`: the arm builds its pinned
worker pool, runs one untimed dispatch of the declared per-worker batch, and
applies every placement, thread-count and prepared-quality check before
writing its result line. The CPU sets are the first 1, 6, 12 or 24 CPUs of
this process's affinity mask, not the runner's resolution. Appends one record
per arm execution to the output and exits nonzero on any failure.

Usage: validate-arms.py --bin-dir DIR --inputs DIR --quality DIR --output FILE
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ACTIVE = HERE.parent
CATALOGUE = json.loads((HERE / "arms.json").read_text())
FRESH_CASE_VAR, FRESH_CASE_VALUE = "GF2_TUNING_FRESH_CASE", "child-v2"
RESULT_PREFIX = "GF2_TUNING_RESULT="
sys.dont_write_bytecode = True
sys.path.insert(0, str(HERE))
candidate_of = __import__("make-plan").candidate_of


def main():
    parser = argparse.ArgumentParser()
    for name in ("--bin-dir", "--inputs", "--quality", "--output"):
        parser.add_argument(name, required=True)
    args = parser.parse_args()
    own = sorted(os.sched_getaffinity(0))
    failed = 0
    with open(args.output, "x", encoding="utf-8") as output:
        for addendum_path in sorted(ACTIVE.glob("addendum-*-pilot.json")):
            addendum = json.loads(addendum_path.read_text())
            for cell in addendum["cells"]:
                code_key = next(k for k in CATALOGUE["codes"] if cell["cell_id"].startswith(k + "-"))
                code = CATALOGUE["codes"][code_key]
                workers = cell["workers"]["declared"]
                case = {
                    "batch_size": cell["workload"]["size"]["timed_batch"],
                    "bundle": os.path.join(args.inputs, code["bundle"]),
                    "code": code["code"],
                    "iteration_cap": cell["decoder"]["iteration_cap"],
                    "normalization_factor": cell["decoder"]["normalization"]["factor"],
                    "quality_frames": cell["decoder"]["input"]["frames"],
                    "syndrome_stopping": True,
                }
                for arm in ("gf2-nms-f32", candidate_of(cell["cell_id"])):
                    spec = CATALOGUE["arms"][arm]
                    request = {
                        "schema": "zen3-benchmark-arm-request-v1",
                        "cell_id": cell["cell_id"],
                        "arm": f"{arm}-{code_key}",
                        "role": "validation",
                        "pair": 0,
                        "case": case,
                        "cache_state": cell["cache_state"],
                        "windows": 5,
                        "window_target_ms": 100,
                        "cpus": own[:workers],
                        "workers_declared": workers,
                    }
                    environment = {
                        "PATH": os.environ["PATH"],
                        "RAYON_NUM_THREADS": "1",
                        FRESH_CASE_VAR: FRESH_CASE_VALUE,
                        "GF2_LDPC_QUALITY": os.path.join(args.quality, f"{arm}-{code_key}.json"),
                        **spec["environment"],
                    }
                    done = subprocess.run(
                        [os.path.join(args.bin_dir, spec["executable"])],
                        input=json.dumps(request, separators=(",", ":")),
                        env=environment, capture_output=True, text=True)
                    lines = [line for line in done.stdout.splitlines() if line.startswith(RESULT_PREFIX)]
                    result = json.loads(lines[0][len(RESULT_PREFIX):]) if len(lines) == 1 else None
                    ok = (done.returncode == 0 and result is not None
                          and result["workers_observed"] == workers
                          and result["cpus_observed"] == own)
                    record = {"addendum": addendum_path.name, "cell_id": cell["cell_id"], "arm": arm,
                              "cpus": own[:workers], "exit": done.returncode, "passed": ok,
                              "result": result, "stderr": done.stderr.strip()}
                    output.write(json.dumps(record) + "\n")
                    output.flush()
                    print(f"{cell['cell_id']} {arm}: {'ok' if ok else 'FAILED'}", file=sys.stderr)
                    failed += not ok
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
