#!/usr/bin/env python3
"""Count the instructions of the check prototypes (jit:bfc2ceab).

Reads `check-prototype.asm.txt` and the committed kernel listings `review.py`
names, and writes `instruction-counts.json` beside itself: per prototype pair
the instructions the checked form adds under RULE, and per kernel symbol its
loop bodies. An instruction count is no timing.

Usage: make-instruction-counts.py [--self-test]
"""

import hashlib
import json
import re
import sys

import review
from locate import HERE, ISSUE, KERNEL_PACKAGE, ROOT, asm_listing, package_files

RULE = (
    "Per symbol, the instruction text `asm_listing.RULE` defines. A line ending in `:` "
    "is a label; every other line is an instruction. A block is a maximal run of "
    "instructions that no label interrupts and that ends at a jump, a `ret` or the end "
    "of the symbol. A panic block is a block whose last instruction calls a "
    "`core::panicking` function. `instructions` counts every instruction; "
    "`outside_panic_blocks` leaves the panic blocks out, so it counts each loop body "
    "once and both sides of a branch. A loop is a jump to a label that stands earlier in "
    "the symbol; its body is the instructions outside panic blocks from that label "
    "through the jump. `added` is `outside_panic_blocks` of the checked form minus that "
    "of the current form."
)
LISTING = "check-prototype.asm.txt"
LABEL = re.compile(r"^(\S+):$")
JUMP = re.compile(r"^\s*(j[a-z]+)\s+(\S+)$")
PANIC = re.compile(r"^\s*call\b.*core::panicking::")


def blocks(text):
    """The symbol's blocks in order, as (label or None, instruction lines)."""
    found, label, run = [], None, []
    for line in text.splitlines():
        named = LABEL.match(line)
        if named:
            if run or label is not None:
                found.append((label, run))
            label, run = named[1], []
            continue
        run.append(line)
        if JUMP.match(line) or line.strip() == "ret":
            found.append((label, run))
            label, run = None, []
    if run or label is not None:
        found.append((label, run))
    return found


def measure(text):
    """The counts of one symbol under RULE."""
    kept, labels, jumps = [], {}, []
    total = 0
    for label, run in blocks(text):
        total += len(run)
        if label is not None:
            labels[label] = len(kept)
        if run and PANIC.match(run[-1]):
            continue
        for line in run:
            jump = JUMP.match(line)
            if jump:
                jumps.append((len(kept), jump[2]))
            kept.append(line)
    loops = sorted(
        index - labels[target] + 1
        for index, target in jumps
        if target in labels and labels[target] <= index
    )
    return {"instructions": total, "outside_panic_blocks": len(kept), "loop_bodies": loops}


SELF_TEST = """f:
\ttest rdi, rdi
\tje .LBB_3
\txor eax, eax
.LBB_1:
\tcmp qword ptr [rsi + 8*rax], rdx
\tja .LBB_4
\tinc rax
\tcmp rax, rdi
\tjne .LBB_1
.LBB_3:
\tjmp rcx
.LBB_4:
\tpush rax
\tcall qword ptr [rip + core::panicking::panic_fmt@GOTPCREL]"""


def self_test():
    got = measure(SELF_TEST)
    want = {"instructions": 11, "outside_panic_blocks": 9, "loop_bodies": [5]}
    if got != want:
        raise SystemExit(f"self-test: {got}")
    print("self-test: ok")


def kernel_listing(name):
    """Root-relative path of the kernel listing called `name`."""
    found = package_files(KERNEL_PACKAGE, name)
    if len(found) != 1:
        raise SystemExit(f"{len(found)} kernel listings are called {name}; exactly one must be")
    return found[0]


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    listing = (HERE / LISTING).read_text()
    symbols = asm_listing.symbols(listing)

    def prototype(name):
        return measure(symbols[f"{review.PROTOTYPE_CRATE}::{name}"])

    def pair(label, current, checked):
        before, after = prototype(current), prototype(checked)
        return {
            "check": label,
            "current": {"symbol": current} | before,
            "checked": {"symbol": checked} | after,
            "added": after["outside_panic_blocks"] - before["outside_panic_blocks"],
            "added_loop_bodies": after["loop_bodies"],
        }

    rows, kernels = [], {}
    for interface in review.INTERFACES:
        pairs = [pair("safe entry point", *interface["prototype"])]
        pairs += [pair(*extra) for extra in interface.get("prototype_extra", [])]
        row = {"interface": interface["id"], "class": interface["class"], "pairs": pairs}
        if "kernel_listing" in interface:
            name, symbol = interface["kernel_listing"]
            path = kernel_listing(name)
            text = (ROOT / path).read_text()
            kernels[path] = hashlib.sha256(text.encode()).hexdigest()
            row["kernel"] = {"listing": path, "symbol": symbol} | measure(
                asm_listing.symbols(text)[symbol]
            )
        rows.append(row)
    record = {
        "schema": "check-instruction-counts-v1",
        "issue": ISSUE,
        "rule": RULE,
        "prototype_listing": {
            "path": str((HERE / LISTING).relative_to(ROOT)),
            "instruction_text_sha256": asm_listing.digest(
                "\n".join(f"{name}\n{text}" for name, text in sorted(symbols.items()))
            ),
        },
        "kernel_listings": [
            {"path": path, "sha256": digest} for path, digest in sorted(kernels.items())
        ],
        "interfaces": rows,
    }
    output = HERE / "instruction-counts.json"
    output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{output.relative_to(ROOT)}: {len(rows)} interfaces")


if __name__ == "__main__":
    main()
