#!/usr/bin/env python3
"""Append a derived frame-traffic and instruction-mix annotation to an asm artefact.

Usage: annotate-asm.py <artefact.asm.txt> [...]

`dev/scripts/regen-asm.sh` writes the disassembly; this script reads those
bytes back and appends one annotation block per symbol. Every number it prints
is counted from the artefact it is given. It states no measured time, no host
condition and no figure typed by hand, and it is idempotent: an existing
annotation block is removed before the new one is appended, so re-running it
over an unchanged artefact reproduces the file byte for byte.

What each row means:

- `stack frame bytes` is the operand of the symbol's `sub rsp, N`, or 0 when
  it has none. It is the frame the routine reserves, whatever it holds.
- `block copies` counts calls to `memcpy`. A 64-word block copied into or out
  of a frame slot appears here rather than as individual stores.
- `frame stores` and `frame loads` count instructions whose destination or
  source operand addresses memory through `rsp`. `lea` is excluded: it
  computes an address without touching memory.
- The mnemonic rows count exact mnemonics, so the classification is checkable
  against the artefact by grep.

Frame traffic alone does not say whether a routine declares a buffer or the
compiler spilled: that is what the routine's source says, and the report reads
the two together.
"""

import pathlib
import re
import sys

MARKER = ";=== derived annotation "
SYMBOL = re.compile(r"^; symbol: (.+)$")
SUB_RSP = re.compile(r"^\s*sub\s+rsp,\s*(\d+)\s*$")
# An instruction line from cargo-show-asm --simplify: one leading tab, a
# mnemonic, and optional operands. Labels end in ':' and directives start '.'.
INSTRUCTION = re.compile(r"^\t([a-z][a-z0-9]*)\s*(.*)$")

MNEMONIC_ROWS = [
    ("vmovdqu", "256-bit unaligned moves"),
    ("vpxor", "vector XOR"),
    ("vpand", "vector AND"),
    ("vpsllq", "vector shift left (quadword)"),
    ("vpsrlq", "vector shift right (quadword)"),
    ("vpblendd", "vector blend (doubleword)"),
    ("vpermq", "cross-lane quadword permute"),
    ("vpshufd", "in-lane doubleword shuffle"),
    ("vpshufb", "in-lane byte shuffle"),
    ("vpmovmskb", "byte sign-bit extraction"),
    ("vpaddb", "vector byte add"),
    ("vpunpcklbw", "byte interleave (low)"),
    ("vpunpckhbw", "byte interleave (high)"),
    ("vpunpcklwd", "word interleave (low)"),
    ("vpunpckhwd", "word interleave (high)"),
    ("vpunpckldq", "doubleword interleave (low)"),
    ("vpunpckhdq", "doubleword interleave (high)"),
    ("vmovq", "64-bit vector move"),
    ("call", "calls"),
]


def split_operands(operands):
    """Left and right of the first comma outside brackets."""
    depth = 0
    for index, char in enumerate(operands):
        if char == "[":
            depth += 1
        elif char == "]":
            depth -= 1
        elif char == "," and depth == 0:
            return operands[:index], operands[index + 1 :]
    return operands, ""


def section_report(name, lines):
    frame_bytes = 0
    stores = loads = total = 0
    calls = []
    counts = {}
    for line in lines:
        match = SUB_RSP.match(line)
        if match:
            frame_bytes = max(frame_bytes, int(match.group(1)))
        match = INSTRUCTION.match(line.rstrip("\n"))
        if not match:
            continue
        mnemonic, operands = match.group(1), match.group(2)
        total += 1
        counts[mnemonic] = counts.get(mnemonic, 0) + 1
        if mnemonic == "call":
            calls.append(operands.strip())
        if mnemonic != "lea":
            destination, source = split_operands(operands)
            if "[rsp" in destination:
                stores += 1
            if "[rsp" in source:
                loads += 1
    memcpy = sum(1 for target in calls if "memcpy" in target)
    rows = [
        ("instructions", total),
        ("stack frame bytes", frame_bytes),
        ("block copies (memcpy)", memcpy),
        ("frame stores", stores),
        ("frame loads", loads),
    ]
    rows += [
        (f"{mnemonic} ({gloss})", counts[mnemonic])
        for mnemonic, gloss in MNEMONIC_ROWS
        if counts.get(mnemonic)
    ]
    width = max(len(label) for label, _ in rows)
    body = [f"; {label.ljust(width)}  {value}" for label, value in rows]
    return [f"{MARKER}for {name} ===", ";"] + body + [";"]


def annotate(path):
    text = pathlib.Path(path).read_text()
    kept = []
    for line in text.splitlines():
        if line.startswith(MARKER):
            break
        kept.append(line)
    while kept and not kept[-1].strip():
        kept.pop()

    sections, current, name = [], [], None
    for line in kept:
        match = SYMBOL.match(line)
        if match:
            if name is not None:
                sections.append((name, current))
            name, current = match.group(1), []
        elif name is not None:
            current.append(line)
    if name is not None:
        sections.append((name, current))
    if not sections:
        raise SystemExit(f"{path}: no '; symbol:' section to annotate")

    out = list(kept)
    out += [
        "",
        "",
        f"{MARKER}block ===",
        ";==========================================================",
        "; Counted from the disassembly above by",
        "; dev/active/1d4fd63d/survey/annotate-asm.py, which reads only these",
        "; bytes. See that script's module docstring for what each row counts.",
        ";==========================================================",
    ]
    for section_name, lines in sections:
        out.append("")
        out.extend(section_report(section_name, lines))
    pathlib.Path(path).write_text("\n".join(out) + "\n")
    print(f"annotated {path}: {len(sections)} symbol(s)")


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    for path in sys.argv[1:]:
        annotate(path)


if __name__ == "__main__":
    main()
