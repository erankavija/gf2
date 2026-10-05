#!/usr/bin/env python3
"""Compare the kernel crate's sources and listings across this task's change (jit:7d44b71f).

The change has two steps, which `locate.py` pins by content: from `anchor` to
`before-1b034786`, and from `after-1b034786` to `contracts-complete`. The step
between the two is the code change of jit:1b034786 and is outside this record,
as is every change after `contracts-complete`.

Writes `asm-comparison.json` beside itself. Per step:

  sources   every package path whose bytes differ across the step, with its
            class under `asm_listing.source_class`
  listings  every assembly listing of either side, split per symbol, with the
            digest of each symbol's instruction text under `asm_listing.RULE`
            on both sides

Exits nonzero after writing when a source is not `comment-or-blank-only` or a
symbol's instruction text differs.

Usage: make-asm-comparison.py
"""

import json

from locate import BASELINES, HERE, ISSUE, ROOT, STEPS, asm_listing, content_anchor, rust_code_text


def main():
    steps = [
        asm_listing.step(
            name,
            content_anchor.digest_changes(BASELINES[first], BASELINES[last]),
            set(BASELINES[first].digests()) | set(BASELINES[last].digests()),
            BASELINES[first].bytes_or_none,
            BASELINES[last].bytes_or_none,
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
