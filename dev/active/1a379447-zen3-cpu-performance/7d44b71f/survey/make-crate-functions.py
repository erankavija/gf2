#!/usr/bin/env python3
"""Digest every function the kernel crate's release build emits (jit:7d44b71f).

Builds `gf2-kernels-simd` with `--emit=asm` under the toolchain the environment
selects and digests each function's instruction text under RULE.

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

import hashlib
import json
import re
import subprocess
import sys

from locate import BASELINES, HERE, ISSUE, PACKAGE_NAME, ROOT, STEPS, repository_files, tracked

RULE = (
    "Per `@function` symbol of the emitted assembly, the lines from its label "
    "to its `.Lfunc_end` label, without assembler directives. Compiler-numbered "
    "local names are reduced to their kind: `.LBB<n>_`, `.LCPI<n>_`, `.LJTI<n>_`, "
    "`.Ltmp<n>`, `.Lfunc_begin<n>`, `.Lexception<n>` and `.Lanon.<hash>.<n>`; "
    "their numbers and hashes follow the crate's function count and source line "
    "positions. Mnemonics, operands, registers, immediates and label suffixes "
    "are compared verbatim."
)
LOCAL = [
    (re.compile(r"\.(LBB|LCPI|LJTI)\d+_"), r".\1_"),
    (re.compile(r"\.L(tmp|func_begin|exception)\d+"), r".L\1"),
    (re.compile(r"\.Lanon\.[0-9a-f]+\.\d+"), ".Lanon"),
]
FUNCTION = re.compile(r"^\t\.type\t(\S+),@function\n", re.M)
DIRECTIVE = re.compile(r"^\s*\.[A-Za-z_0-9.]+(\s|$)")
BUILD = ["cargo", "rustc", "--release", "-p", PACKAGE_NAME, "--lib"]
EMIT = ["--", "--emit=asm"]


def emitted():
    """The assembly text of a fresh release build of the package."""
    target = ROOT / "target" / f"{ISSUE}-emit"
    budget = ROOT / repository_files.live_file(ROOT, "cargo-budget.sh")
    subprocess.run(
        [str(budget), *BUILD, "--target-dir", str(target), *EMIT], cwd=ROOT, check=True
    )
    stem = PACKAGE_NAME.replace("-", "_")
    found = sorted(
        (target / "release" / "deps").glob(f"{stem}-*.s"), key=lambda p: p.stat().st_mtime
    )
    return found[-1].read_text()


def functions(assembly):
    """Function symbol to the digest of its instruction text under RULE."""
    digests = {}
    for found in FUNCTION.finditer(assembly):
        name = found[1]
        start = assembly.index(f"\n{name}:\n", found.end() - 1) + 1
        end = assembly.index("\n.Lfunc_end", start)
        lines = [
            line for line in assembly[start:end].splitlines()
            if line.strip() and not DIRECTIVE.match(line)
        ]
        text = "\n".join(lines)
        for pattern, kind in LOCAL:
            text = pattern.sub(kind, text)
        digests[name] = hashlib.sha256(text.encode()).hexdigest()
    return dict(sorted(digests.items()))


def sources_digest(digests):
    """Digest of the per-path digests of the package files other than listings."""
    sources = {path: held for path, held in digests.items() if not path.endswith(".asm.txt")}
    return hashlib.sha256(json.dumps(sources, sort_keys=True).encode()).hexdigest()


def record(tree, baseline):
    sources = {
        path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in tracked("")
    }
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, check=True, text=True)
    return {
        "schema": "crate-function-digests-v1",
        "issue": ISSUE,
        "tree": tree,
        "command": " ".join(BUILD + EMIT),
        "rustc": rustc.stdout.strip(),
        "rule": RULE,
        "anchor": baseline.identity(),
        "sources_sha256": sources_digest(sources),
        "functions": functions(emitted()),
    }


def write(name, value):
    output = HERE / name
    output.write_text(json.dumps(value, indent=2) + "\n")
    return output.relative_to(ROOT)


def join(name, before, after):
    """One step's rows; fails on a toolchain mismatch between its two records."""
    if before["rustc"] != after["rustc"]:
        raise SystemExit(f"{name}: built with {before['rustc']} and with {after['rustc']}")
    old, new = before["functions"], after["functions"]
    rows = [
        {
            "function": function,
            "before_sha256": old.get(function),
            "after_sha256": new.get(function),
            "instruction_text": (
                "same" if function in old and old[function] == new.get(function) else "differs"
            ),
        }
        for function in sorted(set(old) | set(new))
    ]
    return {
        "step": name,
        "rustc": after["rustc"],
        "before_sources_sha256": before["sources_sha256"],
        "after_sources_sha256": after["sources_sha256"],
        "function_count": len(rows),
        "differing_function_count": sum(row["instruction_text"] != "same" for row in rows),
        "functions": rows,
    }


def held(stage):
    """The committed record of `stage`, checked against that baseline's source digests."""
    found = json.loads((HERE / f"crate-functions-{stage}.json").read_bytes())
    if found["sources_sha256"] != sources_digest(BASELINES[stage].digests()):
        raise SystemExit(f"crate-functions-{stage}.json does not describe the {stage} sources")
    return found


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "freeze" and sys.argv[2] in BASELINES:
        stage = sys.argv[2]
        moved = [path for path in BASELINES[stage].changed() if not path.endswith(".asm.txt")]
        if moved:
            raise SystemExit(f"the working tree is not the {stage} tree: {moved}")
        made = record(stage, BASELINES[stage])
        name = f"crate-functions-{stage}.json"
        print(f"{write(name, made)}: {len(made['functions'])} functions")
        return
    if sys.argv[1:] != ["compare"]:
        raise SystemExit(__doc__)
    steps = [join(name, held(first), held(last)) for name, first, last in STEPS]
    comparison = {
        "schema": "crate-function-comparison-v2",
        "issue": ISSUE,
        "comparison_rule": RULE,
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
