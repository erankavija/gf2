#!/usr/bin/env python3
"""List the rustdoc example sections of one commit against a baseline commit.

Usage: python3 2596b143-changed-examples.py BASE MEASURED VERDICTS [--obvious]

An example section runs from a `# Example` / `# Examples` heading (the heading
pattern of `fa787f85-example-census.py`) to the next rustdoc heading outside a
code fence or the end of the doc block. Its item is the declaration the doc
block documents, qualified by the enclosing top-level `impl` or `trait`; a
`//!` block's item is `(module)`. Sections pair across the two commits by file
path and item. VERDICTS is the baseline verdict table, read from BASE.

Default output, one row per section at MEASURED:
  status    `unchanged` (a BASE section of the same path and item has the same
            text), `changed` (same path and item, different text) or `added`
  location  path:line of the heading at MEASURED
  item
  baseline  path:line of the paired BASE heading, or `-`
  verdict   the VERDICTS class cited within one line of `baseline`, or `-`

With --obvious, one row per VERDICTS row of class OBVIOUS that pairs with a
section at MEASURED by path and item or by identical fence content, then a
count line; rows without such a pair are counted only.

Both trees are read with `git show`, so the working tree does not matter.
"""

import re
import subprocess
import sys
from collections import defaultdict

DOC = re.compile(r"^\s*(///|//!)\s?(.*)$")
EX_HEAD = re.compile(r"^\s*(?:///|//!)\s*#+\s*Examples?\s*$", re.IGNORECASE)
DECL = re.compile(r"\b(fn|struct|enum|trait|type|const|static|mod|union|macro_rules!)\s+(\w+)")
ROOT = subprocess.run(["git", "rev-parse", "--show-toplevel"], check=True,
                      capture_output=True, text=True).stdout.strip()


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True,
                          text=True, errors="replace").stdout


def owner(line: str) -> str:
    """Type name of a column-zero `impl` or `trait` header, else ''."""
    m = re.match(r"(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(impl|trait)\b(.*)", line)
    if not m:
        return ""
    rest = m.group(2).strip()
    if m.group(1) == "impl" and rest.startswith("<"):
        depth = 0
        for i, ch in enumerate(rest):
            depth += (ch == "<") - (ch == ">")
            if depth == 0:
                rest = rest[i + 1:]
                break
    rest = rest.split(" for ")[-1]
    m = re.match(r"\s*&?\s*(?:mut\s+)?(?:dyn\s+)?([\w:]+)", rest)
    return m.group(1).split("::")[-1] if m else ""


def item_of(lines: list[str], end: int, inner: bool, scope: str) -> str:
    if inner:
        return "(module)"
    for line in lines[end:]:
        s = line.strip()
        if not s or s.startswith(("#[", "#!", "//")) or s[0] in ")]":
            continue
        m = DECL.search(s)
        if m:
            name = m.group(2)
        elif owner(s):
            return owner(s)
        else:
            name = re.split(r"[^\w]", s.removeprefix("pub "), maxsplit=1)[0]
        return f"{scope}::{name}" if scope and line[:1].isspace() else name
    return "(end of file)"


def sections(commit: str):
    """Yield (path, line, item, text, fence_text) per example section."""
    paths = git("ls-tree", "-r", "--name-only", commit, "--", "crates").splitlines()
    for path in (p for p in paths if p.endswith(".rs") and "/target/" not in p):
        lines = git("show", f"{commit}:{path}").splitlines()
        scope, i = "", 0
        while i < len(lines):
            m = DOC.match(lines[i])
            if not m:
                scope = owner(lines[i]) or ("" if lines[i][:1] == "}" else scope)
                i += 1
                continue
            start, kind = i, m.group(1)
            while i < len(lines) and (d := DOC.match(lines[i])) and d.group(1) == kind:
                i += 1
            item = item_of(lines, i, kind == "//!", scope)
            body = [DOC.match(x).group(2).rstrip() for x in lines[start:i]]
            fenced, cur = False, None
            for off, text in enumerate(body):
                is_fence = text.lstrip().startswith("```")
                if not fenced and not is_fence and text.startswith("#"):
                    if cur:
                        yield cur
                    cur = None
                    if EX_HEAD.match(lines[start + off]):
                        cur = (path, start + off + 1, item, [], [])
                elif cur:
                    cur[3].append(text)
                    if fenced and not is_fence:
                        cur[4].append(text.strip())
                fenced ^= is_fence
            if cur:
                yield cur


def main() -> int:
    obvious = "--obvious" in sys.argv
    base, measured, verdicts_path = [a for a in sys.argv[1:] if a != "--obvious"]
    by_key, by_fence = defaultdict(list), defaultdict(list)
    for path, line, item, text, fence in sections(base):
        by_key[path, item].append((line, text, fence))
    verdict_rows = [r.split("\t") for r in
                    git("show", f"{base}:{verdicts_path}").splitlines()[1:]]
    cited = {}
    for verdict, location, *_ in verdict_rows:
        path, _, line = location.rpartition(":")
        if line.isdigit():
            for near in (int(line) - 1, int(line) + 1, int(line)):
                cited[path, near] = verdict
    after = list(sections(measured))

    if not obvious:
        print("status\tlocation\titem\tbaseline\tverdict")
        for path, line, item, text, _ in after:
            pairs = by_key.get((path, item), [])
            same = [b for b in pairs if b[1] == text]
            hit = (same or pairs or [None])[0]
            status = "unchanged" if same else "changed" if pairs else "added"
            ref = f"{path}:{hit[0]}" if hit else "-"
            verdict = cited.get((path, hit[0]), "-") if hit else "-"
            print(f"{status}\t{path}:{line}\t{item}\t{ref}\t{verdict}")
        return 0

    for path, line, item, _, fence in after:
        by_fence["\n".join(fence)].append(f"{path}:{line}")
    at_measured = defaultdict(list)
    for path, line, item, _, _ in after:
        at_measured[path, item].append(f"{path}:{line}")
    base_at = {(path, line): (item, fence) for (path, item), rows in by_key.items()
               for line, _, fence in rows}
    print("baseline\titem\tmeasured\tpairing")
    counts = defaultdict(int)
    for verdict, location, name, *_ in verdict_rows:
        if verdict != "OBVIOUS":
            continue
        path, _, line = location.rpartition(":")
        found = next((base_at[path, n] for n in (int(line), int(line) - 1, int(line) + 1)
                      if (path, n) in base_at), None)
        if found is None:
            counts["verdict rows without a section at BASE"] += 1
            print(f"{location}\t{name}\t-\tunresolved")
            continue
        item, fence = found
        by_content = by_fence.get("\n".join(fence), []) if fence else []
        hits = at_measured.get((path, item), [])
        if hits or by_content:
            counts["paired with a section at MEASURED"] += 1
            pairing = "+".join(k for k, v in (("item", hits), ("fence", by_content)) if v)
            print(f"{location}\t{name}\t{(hits or by_content)[0]}\t{pairing}")
        else:
            counts["no section at MEASURED"] += 1
    for label in ("no section at MEASURED", "paired with a section at MEASURED",
                  "verdict rows without a section at BASE"):
        print(f"# {label}: {counts[label]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
