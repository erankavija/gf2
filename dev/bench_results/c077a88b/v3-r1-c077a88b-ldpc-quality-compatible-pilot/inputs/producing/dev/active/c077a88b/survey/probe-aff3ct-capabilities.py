#!/usr/bin/env python3
"""Runtime capability screen of the pinned AFF3CT LDPC decoders (jit:c077a88b).

REQ-03 of the issue asks which AFF3CT schedule, precision and INTRA/INTER
combinations the pinned build actually supports, and which of the supported
ones are fast enough and accurate enough to be worth a confirmatory
measurement. Nothing here is a performance receipt: it is a short screen that
records, for each combination, whether the pinned executable can build the
decoder at all, and if so the frame error rate and the simulator's own
throughput on a small budget. The combinations it admits are then measured
properly by the benchmark runner.

Usage:
  probe-aff3ct-capabilities.py --aff3ct <bin> --alist <file> -K <k> -N <n> \
      --esn0 <db> --iterations 50 --norm 0.75 --frames 64 --output <report.json>
"""

import argparse
import itertools
import json
import os
import re
import signal
import subprocess
import sys

TYPES = ["BP_FLOODING", "BP_HORIZONTAL_LAYERED", "BP_VERTICAL_LAYERED"]
IMPLEMS = ["MS", "NMS", "OMS", "SPA"]
SIMDS = ["", "INTRA", "INTER"]
PRECISIONS = [32, 16, 8]

# The BFER terminal table, after collapsing '|' to whitespace, is
# Es/N0 Eb/N0 FRA BE FE BER FER SIM_THR ET/RT.
ROW = re.compile(r"^\s*-?\d+\.\d+\s")


def run(args, combination):
    kind, implem, simd, precision = combination
    # Quality screening uses the CPU budget and never takes the timing mutex.
    # The dedicated budget lock keeps the CPU-slot mechanism active.
    command = [
        "./scripts/cargo-budget.sh",
        args.aff3ct, "--sim-type", "BFER", "-C", "LDPC",
        "--dec-h-path", args.alist, "-K", str(args.K), "-N", str(args.N),
        "--enc-type", "AZCW", "--src-type", "AZCW",
        "--mdm-type", "BPSK", "--chn-type", "AWGN",
        "--sim-noise-type", "ESN0", "-R", str(args.esn0),
        "--dec-type", kind, "--dec-implem", implem,
        "--dec-ite", str(args.iterations), "--dec-h-reorder", "NONE",
        "--sim-max-fra", str(args.frames), "--sim-stop-time", "5", "--sim-seed", "42", "--ter-freq", "0",
        "-p", str(precision), "-t", "1",
    ]
    if implem == "NMS":
        command += ["--dec-norm", str(args.norm)]
    if simd:
        command += ["--dec-simd", simd]
    # The child runs in its own process group so a timeout can reap the whole
    # tree. Killing only the `flock` wrapper would leave the decoder running
    # and holding the shared side of the benchmark mutex, which blocks every
    # exclusive timed run on the host until the orphan exits.
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
        env=dict(os.environ, GF2_CCX1_LOCK="/tmp/c077a88b-quality-budget.lock", CARGO_CI_NO_SCCACHE="1"),
    )
    try:
        stdout, stderr = process.communicate(timeout=args.timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            stdout, stderr = process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
        return {"supported": None, "reason": "timed out", "command": command, "stdout": stdout, "stderr": stderr}
    text = stdout + stderr
    if "cannot_allocate" in text or "Cannot allocate the object" in text:
        return {
            "supported": False,
            "reason": "the pinned build has no decoder for this combination",
            "command": command, "stdout": stdout, "stderr": stderr,
        }
    row = None
    for line in text.splitlines():
        if line.startswith("#"):
            continue
        cleaned = line.replace("|", " ")
        if ROW.match(cleaned):
            row = cleaned.split()
    if row is None or len(row) < 8:
        first_error = next(
            (line for line in text.splitlines() if "(EE)" in line), "no data row"
        )
        return {"supported": False, "reason": first_error.strip(), "command": command, "stdout": stdout, "stderr": stderr}
    return {
        "supported": True,
        "reason": None,
        "frames": int(row[2]),
        "bit_errors": int(row[3]),
        "frame_errors": int(row[4]),
        "ber": float(row[5]),
        "fer": float(row[6]),
        "simulated_mbps": float(row[7]),
        "command": command, "stdout": stdout, "stderr": stderr,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--aff3ct", required=True)
    parser.add_argument("--alist", required=True)
    parser.add_argument("-K", type=int, required=True)
    parser.add_argument("-N", type=int, required=True)
    parser.add_argument("--esn0", type=float, required=True)
    parser.add_argument("--iterations", type=int, default=50)
    parser.add_argument("--norm", type=float, default=0.75)
    parser.add_argument("--frames", type=int, default=64)
    parser.add_argument("--timeout", type=int, default=600)
    parser.add_argument("--types", nargs="+", default=TYPES, choices=TYPES)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    version = subprocess.run(
        ["./scripts/cargo-budget.sh", args.aff3ct, "--version"],
        capture_output=True, text=True,
    ).stdout.splitlines()[0]
    results = []
    for combination in itertools.product(args.types, IMPLEMS, SIMDS, PRECISIONS):
        outcome = run(args, combination)
        outcome["type"], outcome["implem"] = combination[0], combination[1]
        outcome["simd"], outcome["precision_bits"] = combination[2] or "none", combination[3]
        results.append(outcome)
        state = {True: "ok", False: "no", None: "timeout"}[outcome["supported"]]
        print(
            "%-24s %-4s %-5s %2d  %s" % (
                combination[0], combination[1], combination[2] or "-",
                combination[3], state,
            ),
            file=sys.stderr,
        )
    report = {
        "schema": "ldpc-survey-aff3ct-capabilities-v1",
        "aff3ct_version": version,
        "alist": args.alist,
        "K": args.K,
        "N": args.N,
        "esn0_db": args.esn0,
        "iterations": args.iterations,
        "norm_factor": args.norm,
        "frame_budget": args.frames,
        "combinations": results,
    }
    with open(args.output, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(args.output)


if __name__ == "__main__":
    main()
