#!/usr/bin/env python3
"""Append a derived frame-traffic and instruction-mix annotation to an asm artefact.

Usage: annotate-asm.py <artefact.asm.txt> [...]

`dev/active/53c5a8c0/survey/dump-asm.py` writes the disassembly; this script
reads those bytes back and appends one annotation block per body. Every number
it prints is counted from the artefact it is given. It states no measured time,
no host condition and no figure typed by hand, and it is idempotent: an existing
annotation block is removed before the new one is appended, so re-running it
over an unchanged artefact reproduces the file byte for byte.

What each row means:

- `stack frame bytes` sums the operands of the body's `sub rsp, N`. A frame
  past a page is reserved as a sequence of page-sized subtractions with a probe
  store between them, so the sum is the frame and the largest single operand is
  not. It is the frame the routine reserves, whatever it holds.
- `block copies` counts calls whose target names `memcpy`. A word block copied
  into or out of a frame slot appears here rather than as individual stores.
- `frame stores` and `frame loads` count instructions whose destination or
  source operand addresses memory through `rsp`. `lea` is excluded: it computes
  an address without touching memory.
- `backward branches` counts branches whose target is at or below the branch,
  which is the count of loop closures in the body.
- The mnemonic rows count exact mnemonics, so the classification is checkable
  against the artefact by grep. A mnemonic the artefact does not contain has no
  row.

Frame traffic alone does not say whether a routine declares a buffer or the
compiler spilled: that is what the routine's source says, and the report reads
the two together.
"""

import pathlib
import re
import sys

MARKER = ";=== derived annotation "
SYMBOL = re.compile(r"^; symbol: (.+)$")
SUB_RSP = re.compile(r"^\tsub\trsp,(0x[0-9a-f]+|\d+)$")
# An instruction line from dump-asm.py: one leading tab, a mnemonic, a tab and
# the operands. Dividers start with ';'.
INSTRUCTION = re.compile(r"^\t([a-z][a-z0-9]*)(?:\t(.*))?$")
LOCAL_TARGET = re.compile(r"^\.L\+0x([0-9a-f]+)$")
LOCAL_LABEL = re.compile(r"^(\.L\+0x[0-9a-f]+):$")

MNEMONIC_ROWS = [
    # Binutils prints a carry-less multiply under a mnemonic that encodes the
    # immediate's two halves, so each selector is its own row.
    ("pclmullqlqdq", "carry-less multiply, low x low"),
    ("pclmulhqlqdq", "carry-less multiply, high x low"),
    ("pclmullqhqdq", "carry-less multiply, low x high"),
    ("pclmulhqhqdq", "carry-less multiply, high x high"),
    ("pclmulqdq", "carry-less multiply, immediate selector"),
    ("vpclmullqlqdq", "vector carry-less multiply, low x low"),
    ("vpclmulhqlqdq", "vector carry-less multiply, high x low"),
    ("vpclmullqhqdq", "vector carry-less multiply, low x high"),
    ("vpclmulhqhqdq", "vector carry-less multiply, high x high"),
    ("vpclmulqdq", "vector carry-less multiply, immediate selector"),
    ("tzcnt", "trailing-zero count (bit-serial step)"),
    ("blsr", "clear lowest set bit (bit-serial step)"),
    ("shlx", "variable shift left"),
    ("shld", "double-precision shift left"),
    ("xor", "scalar XOR"),
    ("vpxor", "vector XOR"),
    ("vpternlogq", "three-input vector logic"),
    ("vpslldq", "vector byte shift left"),
    ("vpsrldq", "vector byte shift right"),
    ("vpalignr", "vector byte align"),
    ("vpunpcklqdq", "quadword interleave (low)"),
    ("vpunpckhqdq", "quadword interleave (high)"),
    ("vpermq", "cross-lane quadword permute"),
    ("vinserti128", "128-bit lane insert"),
    ("vextracti128", "128-bit lane extract"),
    ("vmovdqu", "unaligned vector move"),
    ("vmovdqa", "aligned vector move"),
    ("vmovups", "unaligned vector move (fp-typed)"),
    ("vmovq", "64-bit vector move"),
    ("vzeroupper", "upper-lane zeroing"),
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
    stores = loads = total = backward = 0
    calls = []
    counts = {}
    # Instruction index of each local label, so a branch is backward when its
    # target is at or before the branch itself.
    labels = {}
    index = 0
    for line in lines:
        match = LOCAL_LABEL.match(line.rstrip("\n"))
        if match:
            labels[match.group(1)] = index
        elif INSTRUCTION.match(line.rstrip("\n")):
            index += 1
    index = 0
    for line in lines:
        match = SUB_RSP.match(line)
        if match:
            frame_bytes += int(match.group(1), 0)
        match = INSTRUCTION.match(line.rstrip("\n"))
        if not match:
            continue
        mnemonic, operands = match.group(1), match.group(2) or ""
        total += 1
        index += 1
        counts[mnemonic] = counts.get(mnemonic, 0) + 1
        if mnemonic == "call":
            calls.append(operands.strip())
        target = labels.get(operands.strip())
        if target is not None and mnemonic[0] == "j" and target < index:
            backward += 1
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
        ("backward branches", backward),
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
        "; dev/active/53c5a8c0/survey/annotate-asm.py, which reads only these",
        "; bytes. See that script's module docstring for what each row counts.",
        ";==========================================================",
    ]
    for section_name, lines in sections:
        out += [""] + section_report(section_name, lines)
    pathlib.Path(path).write_text("\n".join(out) + "\n")
    print(f"{path}: {len(sections)} bodies annotated", file=sys.stderr)


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    for path in sys.argv[1:]:
        annotate(path)


if __name__ == "__main__":
    main()
