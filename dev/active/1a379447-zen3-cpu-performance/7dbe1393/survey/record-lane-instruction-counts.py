#!/usr/bin/env python3
"""Count the instructions each matvec lane executes per stride (jit:7dbe1393).

Usage: record-lane-instruction-counts.py <dense-arm> <dense-campaign>. Runs the
arm's untimed validation dispatch of each frozen cell under callgrind.
"""

import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True

from locate import HERE, LANES, ROOT, campaign  # noqa: E402

RECORD = HERE / "lane-instruction-counts.json"
STRIDES = [9, 10, 62, 63, 66, 32]
CONTROL_STRIDE = 32
SYMBOLS = [
    "gf2_core::matrix::BitMatrix::matvec",
    "gf2_core::matrix::BitMatrix::matvec_with_route",
    "gf2_core::matrix::BitMatrix::matvec_scalar",
    "gf2_core::matrix::BitMatrix::matvec_simd",
    "gf2_core::matrix::BitMatrix::matvec_simd_parity",
    "gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn",
    "gf2_kernels_simd::x86::avx2::fns::and_parity_fn",
    "gf2_kernels_simd::x86::avx2::avx2_and_popcnt",
    "gf2_kernels_simd::x86::avx2::avx2_and_parity",
]


def symbol_addresses(arm):
    """Start address of each lane symbol the executable defines."""
    listing = subprocess.run(
        ["nm", "-C", "--defined-only", arm], capture_output=True, check=True, text=True
    ).stdout
    found = {}
    for line in listing.splitlines():
        address, kind, name = line.split(" ", 2)
        if kind in "tT" and name in SYMBOLS:
            if name in found:
                raise SystemExit(f"{name} is defined more than once")
            found[name] = int(address, 16)
    return found


def counts(profile, addresses):
    """Execution count of every instruction of the lane symbols, by offset."""
    counted = {name: {} for name in addresses}
    current, skip = None, False
    for line in profile.read_text().splitlines():
        if line.startswith("fn="):
            current = line[3:]
            continue
        if line.startswith("calls="):
            # The next line is the call's inclusive cost, not an instruction's own.
            skip = True
            continue
        if not line.startswith("0x"):
            continue
        if skip:
            skip = False
            continue
        if current not in counted:
            continue
        address, _, executed = line.split()
        offset = int(address, 16) - addresses[current]
        counted[current][offset] = counted[current].get(offset, 0) + int(executed)
    return {
        name: [[offset, executed] for offset, executed in sorted(offsets.items())]
        for name, offsets in counted.items()
        if offsets
    }


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    arm, tool = sys.argv[1], sys.argv[2]
    addresses = symbol_addresses(arm)
    valgrind = subprocess.run(
        ["valgrind", "--version"], capture_output=True, check=True, text=True
    ).stdout.strip()
    cases = []
    with tempfile.TemporaryDirectory() as scratch:
        for lane, family, stem, route in LANES:
            addendum = campaign(family)
            declared = {
                cell["cell_id"]: cell
                for cell in json.loads((ROOT / addendum).read_bytes())["cells"]
            }
            for words in STRIDES:
                control = "-control" if words == CONTROL_STRIDE else ""
                cell = stem.format(words=words, control=control)
                size = declared[cell]["workload"]["size"]
                if size["words"] != words:
                    raise SystemExit(f"{cell} declares {size['words']} words")
                request = subprocess.run(
                    [tool, "validation-request", "--family", family, "--addendum", addendum,
                     "--cell", cell, "--arm", route],
                    capture_output=True, check=True,
                ).stdout
                profile = Path(scratch) / f"{lane}-{words}.out"
                subprocess.run(
                    ["valgrind", "--tool=callgrind", "--dump-instr=yes", "--compress-strings=no",
                     "--compress-pos=no", f"--callgrind-out-file={profile}", arm],
                    input=request, capture_output=True, check=True,
                    env={"PATH": os.environ["PATH"], "GF2_DENSE_ROUTE": route,
                         "GF2_TUNING_FRESH_CASE": "child-v2"},
                )
                cases.append({
                    "lane": lane,
                    "family": family,
                    "cell": cell,
                    "arm": route,
                    "stride_words": words,
                    "rows": size["rows"],
                    "request_sha256": hashlib.sha256(request).hexdigest(),
                    "symbols": counts(profile, addresses),
                })
                print(f"{lane} {cell} {route}", file=sys.stderr)
    record = {
        "schema": "lane-instruction-counts-v1",
        "issue": "7dbe1393",
        "arm_sha256": hashlib.sha256(Path(arm).read_bytes()).hexdigest(),
        "tool": valgrind,
        "event": "Ir: executions of each instruction, by offset inside its symbol",
        "cases": cases,
    }
    text = json.dumps(record, separators=(",", ":")).replace('{"lane"', '\n{"lane"') + "\n"
    RECORD.write_text(text)
    print(RECORD.relative_to(ROOT))


if __name__ == "__main__":
    main()
