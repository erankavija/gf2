#!/usr/bin/env python3
"""Checks that the shared producing-input closure enumerates the runner's sources.

A campaign plan that names no family closure pins
`dev/active/f547c394/producing-inputs.json` (`protocol::RunnerPlan::producing_manifest_path`),
so that document decides which bytes the campaign's behaviour identity covers. A
source the runner compiles and the closure omits leaves a file that can change
what a campaign measures outside the receipt's snapshot, which
`@/inv/behavioral-evidence-validity` and `@/inv/runtime-observed-provenance`
forbid. The closure is maintained by hand and this check owns its completeness;
a family closure generated from its own tree is checked by that generator's
`--check`.

The runner's sources are `src/lib.rs`, every module it declares transitively,
and the runner binary. A module behind a cargo feature outside the crate's
default set is absent from that build; any other `cfg` gate is unknown here and
counts as compiled.

Usage:
  dev/scripts/check-campaign-producing-closure.py [--self-test]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
import tomllib
from pathlib import Path

CLOSURE = "dev/active/f547c394/producing-inputs.json"
CRATE = "dev/tools/tuning-campaign-support"
RUNNER = "src/bin/benchmark-ab-runner.rs"
SCHEMA = "tuning-campaign-producing-inputs-v1"
# The closure sections that select measurement-producing bytes; `lifecycle_sources`
# selects the subset deciding campaign lifecycle and resume, which the arm and
# cell-driving layers are not part of.
REQUIRED_SECTIONS = ("behavior_sources", "build_inputs")

MODULE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
FEATURE_GATE = re.compile(r'^\s*#\[cfg\(feature\s*=\s*"([^"]+)"\)\]\s*$')


def default_features(manifest: Path) -> set[str]:
    """The crate features a build with default features enables."""
    table = tomllib.loads(manifest.read_text()).get("features", {})
    enabled: set[str] = set()
    pending = list(table.get("default", []))
    while pending:
        name = pending.pop()
        # `dep:` and `crate/feature` entries enable no module gate of this crate.
        if name in enabled or name.startswith("dep:") or "/" in name:
            continue
        enabled.add(name)
        pending.extend(table.get(name, []))
    return enabled


def submodule_dir(source: Path) -> Path:
    """The directory holding the submodules `source` declares."""
    return source.parent if source.name in ("lib.rs", "mod.rs") else source.parent / source.stem


def declared_modules(source: Path, features: set[str]) -> list[str]:
    """Names of the modules `source` declares in a build enabling `features`."""
    names: list[str] = []
    gate: str | None = None
    for line in source.read_text().splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        feature = FEATURE_GATE.match(line)
        if feature:
            gate = feature.group(1)
            continue
        module = MODULE.match(line)
        if module:
            if gate is None or gate in features:
                names.append(module.group(1))
            gate = None
            continue
        if not stripped.startswith("#["):
            gate = None
    return names


def runner_sources(root: Path) -> list[str]:
    """Every campaign-tools source the runner binary compiles, root-relative."""
    crate = root / CRATE
    features = default_features(crate / "Cargo.toml")
    library = crate / "src/lib.rs"
    sources = [crate / RUNNER, library]
    pending = [library]
    while pending:
        source = pending.pop()
        directory = submodule_dir(source)
        for name in declared_modules(source, features):
            found = next(
                (
                    candidate
                    for candidate in (directory / f"{name}.rs", directory / name / "mod.rs")
                    if candidate.is_file()
                ),
                None,
            )
            if found is None:
                raise SystemExit(f"{source}: module {name} resolves to no source file")
            sources.append(found)
            pending.append(found)
    return sorted(str(source.relative_to(root)) for source in sources)


def omissions(closure: dict, sources: list[str]) -> list[str]:
    """One line per runner source a required closure section does not name."""
    lines = []
    for section in REQUIRED_SECTIONS:
        named = set(closure.get(section, []))
        lines += [f"{section} omits {path}" for path in sources if path not in named]
    return lines


def check(root: Path) -> list[str]:
    """Reports every defect of the shared closure under `root`."""
    try:
        closure = json.loads((root / CLOSURE).read_text())
    except (OSError, json.JSONDecodeError) as error:
        return [f"{CLOSURE}: {error}"]
    if closure.get("schema") != SCHEMA:
        return [f"{CLOSURE}: schema is not {SCHEMA}"]
    return omissions(closure, runner_sources(root))


def write_fixture(root: Path, name_arm: bool) -> None:
    """Stages a crate whose runner compiles `arm` and a feature-gated `scratch`."""
    crate = root / CRATE
    (crate / "src/bin").mkdir(parents=True)
    (crate / "Cargo.toml").write_text('[features]\ntest-support = []\n')
    (crate / "src/lib.rs").write_text(
        'pub mod arm;\n\n#[cfg(feature = "test-support")]\npub mod scratch;\n'
    )
    for relative in ("src/arm.rs", "src/scratch.rs", RUNNER):
        (crate / relative).write_text("\n")
    named = [f"{CRATE}/src/lib.rs", f"{CRATE}/{RUNNER}"]
    if name_arm:
        named.append(f"{CRATE}/src/arm.rs")
    (root / CLOSURE).parent.mkdir(parents=True)
    (root / CLOSURE).write_text(
        json.dumps(
            {
                "schema": SCHEMA,
                "behavior_sources": sorted(named),
                "lifecycle_sources": [],
                "build_inputs": sorted(named),
            },
            indent=2,
        )
        + "\n"
    )


def self_test() -> int:
    """Asserts the check accepts a complete closure and rejects an omission.

    The complete case names no feature-gated module, so it also asserts that a
    module outside the runner's build is not demanded.
    """
    cases = (
        ("complete", True, []),
        (
            "omitted",
            False,
            [
                f"behavior_sources omits {CRATE}/src/arm.rs",
                f"build_inputs omits {CRATE}/src/arm.rs",
            ],
        ),
    )
    with tempfile.TemporaryDirectory() as directory:
        for case, name_arm, expected in cases:
            root = Path(directory) / case
            root.mkdir()
            write_fixture(root, name_arm)
            observed = check(root)
            if observed != expected:
                print(
                    f"self-test case {case}: expected {expected}, observed {observed}",
                    file=sys.stderr,
                )
                return 1
    print("check-campaign-producing-closure: self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check the checker against a synthetic complete closure and an omission",
    )
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    root = Path(__file__).resolve().parents[2]
    findings = check(root)
    if findings:
        print(f"{CLOSURE} does not enumerate the runner's sources:", file=sys.stderr)
        for finding in findings:
            print(f"    {finding}", file=sys.stderr)
        return 1
    sources = runner_sources(root)
    print(
        f"check-campaign-producing-closure: {CLOSURE} names all "
        f"{len(sources)} runner sources"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
