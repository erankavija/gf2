#!/usr/bin/env python3
"""Digest every function the kernel crate's default release build emits (jit:fa1d8733).

  freeze STAGE  writes `crate-functions-<stage>.json` for `before` or `after`
                through `crate_functions.record`; refuses unless every package
                file other than an assembly listing holds that baseline's
                digest, so the record describes that tree.
  compare       writes `crate-function-comparison.json`, which joins the two
                committed records per function. Each record's
                `sources_sha256` must equal the digest of its baseline's
                source digests. Exits nonzero after writing when a function
                differs or exists on one side only.

Usage: make-crate-functions.py freeze STAGE | compare
"""

import json
import sys

from locate import BASELINES, HERE, ISSUE, PACKAGE_NAME, ROOT, STEP, crate_functions, tracked


def write(name, value):
    output = HERE / name
    output.write_text(json.dumps(value, indent=2) + "\n")
    return output.relative_to(ROOT)


def held(stage):
    return crate_functions.held(
        HERE / f"crate-functions-{stage}.json", BASELINES[stage].digests()
    )


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "freeze" and sys.argv[2] in BASELINES:
        stage = sys.argv[2]
        moved = [
            path for path in BASELINES[stage].changed()
            if not path.endswith(crate_functions.LISTING)
        ]
        if moved:
            raise SystemExit(f"the working tree is not the {stage} tree: {moved}")
        made = crate_functions.record(
            ROOT, PACKAGE_NAME, ISSUE, stage, BASELINES[stage].identity(), tracked("")
        )
        name = f"crate-functions-{stage}.json"
        print(f"{write(name, made)}: {len(made['functions'])} functions")
        return
    if sys.argv[1:] != ["compare"]:
        raise SystemExit(__doc__)
    step = crate_functions.join(STEP, held("before"), held("after"))
    output = write(
        "crate-function-comparison.json",
        {
            "schema": "unroll-removal-crate-function-comparison-v1",
            "issue": ISSUE,
            "comparison_rule": crate_functions.RULE,
            "steps": [step],
        },
    )
    print(f"{output}: {step['function_count']} functions, "
          f"{step['differing_function_count']} differing")
    if step["differing_function_count"]:
        raise SystemExit("a function's instruction text differs across the step")


if __name__ == "__main__":
    main()
