#!/usr/bin/env python3
"""Digest every function the kernel crate's release build emits (jit:7d44b71f).

Builds `gf2-kernels-simd` with `--emit=asm` under the toolchain the environment
selects and digests each function's instruction text under
`crate_functions.RULE`.

  freeze STAGE  writes `crate-functions-<stage>.json` for one of the baselines
                `locate.py` names; refuses unless every package file other than
                an assembly listing holds that baseline's digest, so the record
                describes that tree.
  compare       writes `crate-function-comparison.json`, which joins the
                committed records per function across this task's two steps.
                Each record's `sources_sha256` must equal the digest of its
                baseline's source digests, which ties the record to that tree
                by content. Exits nonzero after writing when a function
                differs across a step or exists on one side only.

Usage: make-crate-functions.py freeze STAGE | compare
"""

import json
import sys

from locate import BASELINES, HERE, ISSUE, PACKAGE_NAME, ROOT, STEPS, crate_functions, tracked


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
        moved = [path for path in BASELINES[stage].changed() if not path.endswith(".asm.txt")]
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
    steps = [
        crate_functions.join(name, held(first), held(last)) for name, first, last in STEPS
    ]
    comparison = {
        "schema": "crate-function-comparison-v2",
        "issue": ISSUE,
        "comparison_rule": crate_functions.RULE,
        "excluded_step": "before-1b034786 to after-1b034786, the code change of jit:1b034786",
        "steps": steps,
    }
    output = write("crate-function-comparison.json", comparison)
    for entry in steps:
        print(f"{output}: {entry['step']}: {entry['function_count']} functions, "
              f"{entry['differing_function_count']} differing")
    if any(entry["differing_function_count"] for entry in steps):
        raise SystemExit("a function's instruction text differs across a step")


if __name__ == "__main__":
    main()
