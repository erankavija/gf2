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
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from repository_files import live_file, package_directory, repository_root, tracked_files

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


# Generators whose `--check` is exercised on a fixture holding the repository's
# own copy of each.
LIVE_GENERATORS = ("make-external-producing-inputs.py", "make-shift-producing-inputs.py")
CAMPAIGN_SUPPORT = "tuning-campaign-support"
HELPERS = ("producing_closure.py", "repository_files.py")


def tree_bytes(root: Path) -> dict[str, bytes]:
    return {
        str(path.relative_to(root)): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file() and ".git" not in path.relative_to(root).parts
    }


def status(root: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain", "--ignored"],
        capture_output=True, check=True, text=True,
    ).stdout


def run_generator(root: Path, generator: str, *arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "-B", str(root / generator), *arguments],
        capture_output=True, text=True, cwd=root,
    )


def stage_live_generator(live: Path, root: Path, name: str, scratch: Path) -> str:
    """Stages the repository's generator `name` over empty copies of its closure's files."""
    generator = live_file(live, name)
    for path in (generator, *(live_file(live, helper) for helper in HELPERS)):
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(live / path, root / path)
    closure = scratch / f"{name}.json"
    run = subprocess.run(
        [sys.executable, "-B", str(live / generator), str(closure)], capture_output=True, text=True
    )
    if run.returncode != 0:
        raise SystemExit(f"{name} does not run on the repository:\n{run.stderr}")
    listed = json.loads(closure.read_text())["build_inputs"]
    for path in listed:
        if not (root / path).exists():
            (root / path).parent.mkdir(parents=True, exist_ok=True)
            (root / path).write_text("")
    manifest = root / package_directory(live, CAMPAIGN_SUPPORT) / "Cargo.toml"
    manifest.write_text(f'[package]\nname = "{CAMPAIGN_SUPPORT}"\n')
    return generator


def live_generator_findings(live: Path, name: str) -> list[str]:
    """Findings for generator `name`: a check that accepts a current closure, writes
    nothing, and rejects a closure that lacks a source added after it was written."""
    with tempfile.TemporaryDirectory() as directory:
        scratch = Path(directory)
        root = scratch / "fixture"
        root.mkdir()
        subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
        generator = stage_live_generator(live, root, name, scratch)
        if run_generator(root, generator).returncode != 0:
            return [f"{name} does not write its closure on the fixture"]
        positional = scratch / "positional.json"
        run_generator(root, generator, str(positional))
        before, listing = tree_bytes(root), status(root)
        written = [b for p, b in before.items() if p.endswith("producing-inputs.json")]
        findings = []
        if len(written) != 1 or positional.read_bytes() != written[0]:
            findings.append(f"{name} writes a different closure to its first argument")
        accepted = run_generator(root, generator, "--check")
        if accepted.returncode != 0:
            findings.append(f"{name} --check rejects a current closure:\n{accepted.stderr}")
        if tree_bytes(root) != before or status(root) != listing:
            findings.append(f"{name} --check writes inside the repository")
        sources = json.loads(positional.read_text())["behavior_sources"]
        member = next(p for p in sources if p.endswith(".rs") and "/src/" in p)
        added = Path(member).parent / "added_after_closure.rs"
        (root / added).write_text("\n")
        stale = run_generator(root, generator, "--check")
        if stale.returncode == 0 or str(added) not in stale.stderr + stale.stdout:
            findings.append(f"{name} --check accepts a closure lacking {added}")
        (root / added).unlink()
        if tree_bytes(root) != before or status(root) != listing:
            findings.append(f"{name} --check leaves the tree changed after a stale verdict")
        return findings


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
        live = repository_root(Path(__file__).resolve())
        findings = [f for name in LIVE_GENERATORS for f in live_generator_findings(live, name)]
        if findings:
            print("self-test: " + "\n".join(findings), file=sys.stderr)
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
