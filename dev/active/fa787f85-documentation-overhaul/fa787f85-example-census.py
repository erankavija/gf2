#!/usr/bin/env python3
"""Mechanical census of the workspace rustdoc example surface.

Produces the counts REQ-07 of epic fa787f85 requires on both sides of the
tautological-example removal: heading count, fence disposition, per-crate
example counts, and the documentation-to-code ratio. Run it before and after
the removal pass so both ends of the comparison come from one method.

Counts headings in both spellings (`# Example` and `# Examples`); a census
matching only the plural form undercounts.

Usage: python3 fa787f85-example-census.py [repo-root]
"""

import re
import sys
from collections import Counter
from pathlib import Path

DOC = re.compile(r"^\s*(///|//!)")
EX_HEAD = re.compile(r"^\s*(?:///|//!)\s*#+\s*Examples?\s*$", re.IGNORECASE)
FENCE = re.compile(r"^\s*(?:///|//!)\s*```(.*)$")
PUB = re.compile(r"^\s*pub(\s|\()")

EXECUTED = {"", "rust", "should_panic"}


def census(root: Path):
    crates, files_with_headings = {}, Counter()
    fences = Counter()
    for crate in sorted(p for p in (root / "crates").iterdir() if p.is_dir()):
        c = Counter()
        for rs in crate.rglob("*.rs"):
            if "/target/" in str(rs):
                continue
            c["files"] += 1
            open_fence = False
            for line in rs.read_text(encoding="utf-8", errors="replace").splitlines():
                if DOC.match(line):
                    c["doc_lines"] += 1
                    if EX_HEAD.match(line):
                        c["headings"] += 1
                        files_with_headings[str(rs.relative_to(root))] += 1
                    m = FENCE.match(line)
                    if m:
                        if not open_fence:
                            fences[m.group(1).strip()] += 1
                        open_fence = not open_fence
                elif line.strip():
                    c["code_lines"] += 1
                    if PUB.match(line):
                        c["pub_decls"] += 1
        crates[crate.name] = c
    return crates, fences, files_with_headings


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    if not (root / "crates").is_dir():
        print(f"no crates/ under {root}", file=sys.stderr)
        return 1
    crates, fences, files_with_headings = census(root)

    print(f"{'crate':<20} {'files':>6} {'doc':>8} {'code':>8} {'doc/code':>9} "
          f"{'pub':>6} {'examples':>9}")
    tot = Counter()
    for name, c in crates.items():
        ratio = c["doc_lines"] / c["code_lines"] if c["code_lines"] else 0
        print(f"{name:<20} {c['files']:>6} {c['doc_lines']:>8} {c['code_lines']:>8} "
              f"{ratio:>8.0%} {c['pub_decls']:>6} {c['headings']:>9}")
        tot.update(c)
    ratio = tot["doc_lines"] / tot["code_lines"] if tot["code_lines"] else 0
    print(f"{'TOTAL':<20} {tot['files']:>6} {tot['doc_lines']:>8} {tot['code_lines']:>8} "
          f"{ratio:>8.0%} {tot['pub_decls']:>6} {tot['headings']:>9}")

    executed = sum(n for k, n in fences.items() if k in EXECUTED)
    compiled_only = fences.get("no_run", 0)
    print(f"\nrustdoc code fences: {sum(fences.values())}")
    print(f"  executed as doctests           {executed}")
    print(f"  compiled, not executed(no_run) {compiled_only}")
    print(f"  never compiled                 {sum(fences.values()) - executed - compiled_only}")
    for kind, n in sorted(fences.items(), key=lambda kv: -kv[1]):
        print(f"    {n:>5}  ```{kind}")

    print(f"\nfiles carrying at least one example heading: {len(files_with_headings)}")
    print("densest files:")
    for path, n in files_with_headings.most_common(15):
        print(f"  {n:>3}  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
