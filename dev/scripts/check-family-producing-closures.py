#!/usr/bin/env python3
"""Checks every family producing-input closure against its tree.

A family closure is written by a generator named `make-*-producing-inputs.py`
that enumerates its sources. A generator whose source offers `--check` compares
the committed closure with that enumeration and writes nothing; a generator
without the option is not run, since its invocation writes its output. A merge that adds or removes an
enumerated source leaves the closure stale and its campaign evidence outside the
receipt's snapshot (`@/inv/behavioral-evidence-validity`). This check runs the
`--check` of every live generator the shared locator finds, so a later family is
covered without a CI edit; the shared closure is checked by
`check-campaign-producing-closure.py`.

Usage:
  check-family-producing-closures.py [--self-test]
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

from repository_files import repository_root, tracked_files

GENERATOR = "make-*-producing-inputs.py"
OPTION = "--check"


def checkable_generators(root: Path) -> list[str]:
    """Root-relative live generators whose source offers `--check`."""
    return [
        path
        for path in tracked_files(root, GENERATOR)
        if f'"{OPTION}"' in (root / path).read_text()
    ]


def check(root: Path) -> list[str]:
    """One block per live generator whose `--check` fails; none when all pass."""
    generators = checkable_generators(root)
    if not generators:
        return [f"no live generator matches {GENERATOR} and offers {OPTION}"]
    findings = []
    for generator in generators:
        run = subprocess.run(
            [sys.executable, "-B", str(root / generator), OPTION],
            capture_output=True,
            text=True,
        )
        if run.returncode != 0:
            detail = (run.stderr + run.stdout).strip()
            findings.append(f"{generator} exited {run.returncode}:\n{detail}")
    return findings


FIXTURE_GENERATOR = '''\
import json, pathlib, subprocess, sys
ROOT = pathlib.Path(subprocess.run(
    ["git", "-C", str(pathlib.Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"],
    capture_output=True, check=True, text=True).stdout.strip())
OUTPUT = ROOT / "{family}/closure.json"
text = json.dumps(sorted(str(p.relative_to(ROOT)) for p in (ROOT / "{family}/src").rglob("*"))) + "\\n"
if "--check" not in sys.argv:
    OUTPUT.write_text(text)
elif OUTPUT.read_text() != text:
    raise SystemExit("closure is not the closure of this tree")
'''


def stage_family(root: Path, family: str) -> None:
    """Stages one family: a generator, a source directory and its closure."""
    (root / family / "src").mkdir(parents=True)
    (root / family / "src/a.rs").write_text("\n")
    generator = root / family / "make-fixture-producing-inputs.py"
    generator.write_text(FIXTURE_GENERATOR.format(family=family))
    subprocess.run([sys.executable, "-B", str(generator)], check=True)


def self_test() -> int:
    """Asserts the check accepts current families and rejects a stale one.

    Each stale case changes the enumerated directory after the closure is
    written; the families are independent, so the check names the stale one only.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
        if check(root) != [f"no live generator matches {GENERATOR} and offers {OPTION}"]:
            print("self-test: a tree without a generator is accepted", file=sys.stderr)
            return 1
        stage_family(root, "current")
        stage_family(root, "added")
        stage_family(root, "removed")
        if check(root):
            print("self-test: current families are rejected", file=sys.stderr)
            return 1
        (root / "added/src/b.rs").write_text("\n")
        (root / "removed/src/a.rs").unlink()
        stale = [finding.split(" ")[0] for finding in check(root)]
        expected = ["added/make-fixture-producing-inputs.py", "removed/make-fixture-producing-inputs.py"]
        if stale != expected:
            print(f"self-test: expected {expected}, observed {stale}", file=sys.stderr)
            return 1
    print("check-family-producing-closures: self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check the checker against fixture families that are current and stale",
    )
    if parser.parse_args().self_test:
        return self_test()
    root = repository_root(Path(__file__).resolve())
    findings = check(root)
    if findings:
        print("a family producing-input closure differs from its tree:", file=sys.stderr)
        for finding in findings:
            print(f"    {finding}", file=sys.stderr)
        return 1
    count = len(checkable_generators(root))
    print(f"check-family-producing-closures: {count} family closures enumerate their trees")
    return 0


if __name__ == "__main__":
    sys.exit(main())
