#!/usr/bin/env python3
"""Compare the kernel crate's default build across the checked wrappers (jit:bfc2ceab).

The checked wrappers of the caller-trusted entry points are compiled under
`--cfg gf2_entry_checked` alone. This script shows that a build without the
cfg emits the functions it emitted without them, with the shared
`crate_functions` records.

  freeze-anchor COMMIT  writes `kernel-anchor-baseline.json`, the digest of
                        every kernel-crate file at COMMIT, and the anchor
                        bytes of the paths the working tree changes under
                        `inputs/kernel-anchor`. The commit id is informational.
  record before         writes `crate-functions-before.json`; refuses unless
                        the working tree holds the anchor's kernel crate.
  record after          writes `crate-functions-default.json` for the working
                        tree.
  compare               writes `default-build-comparison.json`; exits nonzero
                        after writing when a function's instruction text
                        differs or a function exists on one side only.

Usage: make-default-build-comparison.py freeze-anchor COMMIT | record before|after | compare
"""

import json
import sys

from locate import HERE, ISSUE, KERNEL_PACKAGE, ROOT, content_anchor, crate_functions, package

ANCHOR = content_anchor.Anchor(
    ROOT, HERE / "kernel-anchor-baseline.json", HERE / "inputs" / "kernel-anchor"
)
RECORDS = {"before": "crate-functions-before.json", "after": "crate-functions-default.json"}


def write(name, value):
    output = HERE / name
    output.write_text(json.dumps(value, indent=2) + "\n")
    return output.relative_to(ROOT)


def kernel_files():
    """Sorted tracked and untracked, non-ignored files of the kernel crate."""
    import subprocess

    listing = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "--cached", "--others", "--exclude-standard",
         "--", package(KERNEL_PACKAGE)],
        capture_output=True, check=True, text=True,
    ).stdout.split()
    return sorted(listing)


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
            ROOT, KERNEL_PACKAGE, ISSUE, tree, ANCHOR.identity(), kernel_files()
        )
        print(f"{write(RECORDS[tree], made)}: {len(made['functions'])} functions")
        return
    if arguments != ["compare"]:
        raise SystemExit(__doc__)
    before = crate_functions.held(HERE / RECORDS["before"], ANCHOR.digests())
    after = json.loads((HERE / RECORDS["after"]).read_bytes())
    step = crate_functions.join("anchor to the checked wrappers, default build", before, after)
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
    if step["differing_function_count"]:
        raise SystemExit("a function's instruction text differs across the step")


if __name__ == "__main__":
    main()
