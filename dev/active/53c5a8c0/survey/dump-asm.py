#!/usr/bin/env python3
"""Dump release assembly of the measured gf2 paths out of a measured executable.

Usage:
  dump-asm.py --binary <path> --out <artefact.asm.txt>
              [--symbol <exact demangled name>]...
              [--long-product-width <N>]...

The study's gf2 paths are generic and reach the measured executable as
monomorphisations, so `cargo asm` over the library crates cannot see them: a
generic body that no library item instantiates is emitted in the consumer, not
in the crate. This script therefore reads the bytes the receipts pinned. The
executable it is given is the one an arm record names, so the disassembly is
the code that ran rather than a re-emission of it.

Symbol selection:

* `--symbol` takes a demangled name exactly as `nm -C` prints it. A name with
  several definitions is an error, because an artefact that silently picked one
  of them would name a body it cannot identify.
* `--long-product-width N` names one monomorphisation of
  `clmul_crossover_arms::gf2_backend::owned_product` by resolving it through
  the jump table `long_product` dispatches on. `long_product` is a `match` on
  the word count, so its table maps a width to the entry point that serves it;
  the script reads the table's own base and entries out of the file. The
  identification is therefore the executable's own dispatch decision, not a
  guess from a body's size or its address order.

Output shape matches `cargo asm --simplify` closely enough for
`annotate-asm.py` to count it: a `; symbol:` divider per body and one
instruction per line, each line a single leading tab, the mnemonic, and the
operands in Intel syntax. Addresses and encodings are dropped, and a branch
target inside the same body is rewritten to its offset from the body's entry so
the artefact does not move when unrelated code changes the load address.
"""

import argparse
import pathlib
import re
import struct
import subprocess
import sys

LONG_PRODUCT = "clmul_crossover_arms::gf2_backend::long_product"
OWNED_PRODUCT = "clmul_crossover_arms::gf2_backend::owned_product"
# `objdump -d -M intel --no-show-raw-insn` instruction line: address, tab,
# mnemonic, operands, and an optional `# comment` objdump appends for a
# resolved rip-relative reference.
INSN = re.compile(r"^\s*([0-9a-f]+):\t(\S+)(?:\s+([^#]*?))?\s*(?:#.*)?$")
LABEL = re.compile(r"^([0-9a-f]+) <(.+)>:$")
# The `lea reg,[rip+0x...]` that loads a jump table base, and the indexed load
# that reads a table entry.
LEA_RIP = re.compile(r"^lea\s+(\w+),\[rip\+(0x[0-9a-f]+)\]$")
# A branch operand: a hex target, with the symbol objdump resolves it to.
BRANCH = re.compile(r"([0-9a-f]+)(?:\s+<(.+)>)?")
TABLE_LOAD = re.compile(r"^movsxd?\s+\w+,(?:DWORD PTR )?\[(\w+)\+\w+\*4\]$")


def run(*command):
    result = subprocess.run(command, capture_output=True, text=True, check=True)
    return result.stdout


def symbol_table(binary):
    """Every defined symbol of `binary` as name -> [(address, size)]."""
    table = {}
    for line in run("nm", "-C", "-S", "--defined-only", str(binary)).splitlines():
        fields = line.split(" ", 3)
        if len(fields) == 4:
            address, size, _kind, name = fields
            table.setdefault(name.strip(), []).append((int(address, 16), int(size, 16)))
        elif len(fields) == 3:
            address, _kind, name = fields
            table.setdefault(name.strip(), []).append((int(address, 16), 0))
    return table


def unique(table, name):
    """The single definition of `name`, or an error naming the ambiguity."""
    entries = table.get(name)
    if not entries:
        raise SystemExit(f"no symbol {name!r} in the executable")
    if len(entries) > 1:
        places = ", ".join(f"0x{address:x}" for address, _ in sorted(entries))
        raise SystemExit(f"symbol {name!r} has {len(entries)} definitions: {places}")
    return entries[0]


def load_segments(binary):
    """Every PT_LOAD segment as (vaddr, file offset, file size)."""
    segments = []
    data = pathlib.Path(binary).read_bytes()
    if data[:4] != b"\x7fELF" or data[4] != 2:
        raise SystemExit(f"{binary} is not a 64-bit ELF object")
    phoff, = struct.unpack_from("<Q", data, 0x20)
    phentsize, phnum = struct.unpack_from("<HH", data, 0x36)
    for index in range(phnum):
        base = phoff + index * phentsize
        kind, = struct.unpack_from("<I", data, base)
        if kind != 1:
            continue
        offset, vaddr = struct.unpack_from("<QQ", data, base + 0x08)
        filesz, = struct.unpack_from("<Q", data, base + 0x20)
        segments.append((vaddr, offset, filesz))
    return data, segments


def read_at(data, segments, vaddr, length):
    """`length` bytes at virtual address `vaddr`."""
    for base, offset, size in segments:
        if base <= vaddr and vaddr + length <= base + size:
            start = offset + (vaddr - base)
            return data[start : start + length]
    raise SystemExit(f"virtual address 0x{vaddr:x} is in no loaded segment")


def disassemble(binary, address, size):
    """Instruction lines of the body at `address`, in objdump Intel syntax."""
    text = run(
        "objdump", "-d", "-M", "intel", "--demangle", "--no-show-raw-insn",
        f"--start-address=0x{address:x}", f"--stop-address=0x{address + size:x}",
        str(binary),
    )
    lines, inside = [], False
    for line in text.splitlines():
        stripped = line.strip()
        match = LABEL.match(stripped)
        if match:
            inside = int(match.group(1), 16) == address
            continue
        if not inside:
            continue
        match = INSN.match(line)
        if match:
            lines.append((int(match.group(1), 16), match.group(2), (match.group(3) or "").strip()))
    return lines


def jump_table_base(binary, table):
    """Virtual address of the table `long_product` dispatches through.

    Read from `long_product` itself: the register the `lea reg,[rip+d]` loads
    is the one the indexed `movsxd` reads, so the table base is the resolved
    target of that `lea`.
    """
    address, size = unique(table, LONG_PRODUCT)
    body = disassemble(binary, address, size)
    bases = {}
    for offset, mnemonic, operands in body:
        match = LEA_RIP.match(f"{mnemonic} {operands}")
        if match:
            # rip is the address of the next instruction; objdump already
            # resolves that in its comment, but the displacement plus the
            # following address is the same value and needs no comment.
            following = next(
                (o for o, _, _ in body if o > offset), offset
            )
            # objdump prints the displacement as an unsigned 64-bit
            # field; a negative one wraps, so reinterpret it as signed.
            displacement = int(match.group(2), 16)
            if displacement >= 1 << 63:
                displacement -= 1 << 64
            bases[match.group(1)] = following + displacement
        match = TABLE_LOAD.match(f"{mnemonic} {operands}")
        if match and match.group(1) in bases:
            return bases[match.group(1)]
    raise SystemExit(f"{LONG_PRODUCT} dispatches through no readable jump table")


def table_stub(binary, table, body, width):
    """Tail-call stub the jump table names for `width`, or None when the table
    does not cover it.

    The table's extent is the bound of the guard that precedes it: the compare
    whose above-branch skips the indexed jump. A width beyond that bound is
    dispatched some other way.
    """
    base = jump_table_base(binary, table)
    bound = None
    for _, mnemonic, operands in body:
        if mnemonic != "cmp":
            continue
        match = re.fullmatch(r"\w+,(0x[0-9a-f]+|\d+)", operands)
        if match:
            bound = int(match.group(1), 0) + 1
            break
    if bound is None or width > bound:
        return None
    data, segments = load_segments(binary)
    entry, = struct.unpack("<i", read_at(data, segments, base + 4 * (width - 1), 4))
    return base + entry


def compare_stub(body, width):
    """Tail-call stub an explicit `cmp reg, width` dispatches to.

    The compare is followed by a not-equal branch, so the stub is the run of
    instructions that begins at the next address.
    """
    wanted = f"0x{width:x}"
    following = None
    for index, (offset, mnemonic, operands) in enumerate(body):
        if mnemonic != "cmp":
            continue
        match = re.fullmatch(r"\w+,(0x[0-9a-f]+|\d+)", operands)
        if not match or int(match.group(1), 0) != width:
            continue
        rest = body[index + 1 :]
        if rest and rest[0][1].startswith("j"):
            following = rest[1][0] if len(rest) > 1 else None
        if following is not None:
            return following
    raise SystemExit(
        f"{LONG_PRODUCT} dispatches {width} words ({wanted}) through neither "
        "its jump table nor a readable comparison"
    )


def owned_product_at(binary, table, width):
    """The `owned_product` body `long_product` reaches for `width` words.

    A table entry lands on a tail-call stub inside `long_product` that shifts
    the arguments and jumps to the monomorphisation, so the stub's own jump is
    followed to reach the entry point. A width the table does not cover is
    dispatched by an explicit comparison instead, and that comparison's own
    not-taken path leads to the same shape of stub.
    """
    address, size = unique(table, LONG_PRODUCT)
    body = disassemble(binary, address, size)
    stub = table_stub(binary, table, body, width)
    route = "jump table"
    if stub is None:
        stub = compare_stub(body, width)
        route = "explicit comparison"
    bodies = {a: s for a, s in table.get(OWNED_PRODUCT, [])}
    target = None
    for offset, mnemonic, operands in body:
        if offset < stub:
            continue
        match = BRANCH.fullmatch(operands)
        if mnemonic == "jmp" and match:
            target = int(match.group(1), 16)
            break
    if target is None:
        raise SystemExit(
            f"the stub at 0x{stub:x} for {width} words tail-calls nothing readable"
        )
    if target not in bodies:
        places = ", ".join(f"0x{a:x}" for a in sorted(bodies))
        raise SystemExit(
            f"the stub at 0x{stub:x} for {width} words jumps to 0x{target:x}, "
            f"which is no {OWNED_PRODUCT} entry point ({places})"
        )
    return target, bodies[target], route


def render(name, provenance, body):
    """One `; symbol:` section, addresses replaced by entry-relative labels.

    A branch into the same body becomes `.L+0x<offset from entry>`, and every
    such target gets a label line of its own, so the artefact carries the
    body's control flow without carrying its load address.
    """
    entry = body[0][0] if body else 0
    inside = {offset for offset, _, _ in body}
    targets = set()
    for _, _, operands in body:
        match = BRANCH.fullmatch(operands)
        if match and int(match.group(1), 16) in inside:
            targets.add(int(match.group(1), 16))
    out = [
        "",
        ";==========================================================",
        f"; symbol: {name}",
        f"; located: {provenance}",
        ";==========================================================",
        "",
    ]
    for offset, mnemonic, operands in body:
        if offset in targets:
            out.append(f".L+0x{offset - entry:x}:")
        match = BRANCH.fullmatch(operands)
        if match:
            target = int(match.group(1), 16)
            if target in inside:
                operands = f".L+0x{target - entry:x}"
            elif match.group(2):
                operands = match.group(2)
        out.append(f"\t{mnemonic}\t{operands}".rstrip())
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--symbol", action="append", default=[])
    parser.add_argument("--long-product-width", action="append", type=int, default=[])
    arguments = parser.parse_args()

    binary = pathlib.Path(arguments.binary)
    table = symbol_table(binary)
    digest = run("sha256sum", str(binary)).split()[0]

    lines = [
        "; release assembly of the measured gf2 paths (jit:53c5a8c0)",
        ";",
        "; Produced by dev/active/53c5a8c0/survey/dump-asm.py from the bytes of",
        "; the executable named below and from nothing else. Instruction",
        "; addresses are replaced by offsets from each body's entry point, so a",
        "; body that has not changed reproduces byte for byte. A branch into",
        "; the same body reads `.L+0x<offset from that body's entry>`.",
        ";",
        f"; executable   : {binary}",
        f"; sha256       : {digest}",
        f"; objdump      : {run('objdump', '--version').splitlines()[0]}",
        f"; disassembler : objdump -d -M intel --demangle --no-show-raw-insn",
    ]

    sections = []
    for name in arguments.symbol:
        address, size = unique(table, name)
        if size == 0:
            raise SystemExit(f"symbol {name!r} has no recorded size to disassemble")
        sections.append((name, f"nm symbol at 0x{address:x}, {size} bytes", address, size))
    for width in arguments.long_product_width:
        address, size, route = owned_product_at(binary, table, width)
        sections.append((
            f"{OWNED_PRODUCT}::<{width}, {2 * width}>",
            f"long_product {route}, {width}-word entry, tail call to "
            f"0x{address:x}, {size} bytes",
            address,
            size,
        ))
    if not sections:
        raise SystemExit("name at least one --symbol or --long-product-width")

    lines.append(";")
    for name, provenance, _, _ in sections:
        lines.append(f"; body         : {name}")
        lines.append(f";                {provenance}")
    for name, provenance, address, size in sections:
        lines += render(name, provenance, disassemble(binary, address, size))

    out = pathlib.Path(arguments.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n")
    print(f"{out}: {len(sections)} bodies", file=sys.stderr)


if __name__ == "__main__":
    main()
