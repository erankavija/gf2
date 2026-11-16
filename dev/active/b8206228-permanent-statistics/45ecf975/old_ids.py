#!/usr/bin/env python3
"""Inventory, replace, or check old commit references after a history rewrite.

Usage: old_ids.py inventory|apply|check <commit-map> <original-git-dir>
       <checkout> [<data-root>] [--out <tsv>] [--exclude <glob>]...
The map may be plain text or gzip. Each --exclude glob is matched with fnmatch
against the checkout-relative path; matching files are neither read nor written.
Mapped references of at least 12 hex characters are replaced at the same width.
A 7-11 hex token that resolves to one kept old commit and is no issue ID prefix
is replaced by the prefix of its new ID, lengthened to the shortest length
unique among the kept new IDs. Ambiguous, pruned and unmapped references stay
for review. The TSV contains occurrences, with suggestions at the token's width.
JSON line numbers locate the source string, including JSON-escaped newlines.
check exits nonzero while mapped, short or pruned references remain; ambiguous
and unmapped references are reported without failing it.
"""
import argparse
from bisect import bisect_left
from collections import Counter
import csv
import fnmatch
import json
from pathlib import Path
import re
import subprocess
import sys

from map_pins import kept_ancestor, load_map

TOKEN = re.compile(r"(?<![0-9a-fA-F\w])[0-9a-f]{7,40}(?![0-9a-fA-F\w])")
JSON_STRING = re.compile(r'"(?:[^"\\]|\\.)*"')
CLASSES = ("mapped", "pruned", "short", "ambiguous", "unmapped")
MAX_TEXT_BYTES = 2 * 1024 * 1024


def string_values(value, trail=()):
    if isinstance(value, str):
        yield trail, value
    elif isinstance(value, dict):
        for key, child in value.items():
            yield from string_values(child, trail + (key,))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from string_values(child, trail + (index,))


class Source:
    def __init__(self, path, issue=False):
        self.path = path
        self.text = path.read_bytes().decode("utf-8")
        self.issue = json.loads(self.text) if issue else None
        if issue:
            # Lexical values and decoded leaves have the same order. Keys are
            # followed by a colon; their contents are never reference prose.
            values = [m for m in JSON_STRING.finditer(self.text)
                      if not self.text[m.end():].lstrip().startswith(":")]
            leaves = list(string_values(self.issue))
            if len(leaves) != len(values):
                raise SystemExit(f"{path}: {len(values)} lexical strings, "
                                 f"{len(leaves)} decoded values")
            self.strings = [
                (trail, value, self.text.count("\n", 0, match.start()) + 1)
                for (trail, value), match in zip(leaves, values)
                if not (len(trail) == 3 and trail[0] == "documents"
                        and isinstance(trail[1], int) and trail[2] == "commit")
            ]
        else:
            self.strings = [((), self.text, 1)]

    def write(self, replacements):
        if self.issue is None:
            text = replacements[()]
        else:
            for trail, value in replacements.items():
                parent = self.issue
                for key in trail[:-1]:
                    parent = parent[key]
                parent[trail[-1]] = value
            text = json.dumps(self.issue, indent=2, ensure_ascii=False)
            text += "\n" if self.text.endswith("\n") else ""
        self.path.write_bytes(text.encode("utf-8"))


def excluded(path, checkout, globs):
    try:
        name = path.resolve().relative_to(checkout.resolve()).as_posix()
    except ValueError:
        name = path.as_posix()
    return any(fnmatch.fnmatch(name, glob) for glob in globs)


def sources(checkout, data_root, globs=()):
    issues = sorted(p for p in (data_root / "issues").glob("*.json")
                    if not excluded(p, checkout, globs)) if data_root else []
    issue_paths = {p.resolve() for p in issues}
    tracked = subprocess.run(
        ["git", "-C", str(checkout), "ls-files", "-z"],
        check=True, capture_output=True,
    ).stdout.split(b"\0")
    for name in tracked:
        if not name:
            continue
        path = checkout / name.decode("utf-8", errors="surrogateescape")
        if (excluded(path, checkout, globs) or path.is_symlink() or not path.is_file() or path.resolve() in issue_paths
                or path.stat().st_size > MAX_TEXT_BYTES):
            continue
        try:
            source = Source(path)
        except UnicodeDecodeError:
            continue
        if "\0" not in source.text:
            yield source
    for path in issues:
        yield Source(path, issue=True)


def candidates(text, issue_ids):
    for match in TOKEN.finditer(text):
        token = match.group()
        if (text[:match.start()].endswith(("jit:", "@/issue/", "issue/"))
                or any(short.startswith(token) for short in issue_ids)):
            continue
        yield match


def resolve(tokens, original):
    tokens = sorted(tokens)
    if not tokens:
        return {}
    result = subprocess.run(
        ["git", "--git-dir", str(original), "cat-file", "--batch-check"],
        input="".join(token + "^{commit}\n" for token in tokens),
        check=True, capture_output=True, text=True,
    )
    # With ^{commit}, Git prints 'missing' on stdout even for ambiguity.
    ambiguous = set(re.findall(r"short object ID ([0-9a-f]+) is ambiguous", result.stderr))
    resolved = {}
    for token, line in zip(tokens, result.stdout.splitlines()):
        fields = line.split()
        if token in ambiguous or fields[-1] == "ambiguous":
            resolved[token] = "ambiguous"
        elif len(fields) == 3 and fields[1] == "commit":
            resolved[token] = fields[0]
    return resolved


def unique_prefix(new, width, kept):
    """Shortest prefix of `new` of at least `width` hex unique in sorted `kept`."""
    for length in range(width, len(new) + 1):
        prefix = new[:length]
        index = bisect_left(kept, prefix)
        if not (index + 1 < len(kept) and kept[index + 1].startswith(prefix)):
            return prefix
    return None


def classify(commit, token, mapping, original, ancestors, kept):
    if commit == "ambiguous":
        return "ambiguous", ""
    new = mapping.get(commit)
    if new is None:
        return "unmapped", ""
    if set(new) == {"0"}:
        if commit not in ancestors:
            ancestors[commit] = kept_ancestor(commit, str(original), mapping)
        return "pruned", (ancestors[commit] or "")[:len(token)]
    if len(token) >= 12:
        return "mapped", new[:len(token)]
    prefix = unique_prefix(new, len(token), kept)
    return ("short", new[:len(token)]) if prefix is None else ("mapped", prefix)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("inventory", "apply", "check"))
    parser.add_argument("commit_map", type=Path)
    parser.add_argument("original_git_dir", type=Path)
    parser.add_argument("checkout", type=Path)
    parser.add_argument("data_root", nargs="?", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--exclude", action="append", default=[], metavar="GLOB")
    args = parser.parse_args()
    mapping = load_map(args.commit_map)
    issue_ids = {p.name[:8] for p in (args.data_root / "issues").glob("*.json")
                 if re.fullmatch(r"[0-9a-f]{8}", p.name[:8])} if args.data_root else set()
    inputs = list(sources(args.checkout, args.data_root, args.exclude))
    tokens = {m.group() for source in inputs for _, text, _ in source.strings
              for m in candidates(text, issue_ids)}
    resolved = resolve(tokens, args.original_git_dir)
    ancestors = {}
    kept = sorted({new for new in mapping.values() if set(new) != {"0"}})
    classes = {token: classify(commit, token, mapping, args.original_git_dir, ancestors, kept)
               for token, commit in resolved.items()}
    rows = []
    for source in inputs:
        replacements = {}
        for trail, text, base_line in source.strings:
            matches = [m for m in candidates(text, issue_ids) if m.group() in classes]
            for match in matches:
                kind, suggestion = classes[match.group()]
                line = base_line if source.issue is not None else (
                    base_line + text.count("\n", 0, match.start()))
                rows.append((str(source.path), line, match.group(), kind, suggestion))
            if args.mode == "apply":
                updated = text
                for match in reversed(matches):
                    kind, suggestion = classes[match.group()]
                    if kind == "mapped":
                        updated = updated[:match.start()] + suggestion + updated[match.end():]
                if updated != text:
                    replacements[trail] = updated
        if replacements:
            source.write(replacements)
    if args.out or args.mode == "inventory":
        out = args.out.open("w", newline="") if args.out else sys.stdout
        try:
            writer = csv.writer(out, delimiter="\t", lineterminator="\n")
            writer.writerow(("file", "line", "token", "class", "suggestion"))
            writer.writerows(rows)
        finally:
            if args.out:
                out.close()
    counts = Counter(row[3] for row in rows)
    stream = sys.stderr if args.mode == "inventory" and not args.out else sys.stdout
    print(" ".join(f"{kind}={counts[kind]}" for kind in CLASSES)
          + f" total={len(rows)}", file=stream)
    if args.mode == "apply":
        print(f"rewrote {counts['mapped']} mapped references")
    return int(args.mode == "check" and any(
        counts[kind] > 0 for kind in ("mapped", "short", "pruned")))


if __name__ == "__main__":
    sys.exit(main())
