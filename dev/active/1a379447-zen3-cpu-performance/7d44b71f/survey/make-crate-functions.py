#!/usr/bin/env python3
"""Digest every function the kernel crate's release build emits (jit:7d44b71f).

Builds `gf2-kernels-simd` with `--emit=asm` under the toolchain the environment
selects and digests each function's instruction text under RULE.

  anchor   writes `crate-functions-anchor.json`; refuses unless every package
           file other than an assembly listing holds its `anchor-baseline.json`
           digest, so the record describes the anchor sources.
  current  writes `crate-functions-current.json` and
           `crate-function-comparison.json`, which joins the two records per
           function; exits nonzero after writing when a function differs or
           exists on one side only.

Usage: make-crate-functions.py anchor|current
"""

import hashlib
import json
import re
import subprocess
import sys

from locate import ANCHOR, HERE, ISSUE, PACKAGE, PACKAGE_NAME, ROOT, repository_files, tracked

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


def record(mode):
    sources = {
        path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
        for path in tracked("") if not path.endswith(".asm.txt")
    }
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, check=True, text=True)
    return {
        "schema": "crate-function-digests-v1",
        "issue": ISSUE,
        "tree": mode,
        "command": " ".join(BUILD + EMIT),
        "rustc": rustc.stdout.strip(),
        "rule": RULE,
        "anchor": ANCHOR.identity(),
        "sources_sha256": hashlib.sha256(json.dumps(sources, sort_keys=True).encode()).hexdigest(),
        "functions": functions(emitted()),
    }


def write(name, value):
    output = HERE / name
    output.write_text(json.dumps(value, indent=2) + "\n")
    return output.relative_to(ROOT)


def main():
    if sys.argv[1:] == ["anchor"]:
        moved = [path for path in ANCHOR.changed() if not path.endswith(".asm.txt")]
        if moved:
            raise SystemExit(f"the working tree is not the anchor: {moved}")
        made = record("anchor")
        print(f"{write('crate-functions-anchor.json', made)}: {len(made['functions'])} functions")
        return
    if sys.argv[1:] != ["current"]:
        raise SystemExit(__doc__)
    anchor = json.loads((HERE / "crate-functions-anchor.json").read_bytes())
    current = record("current")
    write("crate-functions-current.json", current)
    if anchor["rustc"] != current["rustc"]:
        raise SystemExit(f"anchor built with {anchor['rustc']}, this run with {current['rustc']}")
    old, new = anchor["functions"], current["functions"]
    rows = [
        {
            "function": name,
            "anchor_sha256": old.get(name),
            "current_sha256": new.get(name),
            "instruction_text": "same" if name in old and old[name] == new.get(name) else "differs",
        }
        for name in sorted(set(old) | set(new))
    ]
    differing = sum(row["instruction_text"] != "same" for row in rows)
    comparison = {
        "schema": "crate-function-comparison-v1",
        "issue": ISSUE,
        "rustc": current["rustc"],
        "comparison_rule": RULE,
        "anchor_sources_sha256": anchor["sources_sha256"],
        "current_sources_sha256": current["sources_sha256"],
        "function_count": len(rows),
        "differing_function_count": differing,
        "functions": rows,
    }
    print(f"{write('crate-function-comparison.json', comparison)}: "
          f"{len(rows)} functions, {differing} differing")
    if differing:
        raise SystemExit("a function's instruction text differs from the anchor build")


if __name__ == "__main__":
    main()
