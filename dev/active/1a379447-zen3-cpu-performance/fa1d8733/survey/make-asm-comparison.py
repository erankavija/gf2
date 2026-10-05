#!/usr/bin/env python3
"""Compare the kernel crate's sources and listings across the removal (jit:fa1d8733).

Writes `asm-comparison.json` beside itself for the step from `before` to
`after`, the two trees `locate.py` pins by content: every package path whose
bytes differ, with its class under `asm_listing.source_class`, and every
assembly listing of either tree, split per symbol, with the digest of each
symbol's instruction text under `asm_listing.RULE` on both sides.

Exits nonzero after writing when a symbol's instruction text differs or a
symbol exists on one side only.

Usage: make-asm-comparison.py
"""

import json

from locate import BASELINES, HERE, ISSUE, ROOT, STEP, asm_listing, content_anchor, rust_code_text


def main():
    before, after = BASELINES["before"], BASELINES["after"]
    step = asm_listing.step(
        STEP,
        content_anchor.digest_changes(before, after),
        set(before.digests()) | set(after.digests()),
        before.bytes_or_none,
        after.bytes_or_none,
    )
    output = HERE / "asm-comparison.json"
    output.write_text(
        json.dumps(
            {
                "schema": "unroll-removal-asm-comparison-v1",
                "issue": ISSUE,
                "baselines": {stage: baseline.identity() for stage, baseline in BASELINES.items()},
                "classifier_rule": rust_code_text.RULE,
                "comparison_rule": asm_listing.RULE,
                "steps": [step],
            },
            indent=2,
        )
        + "\n"
    )
    print(
        f"{output.relative_to(ROOT)}: {json.dumps(step['source_class_counts'])}, "
        f"{step['symbol_count']} symbols, {step['differing_symbol_count']} differing"
    )
    if step["differing_symbol_count"]:
        raise SystemExit("a symbol's instruction text differs across the step")


if __name__ == "__main__":
    main()
