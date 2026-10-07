#!/usr/bin/env python3
"""Resolve the unnamed offsets of a profile session's reports (jit:53c5a8c0).

Usage: resolve-symbols.py <profile-dir>

`perf report` names a sample by the object it falls in and the symbol that
object's dynamic symbol table covers. A shared library whose hot routine is a
local symbol - a resolver-selected string or memory routine, for one - offers no
dynamic symbol for it, so the report carries the offset instead. The offset is
not a dead end: the object's own symbol table, which `objdump -t` reads, carries
the local symbol and its extent.

This script reads every `report/*.txt` of the session, collects each
`<object> 0x<offset>` row, resolves the offset against the object the session's
`host.txt` records for that name, and writes `symbol-resolution.json` beside the
reports: the object's absolute path, its SHA-256, the offset, the covering
symbol, and the offset into that symbol.

The object path comes from the session's own `host.txt`, which records what the
arms' loader resolves, so the resolution names the object the session ran rather
than whatever a later search would find. A digest is recorded with it, so a
reader can check the object has not been replaced. An offset no symbol covers is
recorded with a null symbol rather than guessed.
"""

import hashlib
import json
import os
import re
import subprocess
import sys

REPORT_ROW = re.compile(r"^\s+[0-9.]+%\s+\d+\s+(\S+)\s+\[.\]\s+0x([0-9a-f]+)\b")
LDD_ROW = re.compile(r"^\s*(\S+)\s+=>\s+(/\S+)")
# `objdump -t`: value, flags, section, size, name.
OBJDUMP_ROW = re.compile(r"^([0-9a-f]+)\s+(\S+)\s+\S+\s+(\S+)\s+([0-9a-f]+)\s+(.+)$")


def objects_from_host(path):
    """Object base name -> absolute path, from the session's host record."""
    resolved = {}
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            match = LDD_ROW.match(line)
            if match:
                resolved.setdefault(os.path.basename(match.group(2)), match.group(2))
    return resolved


def symbols(path):
    """Every sized function symbol of `path` as (address, size, name)."""
    out = []
    text = subprocess.run(
        ["objdump", "-t", path], capture_output=True, text=True, check=True
    ).stdout
    for line in text.splitlines():
        match = OBJDUMP_ROW.match(line)
        if not match:
            continue
        address, _flags, _section, size, name = match.groups()
        size = int(size, 16)
        if size:
            out.append((int(address, 16), size, name.strip()))
    out.sort()
    return out


def covering(table, offset):
    """The symbol whose extent contains `offset`, or None."""
    for address, size, name in table:
        if address <= offset < address + size:
            return address, size, name
    return None


def digest(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(block)
    return hasher.hexdigest()


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    directory = os.path.abspath(sys.argv[1])
    resolved_objects = objects_from_host(os.path.join(directory, "host.txt"))

    wanted = {}
    reports = os.path.join(directory, "report")
    for name in sorted(os.listdir(reports)):
        if not name.endswith(".txt"):
            continue
        with open(os.path.join(reports, name), encoding="utf-8") as handle:
            for line in handle:
                match = REPORT_ROW.match(line)
                if match:
                    wanted.setdefault(match.group(1), set()).add(int(match.group(2), 16))

    tables, digests, record = {}, {}, []
    for obj in sorted(wanted):
        path = resolved_objects.get(obj)
        if path is None or not os.path.isfile(path):
            for offset in sorted(wanted[obj]):
                record.append({
                    "object": obj, "object_path": path, "object_sha256": None,
                    "offset": "0x{:x}".format(offset), "symbol": None,
                    "offset_into_symbol": None,
                    "note": "the session's host record resolves no readable path for this object",
                })
            continue
        if obj not in tables:
            tables[obj] = symbols(path)
            digests[obj] = digest(path)
        for offset in sorted(wanted[obj]):
            found = covering(tables[obj], offset)
            record.append({
                "object": obj,
                "object_path": path,
                "object_sha256": digests[obj],
                "offset": "0x{:x}".format(offset),
                "symbol": found[2] if found else None,
                "offset_into_symbol": "0x{:x}".format(offset - found[0]) if found else None,
                "symbol_size": found[1] if found else None,
                "note": None if found else "no sized symbol of this object covers the offset",
            })

    out = os.path.join(directory, "symbol-resolution.json")
    with open(out, "w", encoding="utf-8") as handle:
        json.dump({
            "schema": "clmul-crossover-symbol-resolution-v1",
            "objdump": subprocess.run(
                ["objdump", "--version"], capture_output=True, text=True, check=True
            ).stdout.splitlines()[0],
            "entries": record,
        }, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print("{}: {} offsets".format(out, len(record)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
