"""Reads the per-symbol assembly listings `regen-asm.sh` writes.

A listing is a banner followed by one `; symbol:` divider and body per symbol.
`compare` reports, per symbol, whether two listings hold the same instruction
text under RULE.
"""

from __future__ import annotations

import hashlib
import re

RULE = (
    "Per symbol, the lines between its `; symbol:` divider and the next divider, "
    "without blank lines, divider rules and cargo's `Compiling` and `Finished` "
    "status lines. Compiler-numbered local names are reduced to their kind: "
    "`.LBB<n>_`, `.LCPI<n>_` and `.Lanon.<hash>.<n>`; their numbers and hashes "
    "follow the crate's function count and source line positions. Mnemonics, "
    "operands, registers, immediates and label suffixes are compared verbatim."
)
SYMBOL = re.compile(r"^; symbol: (\S+)(?: \(index (\d+)\))?$", re.M)
LOCAL = [
    (re.compile(r"\.LBB\d+_"), ".LBB_"),
    (re.compile(r"\.LCPI\d+_"), ".LCPI_"),
    (re.compile(r"\.Lanon\.[0-9a-f]+\.\d+"), ".Lanon"),
]
STATUS = re.compile(r"^\s*(Compiling|Finished) |^;=+$")
RUSTFLAGS = re.compile(r"^; RUSTFLAGS     : (.*)$", re.M)
UNSET = "<empty>"


def symbols(artefact: str) -> dict[str, str]:
    """Symbol selector to its instruction text under RULE."""
    parts = SYMBOL.split(artefact)[1:]
    found = {}
    for name, index, body in zip(parts[::3], parts[1::3], parts[2::3]):
        lines = [line for line in body.splitlines() if line.strip() and not STATUS.search(line)]
        text = "\n".join(lines)
        for pattern, kind in LOCAL:
            text = pattern.sub(kind, text)
        found[name if index is None else f"{name}#{index}"] = text
    return found


def selectors(artefact: str) -> list[str]:
    """The listing's symbols as `regen-asm.sh` arguments, in listing order."""
    return [
        name if index is None else f"{name}#{index}" for name, index in SYMBOL.findall(artefact)
    ]


def rustflags(artefact: str) -> str:
    """The `EXTRA_RUSTFLAGS` the banner records; empty when it records none."""
    found = RUSTFLAGS.search(artefact)
    return "" if found is None or found[1] == UNSET else found[1]


def digest(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def compare(before: str, after: str) -> list[dict]:
    """One row per symbol of either listing, sorted by symbol."""
    old_symbols, new_symbols = symbols(before), symbols(after)
    rows = []
    for name in sorted(set(old_symbols) | set(new_symbols)):
        old, new = old_symbols.get(name), new_symbols.get(name)
        rows.append(
            {
                "symbol": name,
                "anchor_lines": None if old is None else len(old.splitlines()),
                "current_lines": None if new is None else len(new.splitlines()),
                "anchor_sha256": None if old is None else digest(old),
                "current_sha256": None if new is None else digest(new),
                "instruction_text": "same" if old is not None and old == new else "differs",
            }
        )
    return rows
