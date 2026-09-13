#!/usr/bin/env python3
"""Check the equivalent bit mapping of every arm of the external family.

Usage: verify-bit-mapping.py [--arm <gf2 arm>] [--survey <6fb89a3c survey dir>]

Every arm of a cell reads the same fixture: 64 words drawn from SplitMix64
[Steele2014] seeded at the case's `seed`, word `r` carrying matrix row `r` with
bit `c` the entry (r, c). A transpose writes 64 words whose bit `r` of word `c`
is that entry. This script draws the fixture in Python, computes the transpose
by naive bit arithmetic, and compares it against what each arm prints:

* each lane of `gf2_kernels_simd::transpose`, through this issue's gf2-side arm
  in `--dump` mode;
* M4RI's `mzd_transpose` [AlbrechtBard2026] and Bitshuffle's `bshuf_bitshuffle`
  [Bitshuffle2026], through 6fb89a3c's pinned arms in `--dump-check` mode.

An arm whose output differs at any seed fails the run, so a cell never times
two routes that compute different bits. The report records every arm, every
seed and the comparison outcome; it states nothing the run did not observe.
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys

SEEDS = [0, 1, 0x0123_4567_89AB_CDEF, 0xDEAD_BEEF_CAFE_BABE, 20260913001]
LANES = ["production", "scalar-bit-twiddle", "avx2-bit-twiddle", "avx2-ymm6",
         "avx2-pshufb", "avx2-movemask"]
MASK64 = (1 << 64) - 1


def splitmix64(state):
    while True:
        state = (state + 0x9E37_79B9_7F4A_7C15) & MASK64
        z = state
        z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9) & MASK64
        z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB) & MASK64
        yield z ^ (z >> 31)


def fixture(seed):
    stream = splitmix64(seed)
    return [next(stream) for _ in range(64)]


def naive_transpose(words):
    out = [0] * 64
    for row, word in enumerate(words):
        for col in range(64):
            if (word >> col) & 1:
                out[col] |= 1 << row
    return out


def bit_text(words):
    return "".join(
        "".join("1" if (word >> bit) & 1 else "0" for bit in range(64)) + "\n"
        for word in words
    )


def run(command, environment=None):
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"{command[0]} exited {result.returncode}: {result.stderr.strip()}"
        )
    return result.stdout


def main():
    root = pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--arm",
        default=str(
            root
            / "dev/active/1d4fd63d/external-arms/target/release/transpose-lane-external-arm"
        ),
    )
    parser.add_argument("--survey", default=str(root / "dev/active/6fb89a3c/survey"))
    arguments = parser.parse_args()
    survey = pathlib.Path(arguments.survey)

    checks = []
    failures = 0
    for seed in SEEDS:
        expected = bit_text(naive_transpose(fixture(seed)))
        for lane in LANES:
            environment = dict(os.environ)
            environment["GF2_TRANSPOSE_LANE"] = lane
            produced = run([arguments.arm, "--dump", str(seed)], environment)
            agrees = produced == expected
            failures += not agrees
            checks.append(
                {
                    "arm": f"gf2:{lane}",
                    "seed": seed,
                    "agrees_with_naive": agrees,
                }
            )
        for name in ("m4ri_transpose_arm", "bitshuffle_transpose_arm"):
            executable = survey / name
            produced = run(
                [str(executable), "--dump-check", json.dumps({"n": 64, "seed": seed})]
            )
            agrees = produced == expected
            failures += not agrees
            checks.append(
                {
                    "arm": name,
                    "seed": seed,
                    "agrees_with_naive": agrees,
                }
            )

    report = {
        "schema": "1d4fd63d-bit-mapping-v1",
        "mapping": (
            "word r bit c of the input is matrix entry (r, c); word c bit r of the "
            "output is the same entry; both are little-endian within the word"
        ),
        "fixture": "64 words of SplitMix64 seeded at the case seed",
        "passed": failures == 0,
        "checks": checks,
    }
    output = root / "dev/active/1d4fd63d/survey/bit-mapping-report.json"
    with output.open("w") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(
        f"{len(checks)} comparisons, {failures} disagreements -> {output}",
        file=sys.stderr,
    )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
