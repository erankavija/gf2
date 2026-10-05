"""Shared mechanics of the family producing-input closure generators.

A generator enumerates a family's sources into a closure document. `emit`
writes it, or with `check` compares it with the committed document and writes
nothing, so `check-family-producing-closures.py` can run every generator.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import repository_files

SCHEMA = "tuning-campaign-producing-inputs-v1"


def rust_sources(root: Path, directory: str) -> list[str]:
    """Root-relative `.rs` files below `directory`."""
    return [
        str(path.relative_to(root))
        for path in (root / directory).rglob("*.rs")
        if path.is_file()
    ]


def document(behavior: set[str], lifecycle: list[str], build: set[str]) -> dict:
    return {
        "schema": SCHEMA,
        "behavior_sources": sorted(behavior),
        "lifecycle_sources": sorted(lifecycle),
        "build_inputs": sorted(build),
    }


def differences(recorded: dict, regenerated: dict) -> list[str]:
    """`+`/`- ` lines for every path one closure names and the other does not."""
    lines = []
    for section, paths in regenerated.items():
        if isinstance(paths, list):
            listed = set(recorded.get(section, []))
            lines += [f"+ {section} {path}" for path in sorted(set(paths) - listed)]
            lines += [f"- {section} {path}" for path in sorted(listed - set(paths))]
    return lines


def emit(output: Path, closure: dict, check: bool, regenerate: str) -> None:
    """Writes `closure` to `output`; with `check`, exits non-zero when it differs."""
    text = json.dumps(closure, indent=2) + "\n"
    if not check:
        output.write_text(text)
        print(
            f"{len(closure['behavior_sources'])} behavior, "
            f"{len(closure['lifecycle_sources'])} lifecycle, "
            f"{len(closure['build_inputs'])} build inputs -> {output}",
            file=sys.stderr,
        )
        return
    try:
        committed = output.read_text()
    except OSError as error:
        raise SystemExit(f"{output}: {error}") from error
    if committed == text:
        print(f"{output}: enumerates this tree")
        return
    try:
        recorded = json.loads(committed)
    except json.JSONDecodeError:
        recorded = {}
    detail = differences(recorded, closure) or [
        "the sections name the same paths, so the two differ in schema or rendering"
    ]
    raise SystemExit(
        "\n".join([f"{output} is not the closure of this tree; regenerate it with {regenerate}", *detail])
    )
