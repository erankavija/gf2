#!/usr/bin/env python3
"""Report word runs that a permanent page shares with its prior sources.

Usage: fca578b1-verbatim.py [N]. Reads `fca578b1-verbatim-sources.tsv` beside
this script: one `page<TAB>revision<TAB>pathspec` row per source set, the page
read from the working tree and the sources from the revision's tree, where
`**/` in a pathspec spans directories and `*` stays inside one component.
Prints, per page, the number of source files read and every maximal run of at
least N consecutive prose words (default DEFAULT_N) that also occurs in a
source, with one source that holds it.

Prose is what remains after removing fenced blocks, code spans, LaTeX spans,
link targets, item addresses and URLs; each removed span leaves a break that
no run crosses. Words are compared lowercased with
surrounding punctuation stripped; `.`, `/`, `-` and `'` join the word
characters around them, so `38.212` and `1/2` are single words. A `.rs` file contributes its `//!` and `///`
lines, and a `.rs` page its leading `//!` lines. Runs do not cross a blank
line, heading, list item or table cell.

The comparison establishes the absence of shared runs of N words. It does not
detect paraphrase, reordered sentences or runs shorter than N.
"""

from __future__ import annotations

import csv
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_N = 7
BREAK = "\x00"
FENCE = re.compile(r"^\s*(?:```|~~~)")
RUSTDOC = re.compile(r"^\s*//[/!] ?(.*)$")
REMOVED = re.compile(
    r"`[^`\n]*`"  # code span
    r"|\$[^$\n]*\$"  # LaTeX span
    r"|\]\([^)\s]*\)"  # link target
    r"|<[^>\s]+>"  # autolink or HTML tag
    r"|@[a-z0-9-]*/[a-z]+/[\w.-]+"  # item address
    r"|https?://\S+"
)
BOUNDARY = re.compile(r"^\s*(?:#{1,6}\s|[-*+]\s|\d+\.\s|>|$)")
WORD = re.compile(r"\w+(?:[./'’-]\w+)*")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


ROOT = Path(git("rev-parse", "--show-toplevel").strip())


def doc_lines(text: str, rust: bool, leading_only: bool = False):
    """Yield Markdown lines; for Rust, doc-comment bodies with a blank line for every other line."""
    for line in text.splitlines():
        if not rust:
            yield line
        elif leading_only:
            if line.startswith("//!"):
                yield RUSTDOC.match(line).group(1)
            elif line.strip() and not line.startswith("#!"):
                return
            else:
                yield ""
        else:
            match = RUSTDOC.match(line)
            yield match.group(1) if match else ""


def segments(text: str, rust: bool, leading_only: bool = False) -> list[list[str]]:
    """Split documentation text into word lists that no run may cross."""
    stream: list[str] = []
    fenced = False
    for line in doc_lines(text, rust, leading_only):
        if FENCE.match(line):
            fenced = not fenced
            stream.append(BREAK)
            continue
        if fenced:
            continue
        if BOUNDARY.match(line):
            stream.append(BREAK)
        line = REMOVED.sub(f" {BREAK} ", line).replace("|", f" {BREAK} ")
        for chunk in line.split():
            if BREAK in chunk:
                stream.append(BREAK)
            stream.extend(word.lower() for word in WORD.findall(chunk.replace(BREAK, " ")))
    result, current = [], []
    for token in stream:
        if token == BREAK:
            if current:
                result.append(current)
            current = []
        else:
            current.append(token)
    if current:
        result.append(current)
    return result


def glob_regex(pathspec: str) -> re.Pattern:
    """Translate a glob where `**/` spans directories and `*` stays inside one path component."""
    parts = re.split(r"(\*\*/|\*)", pathspec)
    body = "".join("(?:.*/)?" if part == "**/" else "[^/]*" if part == "*" else re.escape(part) for part in parts)
    return re.compile(body + r"\Z")


def source_files(revision: str, pathspec: str) -> list[str]:
    listed = git("-C", str(ROOT), "ls-tree", "-r", "--name-only", revision).splitlines()
    regex = glob_regex(pathspec)
    return sorted(path for path in listed if regex.match(path))


def blobs(revision: str, paths: list[str]):
    """Yield (path, text) for each path at the revision through one `git cat-file --batch`."""
    request = "".join(f"{revision}:{path}\n" for path in paths).encode()
    data = subprocess.run(
        ["git", "-C", str(ROOT), "cat-file", "--batch"], input=request, check=True, capture_output=True
    ).stdout
    offset = 0
    for path in paths:
        end = data.index(b"\n", offset)
        size = int(data[offset:end].split()[2])
        yield path, data[end + 1 : end + 1 + size].decode("utf-8", errors="replace")
        offset = end + 1 + size + 1


def source_index(revision: str, pathspec: str, n: int, cache: dict) -> tuple[int, dict]:
    """Map every n-gram of the source set to the first source file holding it."""
    key = (revision, pathspec)
    if key not in cache:
        index: dict[tuple[str, ...], str] = {}
        files = source_files(revision, pathspec)
        for path, text in blobs(revision, files):
            for words in segments(text, path.endswith(".rs")):
                for start in range(len(words) - n + 1):
                    index.setdefault(tuple(words[start : start + n]), path)
        cache[key] = (len(files), index)
    return cache[key]


def shared_runs(words: list[str], n: int, indexes: list[tuple[str, dict]]):
    """Yield (run, source) for each maximal run of words covered by shared n-grams."""
    hit: list[str | None] = []
    for start in range(len(words) - n + 1):
        gram = tuple(words[start : start + n])
        hit.append(next((f"{rev[:12]}:{idx[gram]}" for rev, idx in indexes if gram in idx), None))
    start = 0
    while start < len(hit):
        if hit[start] is None:
            start += 1
            continue
        end = start
        while end + 1 < len(hit) and hit[end + 1] is not None:
            end += 1
        yield " ".join(words[start : end + n]), hit[start]
        start = end + 1


def main() -> int:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_N
    table = Path(__file__).with_name("fca578b1-verbatim-sources.tsv")
    pages: dict[str, list[tuple[str, str]]] = {}
    with table.open(encoding="utf-8", newline="") as handle:
        for page, revision, pathspec in csv.reader(handle, delimiter="\t"):
            pages.setdefault(page, []).append((revision, pathspec))
    cache: dict = {}
    total = 0
    print(f"N = {n}")
    for page, sources in pages.items():
        indexes, files = [], 0
        for revision, pathspec in sources:
            count, index = source_index(revision, pathspec, n, cache)
            files += count
            indexes.append((revision, index))
        text = (ROOT / page).read_text(encoding="utf-8")
        rust = page.endswith(".rs")
        runs = [run for words in segments(text, rust, leading_only=rust) for run in shared_runs(words, n, indexes)]
        total += len(runs)
        print(f"{page}: {files} source files, {len(runs)} shared runs")
        for run, source in runs:
            print(f"  [{source}] {run}")
    print(f"pages: {len(pages)}, shared runs: {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
