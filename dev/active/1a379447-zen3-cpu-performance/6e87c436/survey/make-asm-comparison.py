#!/usr/bin/env python3
"""Compare a kernel module's annotated release assembly across this task (jit:6e87c436).

For each SIMD source of `gf2-kernels-simd` whose bytes differ from the task
anchor, which `anchor-baseline.json` identifies by per-path digest, the script
reads the module's `asm/<module>.asm.txt` artefact from the anchor snapshot and
from the working tree, splits both at their `; symbol:` dividers and
writes `asm-comparison.json` beside itself with each symbol's instruction text
digests. Any symbol whose text differs under RULE fails the script after the
record is written.

Usage: make-asm-comparison.py
"""

import hashlib
import json
import pathlib
import re

from locate import HERE, ROOT, anchor_bytes, anchor_identity, changed_since_anchor, repository_files

RULE = (
    "Per symbol, the lines between its `; symbol:` divider and the next divider, "
    "without blank lines, divider rules and cargo's `Compiling` and `Finished` "
    "status lines. Compiler-numbered local names are reduced to their kind: "
    "`.LBB<n>_`, `.LCPI<n>_` and `.Lanon.<hash>.<n>`; their numbers and hashes "
    "follow the crate's function count and source line positions. Mnemonics, "
    "operands, registers, immediates and label suffixes are compared verbatim."
)
SYMBOL = re.compile(r"^; symbol: (\S+)$", re.M)
LOCAL = [
    (re.compile(r"\.LBB\d+_"), ".LBB_"),
    (re.compile(r"\.LCPI\d+_"), ".LCPI_"),
    (re.compile(r"\.Lanon\.[0-9a-f]+\.\d+"), ".Lanon"),
]
STATUS = re.compile(r"^\s*(Compiling|Finished) |^;=+$")


def symbols(artefact):
    """Symbol name to its instruction text under RULE."""
    parts = SYMBOL.split(artefact)[1:]
    found = {}
    for name, body in zip(parts[::2], parts[1::2]):
        lines = [line for line in body.splitlines() if line.strip() and not STATUS.search(line)]
        text = "\n".join(lines)
        for pattern, kind in LOCAL:
            text = pattern.sub(kind, text)
        found[name] = text
    return found


def digest(text):
    return hashlib.sha256(text.encode()).hexdigest()


def main():
    package = pathlib.Path(repository_files.package_directory(ROOT, "gf2-kernels-simd"))
    changed = [
        pathlib.Path(path)
        for path in changed_since_anchor()
        if path.startswith(f"{package}/src/") and path.endswith(".rs")
    ]
    modules, differing = [], 0
    for source in changed:
        artefact = source.parent / "asm" / f"{source.stem}.asm.txt"
        before = symbols(anchor_bytes(str(artefact)).decode())
        after = symbols((ROOT / artefact).read_text())
        rows = []
        for name in sorted(set(before) | set(after)):
            old, new = before.get(name), after.get(name)
            same = old is not None and old == new
            differing += not same
            rows.append(
                {
                    "symbol": name,
                    "anchor_lines": None if old is None else len(old.splitlines()),
                    "current_lines": None if new is None else len(new.splitlines()),
                    "anchor_sha256": None if old is None else digest(old),
                    "current_sha256": None if new is None else digest(new),
                    "instruction_text": "same" if same else "differs",
                }
            )
        modules.append(
            {
                "source": str(source),
                "artefact": str(artefact),
                "symbols": rows,
            }
        )
    output = HERE / "asm-comparison.json"
    output.write_text(
        json.dumps(
            {
                "schema": "dense-asm-comparison-v1",
                "issue": "6e87c436",
                "anchor": anchor_identity(),
                "comparison_rule": RULE,
                "modules": modules,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"{output.relative_to(ROOT)}: {len(modules)} modules, {differing} differing symbols")
    if differing:
        raise SystemExit("a symbol's instruction text differs from the anchor artefact")


if __name__ == "__main__":
    main()
