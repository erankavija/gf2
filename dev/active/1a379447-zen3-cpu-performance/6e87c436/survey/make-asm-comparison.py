#!/usr/bin/env python3
"""Compare a kernel module's annotated release assembly across this task (jit:6e87c436).

For each SIMD source of `gf2-kernels-simd` whose bytes differ from the task
anchor, which `anchor-baseline.json` identifies by per-path digest, the script
reads the module's `asm/<module>.asm.txt` artefact from the anchor snapshot and
from the working tree, splits both at their `; symbol:` dividers and
writes `asm-comparison.json` beside itself with each symbol's instruction text
digests. Any symbol whose text differs under `asm_listing.RULE` fails the script after the
record is written.

Refuses to write on a tree that differs from the one
`dense-verdict-end-state.json` pins.

Usage: make-asm-comparison.py
"""

import json
import pathlib

from locate import ANCHOR, HERE, ROOT, asm_listing, end_state, repository_files


def main():
    end_state().require_matching_tree("asm-comparison.json")
    package = pathlib.Path(repository_files.package_directory(ROOT, "gf2-kernels-simd"))
    changed = [
        pathlib.Path(path)
        for path in ANCHOR.changed()
        if path.startswith(f"{package}/src/") and path.endswith(".rs")
    ]
    modules, differing = [], 0
    for source in changed:
        artefact = source.parent / "asm" / f"{source.stem}.asm.txt"
        rows = asm_listing.compare(
            ANCHOR.bytes(str(artefact)).decode(), (ROOT / artefact).read_text()
        )
        differing += sum(row["instruction_text"] != "same" for row in rows)
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
                "anchor": ANCHOR.identity(),
                "comparison_rule": asm_listing.RULE,
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
