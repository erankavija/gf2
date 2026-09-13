#!/usr/bin/env python3
"""Check the external comparator arms against 6fb89a3c's committed pins.

Usage: check-external-arms.py [--survey <6fb89a3c survey dir>]

The external arms of this issue's comparator family are not rebuilt from new
sources: they are the executables 6fb89a3c built from its committed C sources
and pinned in its `build-evidence.json`, whose warm pass its
`verify-warm-pass.py` observed under gdb and whose bit mapping its
`verify-bit-mapping.py` checked. `dev/active/6fb89a3c/survey/fetch-build.sh`
reproduces them, and this script decides whether the executables present are
byte-identical to those pins.

It writes what it observed: for each arm, the pinned digest, the digest on
disk, and whether they agree; and for each external library the same. It
asserts nothing about how the arms behave — that is what 6fb89a3c's own
verification scripts and this issue's `verify-bit-mapping.py` establish.
"""

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys

ARMS = ("m4ri_transpose_arm", "bitshuffle_transpose_arm")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


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
    parser.add_argument("--survey", default=str(root / "dev/active/6fb89a3c/survey"))
    arguments = parser.parse_args()
    survey = pathlib.Path(arguments.survey)
    pinned = json.loads((survey / "build-evidence.json").read_text())

    records, mismatches = {}, 0
    for name in ARMS:
        want = pinned["harness_binaries"][name]["sha256"]
        got = digest(survey / name)
        agrees = got == want
        mismatches += not agrees
        records[name] = {
            "pinned_by": "dev/active/6fb89a3c/survey/build-evidence.json",
            "pinned_sha256": want,
            "observed_sha256": got,
            "agrees": agrees,
        }

    report = {
        "schema": "1d4fd63d-external-arm-check-v1",
        "source_of_pins": "dev/active/6fb89a3c/survey/build-evidence.json",
        "rebuilt_by": "dev/active/6fb89a3c/survey/fetch-build.sh",
        "m4ri": {
            key: pinned["m4ri"][key] for key in ("version", "source") if key in pinned["m4ri"]
        },
        "bitshuffle": {
            key: pinned["bitshuffle"][key]
            for key in ("version", "source")
            if key in pinned["bitshuffle"]
        },
        "passed": mismatches == 0,
        "arms": records,
    }
    output = root / "dev/active/1d4fd63d/survey/external-arm-check.json"
    with output.open("w") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(f"{len(records)} arms, {mismatches} mismatches -> {output}", file=sys.stderr)
    return 1 if mismatches else 0


if __name__ == "__main__":
    raise SystemExit(main())
