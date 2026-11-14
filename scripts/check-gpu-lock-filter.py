#!/usr/bin/env python3
"""Check that the nextest GPU-lock wrapper applies to exactly the gpu-serial tests.

nextest rejects `group()` in script filters, so each workspace's config repeats
the filter of the override that sets `test-group = 'gpu-serial'` in the script
entry that runs the `gpu-lock` wrapper. Each config must have exactly one such
override and script entry, both on `profile.default`, with identical filter text.
By default, check the main workspace and standalone gf2-kernels-hip workspace.

Usage: scripts/check-gpu-lock-filter.py [nextest.toml ...]
"""

import sys
import tomllib
from pathlib import Path

GROUP = "gpu-serial"
WRAPPER = "gpu-lock"


def check(path: Path) -> bool:
    try:
        config = tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError) as e:
        print(f"check-gpu-lock-filter: {path}: {e}", file=sys.stderr)
        return False
    profiles = config.get("profile", {})

    groups = [
        (name, o.get("filter"))
        for name, p in profiles.items()
        for o in p.get("overrides", [])
        if o.get("test-group") == GROUP
    ]
    scripts = [
        (name, s.get("filter"))
        for name, p in profiles.items()
        for s in p.get("scripts", [])
        if s.get("run-wrapper") == WRAPPER
    ]

    errors = []
    if len(groups) != 1 or groups[0][0] != "default":
        errors.append(f"expected one profile.default override with test-group '{GROUP}', found {groups}")
    if len(scripts) != 1 or scripts[0][0] != "default":
        errors.append(f"expected one profile.default script entry running '{WRAPPER}', found {scripts}")
    if not errors and groups[0][1] != scripts[0][1]:
        errors.append(
            f"the '{WRAPPER}' script filter differs from the '{GROUP}' override filter:\n"
            f"  override: {groups[0][1]!r}\n  script:   {scripts[0][1]!r}"
        )
    for e in errors:
        print(f"check-gpu-lock-filter: {path}: {e}", file=sys.stderr)
    return not errors


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    paths = [Path(p) for p in sys.argv[1:]] if len(sys.argv) > 1 else [
        root / ".config" / "nextest.toml",
        root / "crates" / "gf2-kernels-hip" / ".config" / "nextest.toml",
    ]
    results = [check(path) for path in paths]
    if not all(results):
        return 1
    print(f"check-gpu-lock-filter: ok ({len(paths)} configs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
