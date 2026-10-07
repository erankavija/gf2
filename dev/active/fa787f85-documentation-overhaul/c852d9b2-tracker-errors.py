#!/usr/bin/env python3
"""Classifies the errors of `jit doc check-links --scope all --json` read on stdin.

Distinct errors (type, document, target) are grouped by type, area and file
suffix. Area is `archive` under `archive_root` of `[documentation]`,
`footprint` for a Markdown file of `[docs-mechanical]`, else `other`. For a
Markdown document, `linked` counts the errors whose target is a link of the
tracked file under the grammar of the mechanical documentation check, and
`unresolved` those of them whose path is not tracked; each such link is listed.
"""
import importlib.util
import json
import posixpath
import subprocess
import sys
import tomllib
import urllib.parse
from collections import Counter
from pathlib import Path


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


root = Path(git("-C", str(Path(__file__).resolve().parent), "rev-parse", "--show-toplevel").strip())
spec = importlib.util.spec_from_file_location("docs_mechanical", root / "contrib/gates/docs-mechanical.py")
grammar = importlib.util.module_from_spec(spec)
spec.loader.exec_module(grammar)

tracked = set(git("-C", str(root), "ls-files", "-z").split("\0")[:-1])
present = set(tracked)
for name in tracked:
    while "/" in name:
        name = name.rsplit("/", 1)[0]
        present.add(name)

config = tomllib.loads((root / ".jit/config.toml").read_text())
archive = config["documentation"]["archive_root"].rstrip("/") + "/"
settings = config["docs-mechanical"]
excluded = [root / prefix for prefix in settings.get("exclude", [])]
footprint = {p.relative_to(root).as_posix() for p in grammar.collect(root, settings.get("markdown", []), excluded)}


def links(name):
    text = (root / name).read_text(encoding="utf-8", errors="replace")
    return {target for _, line in grammar.markdown_lines(text) for target in grammar.link_targets(line)}


def is_tracked(name, target):
    path = urllib.parse.unquote(target.partition("#")[0])
    return posixpath.normpath(posixpath.join(posixpath.dirname(name), path)) in present


errors = {(e["type"], e["document"], e.get("link") or e.get("asset") or "") for e in json.load(sys.stdin)["errors"]}
groups, linked, unresolved, listed = Counter(), Counter(), Counter(), []
for kind, name, target in sorted(errors):
    area = "archive" if name.startswith(archive) else "footprint" if name in footprint else "other"
    suffix = ".".join(posixpath.basename(name).split(".")[1:])
    key = (kind, area, suffix)
    groups[key] += 1
    if suffix == "md" and name in tracked and not grammar.SCHEME.match(target) and target in links(name):
        linked[key] += 1
        if not is_tracked(name, target):
            unresolved[key] += 1
            listed.append(f"  {area} {name}: {target}")
print("type area suffix: errors linked unresolved")
for key in sorted(groups):
    counts = f"{linked[key]} {unresolved[key]}" if key[2] == "md" else "- -"
    print(f"{' '.join(key)}: {groups[key]} {counts}")
print("unresolved links of tracked Markdown files:")
print("\n".join(sorted(set(listed))))
