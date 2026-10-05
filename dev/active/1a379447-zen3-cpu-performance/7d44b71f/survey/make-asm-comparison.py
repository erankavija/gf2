#!/usr/bin/env python3
"""Compare the kernel crate's sources and listings with the task anchor (jit:7d44b71f).

Writes `asm-comparison.json` beside itself from the anchor that
`anchor-baseline.json` identifies and the working tree:

  sources   every package path whose bytes differ from the anchor, with one
            class: `comment-or-blank-only` or `code-differs` for a Rust source
            under `rust_code_text.RULE`, `assembly-listing` for a listing,
            `other` otherwise
  listings  every assembly listing of either tree, split per symbol, with the
            digest of each symbol's instruction text under `asm_listing.RULE`
            on both sides

Exits nonzero after writing when a source is not `comment-or-blank-only` or a
symbol's instruction text differs.

Usage: make-asm-comparison.py
"""

import json

from locate import ANCHOR, HERE, ISSUE, ROOT, asm_listing, rust_code_text, tracked

LISTING = ".asm.txt"


def source_class(path):
    if path.endswith(LISTING):
        return "assembly-listing"
    current = ROOT / path
    if path.endswith(".rs") and path in ANCHOR.digests() and current.is_file():
        same = rust_code_text.code_text(ANCHOR.bytes(path).decode()) == rust_code_text.code_text(
            current.read_text()
        )
        return "comment-or-blank-only" if same else "code-differs"
    return "other"


def main():
    changed = ANCHOR.changed()
    sources = [{"path": path, "class": source_class(path)} for path in changed]
    counts = {}
    for entry in sources:
        counts[entry["class"]] = counts.get(entry["class"], 0) + 1

    anchored = {path for path in ANCHOR.digests() if path.endswith(LISTING)}
    listings, symbols, differing = [], 0, 0
    for artefact in sorted(anchored | set(tracked(LISTING))):
        before = ANCHOR.bytes(artefact).decode() if artefact in anchored else ""
        current = ROOT / artefact
        rows = asm_listing.compare(before, current.read_text() if current.is_file() else "")
        symbols += len(rows)
        differing += sum(row["instruction_text"] != "same" for row in rows)
        listings.append(
            {
                "artefact": artefact,
                "regenerated": artefact in changed,
                "symbols": rows,
            }
        )

    output = HERE / "asm-comparison.json"
    output.write_text(
        json.dumps(
            {
                "schema": "safety-contract-asm-comparison-v1",
                "issue": ISSUE,
                "anchor": ANCHOR.identity(),
                "classifier_rule": rust_code_text.RULE,
                "comparison_rule": asm_listing.RULE,
                "source_class_counts": counts,
                "symbol_count": symbols,
                "differing_symbol_count": differing,
                "sources": sources,
                "listings": listings,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"{output.relative_to(ROOT)}: {json.dumps(counts)}, {symbols} symbols, {differing} differing")
    unexpected = [e["path"] for e in sources if e["class"] in ("code-differs", "other")]
    if unexpected:
        raise SystemExit(f"changed package paths outside comments and listings: {unexpected}")
    if differing:
        raise SystemExit("a symbol's instruction text differs from the anchor listing")


if __name__ == "__main__":
    main()
