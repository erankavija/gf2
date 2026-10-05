#!/usr/bin/env python3
"""Compare the `BitMatrix::matvec` assembly before and after `matvec_with_route` (jit:63bad95d).

Reads the four listings `regen-asm.sh` wrote under `asm/`: the default and the
`simd` build of `gf2-core`, each before and after the change. Writes
`matvec-asm-comparison.json` beside itself with the shared listing rule's
verdict per symbol and fails when a symbol of a `before` listing differs or is
missing afterwards. A symbol only the `after` listing holds is listed as added.

Usage: make-matvec-asm-comparison.py, with no arguments
"""

import hashlib
import json

from locate import HERE, ROOT

import asm_listing  # noqa: E402  (beside the shared `repository_files`, which `locate` puts on the path)


def main():
    builds, differing = [], 0
    for build in ("default", "simd"):
        paths = {stage: HERE / "asm" / f"matvec-{build}-{stage}.asm.txt"
                 for stage in ("before", "after")}
        texts = {stage: path.read_text() for stage, path in paths.items()}
        before = asm_listing.symbols(texts["before"])
        after = asm_listing.symbols(texts["after"])
        labels = {
            stage: {selector: body.splitlines()[0].rstrip(":") for selector, body in found.items()}
            for stage, found in (("before", before), ("after", after))
        }
        rows = []
        for selector, body in before.items():
            same = after.get(selector) == body
            differing += not same
            rows.append(
                {
                    "selector": selector,
                    "function": labels["before"][selector],
                    "instruction_lines": len(body.splitlines()),
                    "sha256": asm_listing.digest(body),
                    "instruction_text": "same" if same else "differs",
                }
            )
        builds.append(
            {
                "build": build,
                "features": asm_listing.features(texts["after"]),
                "listings": {
                    stage: {
                        "path": str(path.relative_to(ROOT)),
                        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                    }
                    for stage, path in paths.items()
                },
                "symbols": rows,
                "added": [labels["after"][selector] for selector in after if selector not in before],
            }
        )
    output = HERE / "matvec-asm-comparison.json"
    output.write_text(
        json.dumps(
            {"schema": "matvec-asm-comparison-v1", "issue": "63bad95d",
             "comparison_rule": asm_listing.RULE, "builds": builds},
            indent=1,
        )
        + "\n"
    )
    for build in builds:
        for row in build["symbols"]:
            print(build["build"], row["function"], row["instruction_text"])
        print(build["build"], "added:", ", ".join(build["added"]))
    if differing:
        raise SystemExit(f"{differing} symbols differ from their baseline")


if __name__ == "__main__":
    main()
