"""Digests every function a package's release build emits.

`record` builds one package with `--emit=asm` under the toolchain the
environment selects and digests each function's instruction text under RULE;
`join` compares two such records per function. A record carries the digest of
its tree's source digests, and `held` checks it against a content baseline.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path

import repository_files

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
EMIT = ["--", "--emit=asm"]
LISTING = ".asm.txt"


def build(package: str) -> list[str]:
    return ["cargo", "rustc", "--release", "-p", package, "--lib"]


def emitted(root: Path, package: str, issue: str) -> str:
    """The assembly text of a fresh release build of `package`."""
    target = root / "target" / f"{issue}-emit"
    budget = root / repository_files.live_file(root, "cargo-budget.sh")
    subprocess.run(
        [str(budget), *build(package), "--target-dir", str(target), *EMIT], cwd=root, check=True
    )
    stem = package.replace("-", "_")
    found = sorted(
        (target / "release" / "deps").glob(f"{stem}-*.s"), key=lambda p: p.stat().st_mtime
    )
    return found[-1].read_text()


def functions(assembly: str) -> dict[str, str]:
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


def sources_digest(digests: dict[str, str]) -> str:
    """Digest of the per-path digests of the package files other than listings."""
    sources = {path: held for path, held in digests.items() if not path.endswith(LISTING)}
    return hashlib.sha256(json.dumps(sources, sort_keys=True).encode()).hexdigest()


def record(root: Path, package: str, issue: str, tree: str, anchor: dict, paths: list[str]) -> dict:
    """The function digests of the working tree, whose package files are `paths`.

    `anchor` is the identity of the baseline the tree holds.
    """
    sources = {path: hashlib.sha256((root / path).read_bytes()).hexdigest() for path in paths}
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, check=True, text=True)
    return {
        "schema": "crate-function-digests-v1",
        "issue": issue,
        "tree": tree,
        "command": " ".join(build(package) + EMIT),
        "rustc": rustc.stdout.strip(),
        "rule": RULE,
        "anchor": anchor,
        "sources_sha256": sources_digest(sources),
        "functions": functions(emitted(root, package, issue)),
    }


def join(name: str, before: dict, after: dict) -> dict:
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


def held(path: Path, baseline_digests: dict[str, str]) -> dict:
    """The committed record at `path`, checked against a baseline's source digests."""
    found = json.loads(path.read_bytes())
    if found["sources_sha256"] != sources_digest(baseline_digests):
        raise SystemExit(f"{path.name} does not describe its baseline's sources")
    return found
