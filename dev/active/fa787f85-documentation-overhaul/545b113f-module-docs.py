#!/usr/bin/env python3
"""Prints the record of module docs longer than twelve lines.

Usage: 545b113f-module-docs.py > 545b113f-module-docs.md, from any directory
of the checkout. Scans the leading `//!` block of every tracked Rust file
under `crates/` and `dev/tools/` and joins each block over twelve lines with
its row of `545b113f-module-doc-contracts.tsv`. Exits with status 1 when the
scan and the rows name different files.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(subprocess.run(["git", "-C", str(HERE), "rev-parse", "--show-toplevel"],
                           capture_output=True, check=True, text=True).stdout.strip())
LIMIT = 12


def leading_doc_lines(path: Path) -> int:
    count = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("//!"):
            count += 1
        elif count:
            break
    return count


files = subprocess.run(["git", "-C", str(ROOT), "ls-files", "crates/**/*.rs", "dev/tools/**/*.rs"],
                       capture_output=True, check=True, text=True).stdout.split()
long_docs = {f: n for f in files if (n := leading_doc_lines(ROOT / f)) > LIMIT}
contracts = dict(line.split("\t", 1) for line in
                 (HERE / "545b113f-module-doc-contracts.tsv").read_text(encoding="utf-8").splitlines())
if long_docs.keys() != contracts.keys():
    sys.exit(f"scan and contract rows differ: {sorted(long_docs.keys() ^ contracts.keys())}")

print(f"""# Module docs longer than twelve lines (JIT 545b113f)

Each tracked Rust file under `crates/` and `dev/tools/` whose leading `//!`
block exceeds {LIMIT} lines, with the contract each retained section states.
`{Path(__file__).name}` beside this record prints it; the contract column is
`545b113f-module-doc-contracts.tsv`.

| Lines | File | Retained section and its contract |
|---:|---|---|""")
for name, count in sorted(long_docs.items(), key=lambda item: (-item[1], item[0])):
    print(f"| {count} | `{name}` | {contracts[name]} |")
