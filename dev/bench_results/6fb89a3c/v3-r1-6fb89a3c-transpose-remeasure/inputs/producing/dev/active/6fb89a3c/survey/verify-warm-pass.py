#!/usr/bin/env python3
"""Observe each C arm's warm pass under gdb without measuring a window.

Usage: dev/active/6fb89a3c/survey/verify-warm-pass.py [--arms-dir DIR] [--report PATH]
(from the repo root; defaults: the survey directory and survey/warm-pass-report.json)

Protocol v3 defines `warm` as one untimed pass over the working set before
calibration. For every C arm case that `verify-arm-framing.py` exercises, this
script runs the arm under gdb with a breakpoint on the library call its timed
body makes, counting only calls made from the arm itself:

1. warm request, zero windows: `run_windows` refuses zero windows before its
   calibration call, so every observed call belongs to the warm pass or to a
   setup probe;
2. cold request, zero windows: the same without the warm pass, so the
   difference between the two counts is the warm pass, which must be one call
   per bank (a warm request uses one bank);
3. warm request, one window: the process is killed at the first call after
   those, which is the calibration call of `run_windows`: the timed body on
   bank 0. The warm pass's bank-0 call must pass the same persistent buffers
   (inputs, preallocated outputs, adapter buffers) as that call.

The zero-window runs exit before any timing and the one-window run is killed
inside its calibration call, so no window is measured. The report records each
arm's executable digest. `warm-pass-report.json` checks the current arms;
`warm-pass-report-receipt-arms.json` checks, through `--arms-dir`, the C arm
executables the committed v3 confirmation receipts pin (the M4RI transpose arm
rebuilt byte for byte from its source at commit 7bdad6ff; the other three are
the current executables).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
# (binary, case, library call of the timed body, arguments that name buffers or
# sizes the timed call reuses on every call). Registers follow the SysV ABI;
# `array` is xor_gen_base's pointer array, compared element by element.
CASES = [
    ("m4ri_transpose_arm", {"n": 64, "seed": 1}, "mzd_transpose", ["rdi", "rsi"]),
    ("m4ri_transpose_arm", {"cols": 63, "rows": 63, "seed": 1}, "mzd_transpose", ["rdi", "rsi"]),
    ("bitshuffle_transpose_arm", {"n": 64, "seed": 1}, "bshuf_bitshuffle", ["rdi", "rsi", "rdx", "rcx", "r8"]),
    # At 64x64 the adapter writes the planes straight into the fresh canonical
    # output each call allocates, so the output pointer is not a reused buffer.
    ("bitshuffle_transpose_arm", {"cols": 64, "rows": 64, "seed": 1, "adapter": "padded"}, "bshuf_bitshuffle",
     ["rdi", "rdx", "rcx", "r8"]),
    ("bitshuffle_transpose_arm", {"cols": 65, "rows": 65, "seed": 1, "adapter": "padded"}, "bshuf_bitshuffle",
     ["rdi", "rsi", "rdx", "rcx", "r8"]),
    ("isal_xor_arm", {"alignment_bytes": 32, "seed": 1, "words": 8}, "xor_gen_base", ["edi", "esi", "array"]),
    ("isal_xor_arm", {"alignment_bytes": 32, "seed": 1, "words": 64, "sources": 3}, "xor_gen_base",
     ["edi", "esi", "array"]),
    ("m4ri_genmatrix_arm", {"code": "B1", "seed": 1}, "mzd_copy", ["rdi", "rsi"]),
]

# Runs inside gdb. Stops at main (shared libraries are then mapped), breaks at
# the exact entry of the trigger so [rsp] is the return address, and records
# the arguments of every call whose caller lies in the main executable.
GDB_SCRIPT = r'''
import gdb, json, os
spec = json.loads(os.environ["WARM_PASS_SPEC"])
gdb.execute("set pagination off")
gdb.execute("set confirm off")
calls, exited = [], []
gdb.events.exited.connect(lambda event: exited.append(getattr(event, "exit_code", None)))
gdb.execute("break main")
gdb.execute("run < %s > %s 2>&1" % (spec["request"], spec["output"]))
gdb.Breakpoint("*" + spec["trigger"])
while True:
    gdb.execute("continue")
    if exited or gdb.selected_inferior().pid == 0:
        break
    frame = gdb.selected_frame()
    caller = int(gdb.parse_and_eval("*(unsigned long *)$rsp"))
    if gdb.solib_name(caller) is not None:
        continue
    record = {}
    for name in spec["registers"]:
        if name == "array":
            vects = int(frame.read_register("rdi")) & 0xffffffff
            base = int(frame.read_register("rdx"))
            record[name] = [int(gdb.parse_and_eval("*(unsigned long *)%d" % (base + 8 * i))) for i in range(vects)]
        elif name.startswith("e"):
            record[name] = int(frame.read_register("r" + name[1:])) & 0xffffffff
        else:
            record[name] = int(frame.read_register(name)) & 0xffffffffffffffff
    calls.append(record)
    if spec["stop_after"] and len(calls) >= spec["stop_after"]:
        gdb.execute("kill")
        break
with open(spec["result"], "w") as handle:
    json.dump({"calls": calls, "exit": exited[0] if exited else None}, handle)
'''


def observe(binary: Path, case: dict, trigger: str, registers: list[str], cache: str, windows: int,
            stop_after: int, scratch: Path) -> dict:
    request = {
        "schema": "zen3-benchmark-arm-request-v1", "cell_id": "warm-pass-check", "arm": binary.name,
        "role": "candidate", "pair": 0, "case": dict(sorted(case.items())), "cache_state": cache,
        "windows": windows, "window_target_ms": 1, "cpus": [], "workers_declared": 1,
    }
    (scratch / "request.json").write_text(json.dumps(request, separators=(",", ":")))
    (scratch / "gdb_script.py").write_text(GDB_SCRIPT)
    spec = {"request": str(scratch / "request.json"), "output": str(scratch / "arm-output.txt"),
            "result": str(scratch / "result.json"), "trigger": trigger, "registers": registers,
            "stop_after": stop_after}
    env = dict(os.environ, GF2_TUNING_FRESH_CASE="child-v2", WARM_PASS_SPEC=json.dumps(spec))
    (scratch / "result.json").unlink(missing_ok=True)
    process = subprocess.run(
        ["gdb", "-batch", "-nx", "-iex", "set debuginfod enabled off", "-x", str(scratch / "gdb_script.py"),
         "--args", str(binary)],
        cwd=REPO, env=env, capture_output=True, text=True, timeout=300,
    )
    if not (scratch / "result.json").exists():
        raise SystemExit(f"{binary.name} {case} {cache}: gdb produced no result: {process.stdout[-2000:]} {process.stderr[-2000:]}")
    return json.loads((scratch / "result.json").read_text())


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--arms-dir", type=Path, default=ROOT)
    parser.add_argument("--report", type=Path, default=ROOT / "warm-pass-report.json")
    args = parser.parse_args()
    gdb_version = subprocess.run(["gdb", "--version"], capture_output=True, text=True).stdout.splitlines()[0]
    results, failures = [], []
    with tempfile.TemporaryDirectory() as directory:
        scratch = Path(directory)
        for name, case, trigger, registers in CASES:
            binary = (args.arms_dir / name).resolve()
            warm0 = observe(binary, case, trigger, registers, "warm", 0, 0, scratch)
            cold0 = observe(binary, case, trigger, registers, "cold", 0, 0, scratch)
            warm_calls = len(warm0["calls"]) - len(cold0["calls"])
            calibration = len(warm0["calls"]) + 1
            warm1 = observe(binary, case, trigger, registers, "warm", 1, calibration, scratch)
            checks = {
                "zero_window_requests_refused": warm0["exit"] not in (None, 0) and cold0["exit"] not in (None, 0),
                "one_warm_call_per_bank": warm_calls == 1,
                "calibration_call_reached": len(warm1["calls"]) == calibration,
            }
            same = {}
            if checks["calibration_call_reached"] and warm_calls >= 1:
                first, timed = warm1["calls"][0], warm1["calls"][-1]
                same = {register: first[register] == timed[register] for register in registers}
            checks["warm_call_uses_the_timed_buffers"] = bool(same) and all(same.values())
            status = "pass" if all(checks.values()) else "fail"
            results.append({
                "binary": name,
                "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "case": case,
                "trigger": trigger,
                "warm_pass_calls": warm_calls,
                "setup_probe_calls": len(cold0["calls"]),
                "compared_arguments": same,
                "checks": checks,
                "status": status,
            })
            if status != "pass":
                failures.append(f"{name} {case}: {checks} {same}")
    report = {
        "schema": "6fb89a3c-warm-pass-report-v1",
        "method": ("gdb breakpoint at the library call of each C arm's timed body, counting calls from the arm: warm "
                   "and cold zero-window requests isolate the warm pass, and a one-window warm request is killed at "
                   "its calibration call, the timed body on bank 0, whose buffer arguments the warm call must repeat; "
                   "no window is measured"),
        "gdb": gdb_version,
        "arms": results,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    for item in results:
        print(f"{item['status'].upper()} {item['binary']} {item['case']}: warm-pass calls {item['warm_pass_calls']}, "
              f"same buffers {item['compared_arguments']}")
    if failures:
        raise SystemExit("warm pass does not repeat the timed call:\n" + "\n".join(failures))
    print(f"-> {args.report}")


if __name__ == "__main__":
    main()
