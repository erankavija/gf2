#!/usr/bin/env python3
"""Join two outputs of `62f0d0e6-comment-census.py` into one table.

Usage: python3 030496bd-census-compare.py BASELINE_CENSUS AFTER_CENSUS

Prints both measured commits, then per crate and in total the comment lines,
non-blank lines, share and pattern lines of each census, baseline first.
Every figure is copied from the input files.
"""

import sys
from pathlib import Path


def read(path: str) -> tuple[str, dict[str, list[str]]]:
    lines = Path(path).read_text(encoding="utf-8").splitlines()
    commit = lines[0].removeprefix("commit: ")
    return commit, {r[0]: r[1:] for r in (line.split() for line in lines[3:])}


def main() -> None:
    (base_commit, base), (after_commit, after) = read(sys.argv[1]), read(sys.argv[2])
    print(f"baseline: {base_commit}")
    print(f"after:    {after_commit}")
    print(f"{'':<24}" + "".join(f"{h:>18}" for h in ("comment", "non-blank", "share", "pattern")))
    print(f"{'crate':<24}" + 4 * f"{'before':>10}{'after':>8}")
    for name in sorted(base.keys() | after.keys(), key=lambda n: (n == "total", n)):
        b, a = base.get(name, ["-"] * 4), after.get(name, ["-"] * 4)
        print(f"{name:<24}" + "".join(f"{x:>10}{y:>8}" for x, y in zip(b, a)))


if __name__ == "__main__":
    main()
