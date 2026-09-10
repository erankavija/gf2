#!/usr/bin/env python3
"""Compare two builds of one arm executable: file, sections and functions.

Usage: dev/active/6fb89a3c/survey/compare-arm-code.py <old-elf> <new-elf> [function ...]

Prints both SHA-256 digests, the digest of every allocated section, the
sections that hold differing bytes when the files have equal size, and for
each named function whether its machine code is identical once addresses
are normalized (call and jump targets become symbol names, intra-function
offsets and RIP-relative displacements are masked, alignment padding is
dropped), together with its address in both builds. With no function named
it lists every function whose code or address changed and counts the rest.
The survey uses it to
state what changed in an arm executable between a family's pilot and its
confirmation.
"""
from __future__ import annotations

import hashlib
import re
import subprocess
import sys
from pathlib import Path

HEADER = re.compile(r"^([0-9a-f]+) <(.+)>:$")
INSTRUCTION = re.compile(r"^\s*[0-9a-f]+:\s+(.*)$")
SECTION = re.compile(r"^\s*\[\s*\d+\]\s+(\S+)\s+\S+\s+([0-9a-f]+)\s+([0-9a-f]+)\s+([0-9a-f]+)\s+\S+\s+(\S*)")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sections(path: Path) -> list[tuple[str, int, int, str]]:
    """(name, file offset, size, flags) of every section with file bytes."""
    listing = subprocess.run(["readelf", "-S", "-W", str(path)], capture_output=True, text=True, check=True).stdout
    found = []
    for line in listing.splitlines():
        match = SECTION.match(line)
        if match and match.group(1) != "NULL":
            found.append((match.group(1), int(match.group(3), 16), int(match.group(4), 16), match.group(5)))
    return found


def functions(path: Path) -> dict[str, tuple[int, list[str]]]:
    listing = subprocess.run(["objdump", "-d", "--no-show-raw-insn", str(path)],
                             capture_output=True, text=True, check=True).stdout
    found: dict[str, tuple[int, list[str]]] = {}
    body: list[str] | None = None
    for line in listing.splitlines():
        header = HEADER.match(line)
        if header:
            body = []
            found.setdefault(header.group(2), (int(header.group(1), 16), body))
            continue
        instruction = INSTRUCTION.match(line)
        if instruction and body is not None:
            text = re.sub(r"\b[0-9a-f]+ <", "<", instruction.group(1))
            text = re.sub(r"<([^>+]+)\+0x[0-9a-f]+>", r"<\1+off>", text)
            text = re.sub(r"-?0x[0-9a-f]+\(%rip\)", "REL(%rip)", text)
            text = re.sub(r"#.*$", "", text).strip()
            # Alignment padding after the last instruction is never executed.
            if not re.fullmatch(r"(cs |data16 )*(nop[wl]?|xchg %ax,%ax)\b.*", text):
                body.append(text)
    return found


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    old, new = Path(sys.argv[1]), Path(sys.argv[2])
    old_bytes, new_bytes = old.read_bytes(), new.read_bytes()
    print(f"old sha256 {sha256(old_bytes)} size {len(old_bytes)}")
    print(f"new sha256 {sha256(new_bytes)} size {len(new_bytes)}")
    old_sections, new_sections = sections(old), sections(new)
    new_by_name = {name: (offset, size) for name, offset, size, _ in new_sections}
    print("sections (name: identical|differs, old size -> new size):")
    for name, offset, size, flags in old_sections:
        if "A" not in flags or name not in new_by_name or name.startswith(".bss"):
            continue
        new_offset, new_size = new_by_name[name]
        same = old_bytes[offset:offset + size] == new_bytes[new_offset:new_offset + new_size]
        print(f"  {name}: {'identical' if same else 'differs'}, {size} -> {new_size}")
    if len(old_bytes) == len(new_bytes):
        differing = [i for i, (a, b) in enumerate(zip(old_bytes, new_bytes)) if a != b]
        placed: dict[str, int] = {}
        for index in differing:
            owner = next((name for name, offset, size, _ in old_sections if offset <= index < offset + size), "<no section>")
            placed[owner] = placed.get(owner, 0) + 1
        print(f"differing bytes {len(differing)}: " + ", ".join(f"{name} {count}" for name, count in placed.items()))
    old_functions, new_functions = functions(old), functions(new)
    named = sys.argv[3:]
    names = named or sorted(set(old_functions) | set(new_functions))
    print("functions (name: identical|differs|absent, old address -> new address):")
    unchanged = 0
    for name in names:
        before, after = old_functions.get(name), new_functions.get(name)
        if before is None or after is None:
            print(f"  {name}: absent in {'old' if before is None else 'new'} build")
            continue
        status = "identical" if before[1] == after[1] else "differs"
        if not named and status == "identical" and before[0] == after[0]:
            unchanged += 1
            continue
        print(f"  {name}: {status}, {before[0]:#x} -> {after[0]:#x} ({len(before[1])} instructions)")
    if not named:
        print(f"  the other {unchanged} functions: identical code at the same address")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
