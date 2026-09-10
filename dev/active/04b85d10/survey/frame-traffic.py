#!/usr/bin/env python3
"""Frame traffic per disassembled routine of the profile (jit:04b85d10).

Counts the instructions each disassembled routine uses to write a register
into its own stack frame and to read one back, and joins each count with the
source-derived reason the routine has that traffic. The counter alone cannot
separate a compiler spill from a buffer the algorithm is defined over, so the
reason comes from the classification table, whose every row cites the
declaration that settles it.

Usage: frame-traffic.py <asm-dir> <classification-file>
"""

import pathlib
import re
import sys

STORE = re.compile(r"^\s+[0-9a-f]+:\s+v?mov[a-z]*\s+%[a-z0-9]+,-?0x[0-9a-f]+\(%rsp\)")
LOAD = re.compile(r"^\s+[0-9a-f]+:\s+v?mov[a-z]*\s+-?0x[0-9a-f]+\(%rsp\),%[a-z0-9]+")
INSTRUCTION = re.compile(r"^\s+[0-9a-f]+:")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    asm_dir = pathlib.Path(sys.argv[1])
    classification = {}
    with open(sys.argv[2], encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            label, kind, evidence = line.split("|", 2)
            classification[label] = (kind, evidence)

    print("# Frame traffic per disassembled routine (jit:04b85d10)")
    print("#")
    print("# One instance of each routine is disassembled and bounded by its")
    print("# own symbol size, so a count is the cost of one call rather than")
    print("# of the copies link-time optimization emitted. `stores` counts")
    print("# instructions writing a register into the routine's own frame and")
    print("# `loads` those reading one back. The reason column separates the")
    print("# two costs the counter cannot: a compiler spill of a value that")
    print("# did not fit in a register, and a buffer the algorithm is defined")
    print("# over. Its evidence is the source declaration that settles it.")
    print()
    print(f"{'routine':<24}{'stores':>8}{'loads':>7}{'instr':>7}  reason")
    for path in sorted(asm_dir.glob("*.txt")):
        label = path.stem
        if label in ("symbols", "frame-traffic", "popcnt-attribution"):
            continue
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        stores = sum(1 for line in lines if STORE.match(line))
        loads = sum(1 for line in lines if LOAD.match(line))
        total = sum(1 for line in lines if INSTRUCTION.match(line))
        kind, evidence = classification.get(
            label, ("unclassified", "no row in the classification table")
        )
        print(f"{label:<24}{stores:>8}{loads:>7}{total:>7}  {kind}: {evidence}")


if __name__ == "__main__":
    main()
