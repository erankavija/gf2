#!/usr/bin/env python3
"""Tabulate what each matvec lane executes per row and stride (jit:7dbe1393).

Usage: make-asm-attribution.py, with no arguments. Untimed.

The table joins two committed records of one executable: the assembly record
`asm/simd/` (written by the dense story's `disassemble-dense.sh --out`) and
the execution counts `lane-instruction-counts.json` (written by
`record-lane-instruction-counts.py`). Loops are the strongly connected
components of each symbol's control-flow graph, nested by removing a
component's entry instructions; nothing is classified by hand. Every figure is
an execution count divided by the cell's row count. The script fails when the
two records name different executables or a counted offset is not an
instruction of the listing.
"""

import json
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True

from locate import HERE, ROOT  # noqa: E402

ASM = HERE / "asm" / "simd"
COUNTS = HERE / "lane-instruction-counts.json"
OUTPUT = HERE.parent / "asm-attribution-tables.md"

INSTRUCTION = re.compile(r"^\s+([0-9a-f]+):\t(\S+)\s*(.*)$")
HEADER = re.compile(r"^## (\S.*) at ([0-9a-f]+), 0x([0-9a-f]+) bytes$")
TARGET = re.compile(r"^([0-9a-f]+) <")
# Mnemonics whose per-row execution count names a method: the multiply of the
# portable word count, the nibble lookup and its byte sum, the masked load and
# the parity flag.
MARKERS = ["imul", "vpshufb", "vpsadbw", "vpmaskmovq", "setnp", "call"]
ROW_LOOPS = {
    "scalar": "gf2_core::matrix::BitMatrix::matvec_scalar",
    "simd": "gf2_core::matrix::BitMatrix::matvec_simd",
    "parity-kernel": "gf2_core::matrix::BitMatrix::matvec_simd_parity",
}
KERNELS = {
    "simd": ["gf2_kernels_simd::x86::avx2::fns::and_popcnt_fn",
             "gf2_kernels_simd::x86::avx2::avx2_and_popcnt"],
    "parity-kernel": ["gf2_kernels_simd::x86::avx2::fns::and_parity_fn",
                      "gf2_kernels_simd::x86::avx2::avx2_and_parity"],
    "scalar": [],
}
ENTRIES = ["gf2_core::matrix::BitMatrix::matvec",
           "gf2_core::matrix::BitMatrix::matvec_with_route"]


def listings():
    """Every symbol of the assembly record: its instructions by offset."""
    symbols = {}
    for path in sorted(ASM.glob("*.asm.txt")):
        name = start = None
        for line in path.read_text().splitlines():
            header = HEADER.match(line)
            if header:
                name, start = header.group(1), int(header.group(2), 16)
                symbols[name] = {"end": int(header.group(3), 16), "instructions": {}}
                continue
            found = INSTRUCTION.match(line)
            if found and name:
                address = int(found.group(1), 16)
                target = TARGET.match(found.group(3))
                symbols[name]["instructions"][address - start] = {
                    "mnemonic": found.group(2),
                    "target": int(target.group(1), 16) - start if target else None,
                }
    return symbols


def successors(symbol):
    """Control-flow edges between the instructions of one symbol."""
    offsets = sorted(symbol["instructions"])
    following = dict(zip(offsets, offsets[1:]))
    edges = {}
    for offset in offsets:
        instruction = symbol["instructions"][offset]
        mnemonic, target = instruction["mnemonic"], instruction["target"]
        out = []
        jump = mnemonic.startswith("j")
        if jump and target in symbol["instructions"]:
            out.append(target)
        if mnemonic not in ("jmp", "ret", "ud2") and offset in following:
            out.append(following[offset])
        edges[offset] = out
    return edges


def components(nodes, edges):
    """Strongly connected components of the subgraph on `nodes` that hold a cycle."""
    index, low, stack, on_stack, found = {}, {}, [], set(), []
    for root in sorted(nodes):
        if root in index:
            continue
        work = [(root, iter([n for n in edges[root] if n in nodes]))]
        index[root] = low[root] = len(index)
        stack.append(root)
        on_stack.add(root)
        while work:
            node, pending = work[-1]
            advanced = False
            for successor in pending:
                if successor not in index:
                    index[successor] = low[successor] = len(index)
                    stack.append(successor)
                    on_stack.add(successor)
                    work.append((successor, iter([n for n in edges[successor] if n in nodes])))
                    advanced = True
                    break
                if successor in on_stack:
                    low[node] = min(low[node], index[successor])
            if advanced:
                continue
            work.pop()
            if work:
                low[work[-1][0]] = min(low[work[-1][0]], low[node])
            if low[node] == index[node]:
                component = set()
                while True:
                    member = stack.pop()
                    on_stack.discard(member)
                    component.add(member)
                    if member == node:
                        break
                if len(component) > 1 or node in edges[node]:
                    found.append(component)
    return found


def loops(symbol):
    """Loops of one symbol, outermost first: (depth, instruction offsets)."""
    edges = successors(symbol)
    entry = min(symbol["instructions"])
    nested = []

    def descend(nodes, depth):
        for component in sorted(components(nodes, edges), key=min):
            nested.append((depth, component))
            entries = {
                node for node in component
                if node == entry or any(
                    node in edges[other] for other in edges if other not in component
                )
            }
            descend(component - entries, depth + 1)

    descend(set(symbol["instructions"]), 0)
    return nested


def per_row(value, rows):
    return f"{value / rows:.2f}"


def main():
    if len(sys.argv) != 1:
        raise SystemExit(__doc__)
    record = json.loads(COUNTS.read_bytes())
    index = (ASM / "index.txt").read_text()
    digest = re.search(r"^# sha256: ([0-9a-f]{64})$", index, re.MULTILINE).group(1)
    if digest != record["arm_sha256"]:
        raise SystemExit("the assembly record and the counts name different executables")
    symbols = listings()
    nesting = {name: loops(symbol) for name, symbol in symbols.items()}

    summary, by_loop, entries = [], [], []
    for case in record["cases"]:
        lane, words, rows = case["lane"], case["stride_words"], case["rows"]
        executed = {}
        for name, pairs in case["symbols"].items():
            for offset, count in pairs:
                if offset not in symbols[name]["instructions"]:
                    raise SystemExit(f"{name}+{offset:#x} is not a listed instruction")
            executed[name] = dict(pairs)
        row_symbol = ROW_LOOPS[lane]
        if executed[row_symbol].get(min(symbols[row_symbol]["instructions"])) != 1:
            raise SystemExit(f"{case['cell']}: the lane body is not entered exactly once")

        def inside(name, depth_at_least):
            members = set()
            for depth, component in nesting[name]:
                if depth >= depth_at_least:
                    members |= component
            return members

        # The row loop is the lane body's outermost loop; a loop nested in it,
        # and every loop of a kernel, is a loop over the words of one row.
        row_body = sum(
            count for offset, count in executed[row_symbol].items()
            if offset in inside(row_symbol, 0) and offset not in inside(row_symbol, 1)
        )
        word_loops = sum(
            count for offset, count in executed[row_symbol].items()
            if offset in inside(row_symbol, 1)
        )
        once = sum(
            count for offset, count in executed[row_symbol].items()
            if offset not in inside(row_symbol, 0)
        )
        straight = 0
        for name in KERNELS[lane]:
            members = inside(name, 0)
            word_loops += sum(c for o, c in executed.get(name, {}).items() if o in members)
            straight += sum(c for o, c in executed.get(name, {}).items() if o not in members)
        markers = {marker: 0 for marker in MARKERS}
        lane_symbols = [row_symbol, *KERNELS[lane]]
        for name in lane_symbols:
            for offset, count in executed.get(name, {}).items():
                mnemonic = symbols[name]["instructions"][offset]["mnemonic"]
                if mnemonic in markers:
                    markers[mnemonic] += count
        total = row_body + word_loops + straight
        summary.append(
            f"| {lane} | {words} | {per_row(row_body, rows)} | {per_row(word_loops, rows)} | "
            f"{per_row(straight, rows)} | {per_row(total, rows)} | "
            + " | ".join(per_row(markers[marker], rows) for marker in MARKERS)
            + f" | {once} |"
        )

        for name in lane_symbols:
            for depth, component in nesting[name]:
                if name == row_symbol and depth == 0:
                    continue
                counted = executed.get(name, {})
                header = min(component)
                trips = max((counted.get(offset, 0) for offset in component), default=0)
                body_markers = sorted({
                    symbols[name]["instructions"][offset]["mnemonic"] for offset in component
                } & {"imul", "vpshufb", "vpsadbw", "vpand", "vpxor", "pand", "pxor",
                     "vmovdqu", "movdqu", "vpmaskmovq"})
                by_loop.append(
                    f"| {lane} | {words} | `{name.rsplit('::', 1)[1]}+{header:#x}` | "
                    f"{len(component)} | {per_row(trips, rows)} | "
                    f"{per_row(sum(counted.get(o, 0) for o in component), rows)} | "
                    f"{', '.join(f'`{m}`' for m in body_markers)} |"
                )
        for name in ENTRIES:
            if name in executed:
                entries.append(
                    f"| {lane} | {words} | `{name.rsplit('::', 1)[1]}` | "
                    f"{sum(executed[name].values())} |"
                )

    sizes = [
        f"| `{name}` | {len(symbol['instructions'])} | {symbol['end']} | "
        f"{sum(1 for depth, _ in nesting[name] if depth == 0)} |"
        for name, symbol in sorted(symbols.items())
        if name in set(ROW_LOOPS.values()) | {k for names in KERNELS.values() for k in names}
        | set(ENTRIES)
    ]
    lines = [
        "# Matvec lane instruction attribution",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-asm-attribution.py` from the assembly record",
        "[`survey/asm/simd/`](survey/asm/simd/index.txt) and the execution counts",
        "[`survey/lane-instruction-counts.json`](survey/lane-instruction-counts.json) of the",
        f"executable with SHA-256 `{digest}`, counted by `{record['tool']}`.",
        "",
        "Every per-row figure is the execution count of the named instructions in one call of",
        "the allocated product on a 1024-row matrix, divided by the row count. The counts are",
        "exact and carry no time: an instruction count is no latency estimate.",
        "",
        "## Instructions per row",
        "",
        "*Row body* is the lane body's row loop outside any loop over the words of a row: the",
        "bundle call, the result test and the output append on the two kernel lanes, and on the",
        "scalar lane also its unrolled tail words, its reduction and its word count, which that",
        "lane inlines. *Word loops* are the loops over the words of one row. *Kernel",
        "straight-line* is the kernel's code outside its loops, the bundle entry included. The",
        "marker columns count the executions of one mnemonic in the lane's symbols per row.",
        "*Once per call* counts the lane body's instructions outside its row loop, per call",
        "and not per row.",
        "",
        "| Lane | Stride (words) | Row body | Word loops | Kernel straight-line | Total | "
        + " | ".join(f"`{marker}`" for marker in MARKERS) + " | Once per call |",
        "|---|---:|---:|---:|---:|---:|" + "---:|" * len(MARKERS) + "---:|",
        *summary,
        "",
        "## Loops over the words of a row",
        "",
        "One row per loop of the lane's symbols, named by its first instruction's offset.",
        "*Trips per row* is the execution count of the loop's most executed instruction.",
        "",
        "| Lane | Stride (words) | Loop | Instructions in the loop | Trips per row | "
        "Instructions per row | Vector and count mnemonics |",
        "|---|---:|---|---:|---:|---:|---|",
        *by_loop,
        "",
        "## Entries",
        "",
        "Instructions the entry symbol executes per call. An entry absent from a lane's rows",
        "is inlined into the arm.",
        "",
        "| Lane | Stride (words) | Entry symbol | Instructions per call |",
        "|---|---:|---|---:|",
        *entries,
        "",
        "## Symbols",
        "",
        "| Symbol | Instructions | Bytes | Outermost loops |",
        "|---|---:|---:|---:|",
        *sizes,
        "",
    ]
    OUTPUT.write_text("\n".join(lines))
    print(OUTPUT.relative_to(ROOT))


if __name__ == "__main__":
    main()
