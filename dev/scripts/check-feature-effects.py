#!/usr/bin/env python3
"""Fails when a cargo feature of gf2-coding or gf2-sim has no effect.

A feature has an effect when its table entry is non-empty (it enables a
dependency or another feature) or a `cfg(feature = "...")` in the package's
Rust sources names it. An empty `default` is a violation.
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

import repository_files

PACKAGES = ("gf2-coding", "gf2-sim")


def read_features(root: Path, directory: str) -> set[str]:
    """Feature names that some Rust source of the package names in a cfg."""
    names: set[str] = set()
    pattern = re.compile(r'feature\s*=\s*"([^"]+)"')
    for source in (root / directory).rglob("*.rs"):
        names.update(pattern.findall(source.read_text(errors="replace")))
    return names


def violations(root: Path, package: str) -> list[str]:
    directory = repository_files.package_directory(root, package)
    manifest = tomllib.loads((root / directory / "Cargo.toml").read_text())
    read = read_features(root, directory)
    found = []
    for feature, enables in manifest.get("features", {}).items():
        if feature == "default":
            if not enables:
                found.append(f"{package}: `default` is empty")
        elif not enables and feature not in read:
            found.append(f"{package}: feature `{feature}` enables nothing and no cfg reads it")
    return found


def main() -> int:
    root = repository_files.repository_root(Path(__file__).resolve())
    found = [line for package in PACKAGES for line in violations(root, package)]
    for line in found:
        print(line, file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
