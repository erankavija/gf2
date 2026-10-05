#!/usr/bin/env python3
"""Compare the kernel crate's sources and listings across this task's change (jit:7d44b71f).

The change has two steps, which `locate.py` pins by content: from `anchor` to
`before-1b034786`, and from `after-1b034786` to `contracts-complete`. The step
between the two is the code change of jit:1b034786 and is outside this record,
as is every change after `contracts-complete`.

Writes `asm-comparison.json` beside itself. Per step:

  sources   every package path whose bytes differ across the step, with one
            class: `comment-or-blank-only` or `code-differs` for a Rust source
            under `rust_code_text.RULE`, `assembly-listing` for a listing,
            `other` otherwise
  listings  every assembly listing of either side, split per symbol, with the
            digest of each symbol's instruction text under `asm_listing.RULE`
            on both sides

Exits nonzero after writing when a source is not `comment-or-blank-only` or a
symbol's instruction text differs.

Usage: make-asm-comparison.py
"""

import json

from locate import (
    BASELINES,
    HERE,
    ISSUE,
    ROOT,
    STEPS,
    asm_listing,
    digest_changes,
    rust_code_text,
)

LISTING = ".asm.txt"


def source_class(path, before, after):
    """`before` and `after` are the path's bytes on each side, `None` where absent."""
    if path.endswith(LISTING):
        return "assembly-listing"
    if path.endswith(".rs") and before is not None and after is not None:
        same = rust_code_text.code_text(before.decode()) == rust_code_text.code_text(after.decode())
        return "comment-or-blank-only" if same else "code-differs"
    return "other"


def step(name, changed, paths, before, after):
    """One step's record; `before` and `after` map a path to its bytes or `None`."""
    sources = [{"path": path, "class": source_class(path, before(path), after(path))} for path in changed]
    counts = {}
    for entry in sources:
        counts[entry["class"]] = counts.get(entry["class"], 0) + 1
    listings, symbols, differing = [], 0, 0
    for artefact in sorted(path for path in paths if path.endswith(LISTING)):
        old, new = before(artefact), after(artefact)
        rows = asm_listing.compare((old or b"").decode(), (new or b"").decode())
        symbols += len(rows)
        differing += sum(row["instruction_text"] != "same" for row in rows)
        listings.append({"artefact": artefact, "regenerated": artefact in changed, "symbols": rows})
    return {
        "step": name,
        "source_class_counts": counts,
        "symbol_count": symbols,
        "differing_symbol_count": differing,
        "sources": sources,
        "listings": listings,
    }


def baseline_bytes(baseline):
    digests = baseline.digests()
    return lambda path: baseline.bytes(path) if path in digests else None


def main():
    steps = [
        step(
            name,
            digest_changes(BASELINES[first], BASELINES[last]),
            set(BASELINES[first].digests()) | set(BASELINES[last].digests()),
            baseline_bytes(BASELINES[first]),
            baseline_bytes(BASELINES[last]),
        )
        for name, first, last in STEPS
    ]
    baselines = {stage: baseline.identity() for stage, baseline in BASELINES.items()}
    output = HERE / "asm-comparison.json"
    output.write_text(
        json.dumps(
            {
                "schema": "safety-contract-asm-comparison-v2",
                "issue": ISSUE,
                "baselines": baselines,
                "excluded_step": "before-1b034786 to after-1b034786, the code change of jit:1b034786",
                "classifier_rule": rust_code_text.RULE,
                "comparison_rule": asm_listing.RULE,
                "steps": steps,
            },
            indent=2,
        )
        + "\n"
    )
    unexpected, differing = [], 0
    for entry in steps:
        print(
            f"{output.relative_to(ROOT)}: {entry['step']}: {json.dumps(entry['source_class_counts'])}, "
            f"{entry['symbol_count']} symbols, {entry['differing_symbol_count']} differing"
        )
        unexpected += [e["path"] for e in entry["sources"] if e["class"] in ("code-differs", "other")]
        differing += entry["differing_symbol_count"]
    if unexpected:
        raise SystemExit(f"changed package paths outside comments and listings: {unexpected}")
    if differing:
        raise SystemExit("a symbol's instruction text differs across a step")


if __name__ == "__main__":
    main()
