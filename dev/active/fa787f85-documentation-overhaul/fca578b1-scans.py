#!/usr/bin/env python3
"""Pattern scans over the permanent documentation surface.

Usage: fca578b1-scans.py <scan>, with <scan> one of the keys of SCANS, or
`surface` to print the scanned files. Output goes to stdout as
`<file>:<line>: <matched text>: <source line>`, followed by one `hits: <n>`
line. The surface is derived from the Git index at run time: Markdown pages
are scanned whole, a crate-root `lib.rs` contributes its `//!` lines only, and
a symbolic link is listed with its target and scanned through that target.

A scan reports text that matches a pattern. It does not judge the match and
does not find a defect that its patterns do not spell.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

PAGES = ["README.md", "docs", "crates/*/README.md", "AGENTS.md", "CLAUDE.md", "crates/*/AGENTS.md"]
CRATE_ROOTS = ["crates/*/src/lib.rs"]
EVIDENCE_PAGE = "docs/reference/performance-evidence.md"

NARRATION = (
    r"\b(?:now|new|newly|old|older|previous|previously|former|formerly|no longer|currently|"
    r"yet|will|won't|shall|planned|plans?|plan to|future|upcoming|soon|eventually|later|"
    r"legacy|migrat\w*|deprecat\w*|supersed\w*|obsolete|originally|initially|earlier|recently|"
    r"used to|for now|temporar\w*|interim|todo|fixme|wip|roadmap|non-goals?|out of scope|"
    r"not supported yet|intend\w*|aims?|hope\w*|expect(?:ed)? to|should eventually|"
    r"was|were|has been|have been|had been|became|replac\w*|renam\w*|remov\w*|rewrit\w*|rewrote)\b"
)
MARKETING = (
    r"\b(?:fast|fastest|efficient\w*|high-performance|state[- ]of[- ]the[- ]art|powerful|simple|simply|"
    r"easy|easily|robust\w*|seamless\w*|blazing\w*|best|optimal\w*|cutting[- ]edge|elegant\w*|modern|"
    r"rich|comprehensive|world[- ]class|lightweight|production[- ]grade|production[- ]ready|"
    r"research[- ]grade|significant\w*|dramatic\w*|superior|unmatched|highly|extremely|very|"
    r"excellent|great|advanced|novel|unique|flexible|scalable|rigorous\w*|clean)\b"
)
UNITS = (
    r"x|×|\\times|ns|µs|us|ms|s|sec|secs|seconds?|min|minutes?|hours?|"
    r"[kKMGT]i?B(?:/s)?|[kMG]bit/s|[kMG]bps|fps|Hz|[kMG]Hz|%|percent|cores?|threads?|workers?|frames?"
)
FIGURES = (
    rf"(?:(?<![\w.])\$?\d[\d.,\\ ]*\$?\s?(?:{UNITS})(?![\w/]))|"
    r"\b(?:speed-?ups?|faster|slower|throughput|latency|crossovers?|wall-clock|elapsed|"
    r"per second|overhead|scal(?:es|ing|ability)|feasib\w*|infeasib\w*|tractab\w*|intractab\w*)\b"
)
ISSUE_REFS = (
    r"@/issue/[^\s`)]*|\bjit:[^\s`)]+|\bissues? +#?[0-9a-f]{6,}\b|(?<![\w/&])#\d+\b|"
    r"(?<![\w@/.-])(?=[0-9a-f]*\d)(?=[0-9a-f]*[a-f])[0-9a-f]{8}(?![\w])"
)
DEV_PATHS = r"(?<![\w.-])dev/[\w./*<>-]*"
PLAIN_MATH = r"\bGF\(|\bF_\{?\w|\b\w+\^[\w({]|<=|>=|≤|≥|\bmod\b"
STANDARDS = {
    "@/citation/Etsi2015": r"DVB-T2|EN 302 755",
    "@/citation/ThreeGpp2017": r"\b5G\b|\bNR\b|38\.212",
}

CODE_SPAN = re.compile(r"`[^`\n]*`")
LATEX = re.compile(r"\$[^$\n]*\$")
FENCE = re.compile(r"^\s*(?:```|~~~)")
RUSTDOC = re.compile(r"^//! ?(.*)$")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


ROOT = Path(git("rev-parse", "--show-toplevel").strip())


def tracked() -> list[str]:
    listed = git("-C", str(ROOT), "ls-files", "--", *PAGES, *CRATE_ROOTS)
    return sorted(set(listed.split()))


def surface() -> list[str]:
    return [path for path in tracked() if not (ROOT / path).is_symlink()]


def lines(path: str):
    """Yield (number, text, fenced) for the documentation lines of one surface file."""
    text = (ROOT / path).read_text(encoding="utf-8")
    fenced = False
    for number, line in enumerate(text.splitlines(), 1):
        if path.endswith(".rs"):
            match = RUSTDOC.match(line)
            if not match:
                continue
            line = match.group(1)
        if FENCE.match(line):
            fenced = not fenced
            continue
        yield number, line, fenced


def report(hits: list[tuple[str, int, str, str]]) -> None:
    for path, number, matched, line in hits:
        print(f"{path}:{number}: {matched}: {line.strip()}")
    print(f"hits: {len(hits)}")


def pattern_scan(pattern: str, *, fenced: bool, spans: bool, flags: int = re.IGNORECASE):
    """Scan every surface file; `fenced` includes fenced blocks, `spans` includes code spans and LaTeX."""
    regex = re.compile(pattern, flags)
    hits = []
    for path in surface():
        for number, line, in_fence in lines(path):
            if in_fence and not fenced:
                continue
            text = line if spans else LATEX.sub(" ", CODE_SPAN.sub(" ", line))
            for match in regex.finditer(text):
                hits.append((path, number, match.group(0), line))
    return hits


def figures():
    """Figure and performance-word matches, with the evidence page reported apart."""
    hits = pattern_scan(FIGURES, fenced=False, spans=True)
    outside = [hit for hit in hits if hit[0] != EVIDENCE_PAGE]
    inside = len(hits) - len(outside)
    print(f"# {EVIDENCE_PAGE}: {inside} matches, not listed")
    return outside


def standards():
    """Mentions of a standard in a file that does not cite the standard's registry key."""
    hits = []
    for path in surface():
        body = [(number, line) for number, line, _ in lines(path)]
        whole = "\n".join(line for _, line in body)
        for key, pattern in STANDARDS.items():
            if key in whole:
                continue
            regex = re.compile(pattern)
            for number, line in body:
                for match in regex.finditer(line):
                    hits.append((path, number, f"{match.group(0)} without {key}", line))
    return hits


SCANS = {
    "narration": lambda: pattern_scan(NARRATION, fenced=False, spans=False),
    "marketing": lambda: pattern_scan(MARKETING, fenced=False, spans=False),
    "figures": figures,
    "issue-refs": lambda: pattern_scan(ISSUE_REFS, fenced=True, spans=True, flags=0),
    "dev-paths": lambda: pattern_scan(DEV_PATHS, fenced=True, spans=True, flags=0),
    "plain-math": lambda: pattern_scan(PLAIN_MATH, fenced=False, spans=False, flags=0),
    "standards": standards,
}


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in {*SCANS, "surface"}:
        print(__doc__, file=sys.stderr)
        return 2
    if sys.argv[1] == "surface":
        for path in tracked():
            link = ROOT / path
            print(f"{path} -> {link.readlink()}" if link.is_symlink() else path)
        return 0
    report(SCANS[sys.argv[1]]())
    return 0


if __name__ == "__main__":
    sys.exit(main())
