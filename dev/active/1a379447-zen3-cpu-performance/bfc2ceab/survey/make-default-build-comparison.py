#!/usr/bin/env python3
"""Compare the kernel crate's default build with its anchor (jit:bfc2ceab).

Usage: make-default-build-comparison.py freeze-anchor COMMIT | record before|after | compare

Records, through the shared `crate_functions`, the functions a release build of
the kernel crate emits at the anchor, which precedes the entry-point checks,
and in the working tree, and which of them differ. `verification.md` states
what each command writes and refuses.
"""

import json
import sys

from locate import (
    HERE, ISSUE, KERNEL_PACKAGE, ROOT, content_anchor, crate_functions, package, package_files,
)

ANCHOR = content_anchor.Anchor(
    ROOT, HERE / "kernel-anchor-baseline.json"
)
RECORDS = {"before": "crate-functions-before.json", "after": "crate-functions-default.json"}


def write(name, value):
    output = HERE / name
    output.write_text(json.dumps(value, indent=2) + "\n")
    return output.relative_to(ROOT)


def main():
    arguments = sys.argv[1:]
    if len(arguments) == 2 and arguments[0] == "freeze-anchor":
        count = ANCHOR.freeze(
            arguments[1], [package(KERNEL_PACKAGE)], "kernel-crate-anchor-v1", ISSUE
        )
        print(f"{ANCHOR.baseline.relative_to(ROOT)}: {count} files")
        return
    if len(arguments) == 2 and arguments[0] == "record" and arguments[1] in RECORDS:
        tree = arguments[1]
        if tree == "before":
            moved = [path for path in ANCHOR.changed() if not path.endswith(".asm.txt")]
            if moved:
                raise SystemExit(f"the working tree is not the anchor tree: {moved}")
        made = crate_functions.record(
            ROOT, KERNEL_PACKAGE, ISSUE, tree, ANCHOR.identity(), package_files(KERNEL_PACKAGE, "*")
        )
        print(f"{write(RECORDS[tree], made)}: {len(made['functions'])} functions")
        return
    if arguments != ["compare"]:
        raise SystemExit(__doc__)
    before = crate_functions.held(HERE / RECORDS["before"], ANCHOR.digests())
    after = json.loads((HERE / RECORDS["after"]).read_bytes())
    step = crate_functions.join("anchor to the checked entry points, default build", before, after)
    comparison = {
        "schema": "default-build-comparison-v1",
        "issue": ISSUE,
        "comparison_rule": crate_functions.RULE,
        "anchor": ANCHOR.identity(),
        "step": step,
    }
    output = write("default-build-comparison.json", comparison)
    print(f"{output}: {step['function_count']} functions, "
          f"{step['differing_function_count']} differing")


if __name__ == "__main__":
    main()
