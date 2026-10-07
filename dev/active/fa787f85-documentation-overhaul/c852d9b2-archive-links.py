#!/usr/bin/env python3
"""Internal-link scan of the archives under `archive_root` of `[documentation]`.

Roots: each directory holding a tracked `.jit-container` marker, the directory
`legacy` beside them, and each deck, a directory under `archive_root` that
holds a tracked HTML file and a tracked stylesheet. A container or legacy root
covers its tracked Markdown files; a deck root covers its tracked HTML and CSS
files. Markdown links and anchors use the grammar of the mechanical
documentation check; HTML contributes `href`, `src` and `url(`, CSS `url(`.

A target with a scheme or a leading `//` is counted per scheme and not
fetched; a fragment-only target of an HTML file is counted and not checked.
Every other target is internal and resolves against the tracked files.

Prints per root the files scanned, the internal links checked, the other
counts and each unresolved link as `<file>:<line>: <target>`. Exit 1 on any
unresolved link.
"""
import importlib.util
import posixpath
import re
import subprocess
import sys
import tomllib
import urllib.parse
from collections import Counter
from pathlib import Path

ATTRIBUTE = re.compile(r"""\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)')""")
CSS_URL = re.compile(r"""\burl\(\s*["']?([^"')\s]+)["']?\s*\)""")


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


root = Path(git("-C", str(Path(__file__).resolve().parent), "rev-parse", "--show-toplevel").strip())
spec = importlib.util.spec_from_file_location("docs_mechanical", root / "contrib/gates/docs-mechanical.py")
grammar = importlib.util.module_from_spec(spec)
spec.loader.exec_module(grammar)
checker = grammar.Checker(root, "jit")

tracked = git("-C", str(root), "ls-files", "-z").split("\0")[:-1]
present = set(tracked)
for name in tracked:
    while "/" in name:
        name = name.rsplit("/", 1)[0]
        present.add(name)

archive = tomllib.loads((root / ".jit/config.toml").read_text())["documentation"]["archive_root"].rstrip("/")
archived = [f for f in tracked if f.startswith(archive + "/")]
containers = sorted(posixpath.dirname(f) for f in archived if posixpath.basename(f) == ".jit-container")
styled = {posixpath.dirname(f) for f in archived if f.endswith(".css")}
decks = sorted({posixpath.dirname(f) for f in archived if f.endswith(".html")} & styled)


def under(directory, suffixes):
    return [f for f in archived if f.startswith(directory + "/") and f.endswith(suffixes)]


def deck_of(name):
    """The deepest deck directory holding the file, so nested decks share no file."""
    return max((d for d in decks if name.startswith(d + "/")), key=len)


def targets(name):
    text = (root / name).read_text(encoding="utf-8", errors="replace")
    if name.endswith(".md"):
        for number, line in grammar.markdown_lines(text):
            for target in grammar.link_targets(line):
                yield number, target
        return
    for number, line in enumerate(text.splitlines(), 1):
        if name.endswith(".html"):
            for double, single in ATTRIBUTE.findall(line):
                yield number, double or single
        for target in CSS_URL.findall(line):
            yield number, target


def resolves(name, target):
    raw_path, _, fragment = target.partition("#")
    path = urllib.parse.unquote(raw_path)
    if not path:
        resolved = name
    elif path.startswith("/"):
        resolved = posixpath.normpath(path.lstrip("/"))
    else:
        resolved = posixpath.normpath(posixpath.join(posixpath.dirname(name), path))
    if resolved not in present:
        return False
    if not fragment or resolved not in tracked_files:
        return True
    line_anchor = grammar.LINE_ANCHOR.match(fragment)
    if line_anchor:
        with (root / resolved).open("rb") as handle:
            return int(line_anchor.group(2) or line_anchor.group(1)) <= sum(1 for _ in handle)
    if resolved.endswith(".md"):
        return urllib.parse.unquote(fragment).lower() in checker.anchors(root / resolved)
    return True


def scan(kind, directory, files):
    internal, fragments, schemes, unresolved = 0, 0, Counter(), []
    for name in files:
        for number, target in targets(name):
            scheme = grammar.SCHEME.match(target)
            if scheme or target.startswith("//"):
                schemes[scheme.group(0).lower() if scheme else "//"] += 1
            elif not name.endswith(".md") and (not target or target.startswith("#")):
                fragments += 1
            else:
                internal += 1
                if not resolves(name, target):
                    unresolved.append(f"{name}:{number}: {target}")
    other = "".join(f", {count} {scheme}" for scheme, count in sorted(schemes.items()))
    if kind == "deck":
        other += f", {fragments} fragment-only"
    print(f"{kind} {directory}: {len(files)} files, {internal} internal links, {len(unresolved)} unresolved{other}")
    for line in unresolved:
        print(f"  {line}")
    return len(unresolved)


tracked_files = set(tracked)
total = sum(scan("container", d, under(d, ".md")) for d in containers)
total += scan("legacy", f"{archive}/legacy", under(f"{archive}/legacy", ".md"))
total += sum(scan("deck", d, [f for f in under(d, (".html", ".css")) if deck_of(f) == d]) for d in decks)
print(f"unresolved internal links: {total}")
sys.exit(1 if total else 0)
