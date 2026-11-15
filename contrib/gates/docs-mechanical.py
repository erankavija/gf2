#!/usr/bin/env python3
"""Mechanical documentation check over the footprint in `contrib/gates/docs-mechanical.toml`.

Settings keys: `markdown` and `rustdoc` globs, `citations` (the
Markdown files whose repository-path code spans are checked), `exclude` path
prefixes, and an optional `baseline` record whose table rows suppress findings.

Prints one `<file>:<line>: <class>: <target>` line per finding. Classes:
broken-link, broken-anchor, missing-citation, unresolved-item and
stale-projection. Item references resolve through `jit item show` (bare issue
addresses through `jit issue show`); projection freshness compares each
configured target with a `jit project render` run in a scratch copy. Missing
paths that git ignores are local runtime state, not findings.

The tracker data root is `JIT_DATA_DIR` when set, otherwise `.jit/` under
`--root` (the current checkout's top level by default). Configuration and
unignored tracker files come from that data root's
git repository. Rendering sets both `JIT_DATA_DIR` and `JIT_WORKTREE_ROOT` to
the scratch copy; item lookups use the source store and the checked code root.
Logical `.jit/` links, citations and projection targets address the data root.

Exit status: 0 pass, 1 findings, 2 environment error (configuration, jit, git).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import urllib.parse
from pathlib import Path

FENCE = re.compile(r"^\s*(```|~~~)")
INLINE_LINK = re.compile(r"!?\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^()\s<>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
REF_DEFINITION = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?")
CODE_SPAN = re.compile(r"`([^`\n]+)`")
CITATION = re.compile(r"[\w.\-]+(?:/[\w.\-]+)+/?")
BARE_FILE = re.compile(r"[\w-][\w.-]*\.[a-z][a-z0-9]{0,4}")
ITEM_REF = re.compile(r"(?<![\w.@])@[a-z0-9-]*/[a-z]+/[A-Za-z0-9][\w.-]*(?:/[a-z]+/[A-Za-z0-9][\w.-]*)?")
SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
HEADING = re.compile(r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$")
HTML_ANCHOR = re.compile(r"<a\s[^>]*\b(?:id|name)=\"([^\"]+)\"")
RUSTDOC = re.compile(r"^\s*//[/!] ?(.*)$")
LINE_SUFFIX = re.compile(r":\d+(?:-\d+)?$")
LINE_ANCHOR = re.compile(r"^L(\d+)(?:-L(\d+))?$")
BASELINE_ROW = re.compile(r"^\|\s*`([^`]+):(\d+)`\s*\|\s*`([a-z-]+)`\s*\|\s*`([^`]+)`\s*\|")
UNRESOLVED_CODES = {"ITEM_COMMAND_FAILED", "ISSUE_NOT_FOUND"}
SETTINGS = Path("contrib/gates/docs-mechanical.toml")


class EnvironmentFailure(Exception):
    pass


def slug(heading: str) -> str:
    """GitHub heading anchor: rendered text, lowercased, punctuation dropped, spaces to hyphens."""
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", heading)
    text = re.sub(r"<[^>]+>", "", text).strip().lower()
    return re.sub(r"[^\w\- ]", "", text).replace(" ", "-")


def markdown_lines(text: str):
    fenced = False
    for number, line in enumerate(text.splitlines(), 1):
        if FENCE.match(line):
            fenced = not fenced
        elif not fenced:
            yield number, line


def rustdoc_lines(text: str):
    fenced = False
    for number, line in enumerate(text.splitlines(), 1):
        match = RUSTDOC.match(line)
        if not match:
            continue
        if FENCE.match(match.group(1)):
            fenced = not fenced
        elif not fenced:
            yield number, match.group(1)


def link_targets(line: str):
    stripped = CODE_SPAN.sub("", line)
    yield from INLINE_LINK.findall(stripped)
    definition = REF_DEFINITION.match(stripped)
    if definition:
        yield definition.group(1)


def failure_text(result: subprocess.CompletedProcess) -> str:
    return f"exit {result.returncode}: {(result.stderr or result.stdout).strip()[:400]}"


class Checker:
    def __init__(self, root: Path, jit: str):
        self.root = root
        self.data_root = Path(os.environ.get("JIT_DATA_DIR", root / ".jit")).resolve()
        self.jit = jit
        self.findings: set[tuple[str, int, str, str]] = set()
        self.missing: list[tuple[Path, int, str, str, str]] = []
        self.references: dict[str, list[tuple[Path, int]]] = {}
        self.anchor_cache: dict[Path, set[str]] = {}
        self.names: set[str] | None = None
        self.top_level = {entry.name for entry in root.iterdir()} - {".git"}
        self.top_level.add(".jit")

    def repository_path(self, relative: str | Path) -> Path:
        """Logical `.jit/` paths address the selected data root, as they do in jit."""
        path = Path(relative)
        return self.data_root / path.relative_to(".jit") if path.is_relative_to(".jit") else self.root / path

    def report(self, path: Path, line: int, kind: str, target: str):
        self.findings.add((path.relative_to(self.root).as_posix(), line, kind, target))

    def anchors(self, path: Path) -> set[str]:
        if path not in self.anchor_cache:
            seen: dict[str, int] = {}
            anchors: set[str] = set()
            for _, line in markdown_lines(path.read_text(encoding="utf-8", errors="replace")):
                anchors.update(HTML_ANCHOR.findall(line))
                heading = HEADING.match(line)
                if heading:
                    base = slug(heading.group(1))
                    count = seen.get(base, 0)
                    seen[base] = count + 1
                    anchors.add(base if count == 0 else f"{base}-{count}")
            self.anchor_cache[path] = anchors
        return self.anchor_cache[path]

    def check_link(self, source: Path, line: int, target: str):
        if SCHEME.match(target):
            return
        raw_path, _, fragment = target.partition("#")
        path_part = urllib.parse.unquote(raw_path)
        if not path_part:
            resolved = source
        elif path_part.startswith("/"):
            resolved = (self.root / path_part.lstrip("/")).resolve()
        else:
            resolved = (source.parent / path_part).resolve()
        if not resolved.is_relative_to(self.root):
            self.report(source, line, "broken-link", target)
            return
        relative = resolved.relative_to(self.root).as_posix()
        resolved = self.repository_path(relative)
        if not resolved.exists():
            self.missing.append((source, line, "broken-link", target, relative))
        elif fragment and resolved.is_file():
            line_anchor = LINE_ANCHOR.match(fragment)
            if line_anchor:
                with resolved.open("rb") as handle:
                    if int(line_anchor.group(2) or line_anchor.group(1)) > sum(1 for _ in handle):
                        self.report(source, line, "broken-anchor", target)
            elif resolved.suffix == ".md" and urllib.parse.unquote(fragment).lower() not in self.anchors(resolved):
                self.report(source, line, "broken-anchor", target)

    def check_markdown(self, path: Path, cite: bool):
        for number, line in markdown_lines(path.read_text(encoding="utf-8", errors="replace")):
            for target in link_targets(line):
                self.check_link(path, number, target)
            for reference in ITEM_REF.findall(line):
                self.references.setdefault(reference.rstrip(".-_"), []).append((path, number))
            if not cite:
                continue
            for span in CODE_SPAN.findall(INLINE_LINK.sub("", line)):
                cited = LINE_SUFFIX.sub("", span.strip())
                segments = cited.rstrip("/").split("/")
                if CITATION.fullmatch(cited) and segments[0] in self.top_level and "..." not in segments:
                    if not self.repository_path(cited).exists():
                        self.missing.append((path, number, "missing-citation", span.strip(), cited))
                elif BARE_FILE.fullmatch(cited) and cited not in self.file_names():
                    self.report(path, number, "missing-citation", span.strip())

    def check_rustdoc(self, path: Path):
        """Checks file-relative links; intra-doc paths and rustdoc page links are rustdoc's own check."""
        for number, line in rustdoc_lines(path.read_text(encoding="utf-8", errors="replace")):
            for target in link_targets(line):
                last = target.partition("#")[0].rsplit("/", 1)[-1]
                if "::" in target or target.startswith("#") or last.endswith(".html"):
                    continue
                if "/" in target or "." in last:
                    self.check_link(path, number, target)

    def git(self, *arguments: str, stdin: str = "", ok=(0,), cwd: Path | None = None) -> str:
        try:
            result = subprocess.run(["git", *arguments], cwd=cwd or self.root, input=stdin, capture_output=True, text=True)
        except OSError as error:
            raise EnvironmentFailure(f"cannot run git: {error}") from error
        if result.returncode not in ok:
            raise EnvironmentFailure(f"git {arguments[0]} failed ({failure_text(result)})")
        return result.stdout

    def file_names(self) -> set[str]:
        """Bare file names cite a file by name, so they resolve against every unignored file's name."""
        if self.names is None:
            paths = self.unignored_files(".") + self.unignored_files(".", cwd=self.data_root)
            self.names = {relative.rsplit("/", 1)[-1] for relative in paths}
        return self.names

    def unignored_files(self, directory: str, cwd: Path | None = None) -> list[str]:
        return self.git("ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", directory,
                        cwd=cwd).split("\0")[:-1]

    def report_missing(self):
        if not self.missing:
            return
        for tracker, directory in ((False, self.root), (True, self.data_root)):
            missing = [(entry, Path(entry[-1]).relative_to(".jit").as_posix() if tracker else entry[-1])
                       for entry in self.missing if Path(entry[-1]).is_relative_to(".jit") == tracker]
            if not missing:
                continue
            queries = "".join(f"{p}\n{p.rstrip('/')}/\n" for _, p in missing)
            ignored = {line.rstrip("/") for line in self.git("check-ignore", "--stdin", stdin=queries,
                                                            ok=(0, 1), cwd=directory).splitlines()}
            for (source, line, kind, target, _), relative in missing:
                if relative.rstrip("/") not in ignored:
                    self.report(source, line, kind, target)

    def run_jit(self, cwd: Path, *arguments: str, data_root: Path | None = None) -> subprocess.CompletedProcess:
        environment = {**os.environ, "JIT_DATA_DIR": str(data_root or self.data_root),
                       "JIT_WORKTREE_ROOT": str(cwd)}
        try:
            return subprocess.run([self.jit, *arguments], cwd=cwd, env=environment,
                                  capture_output=True, text=True, timeout=300)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise EnvironmentFailure(f"cannot run {self.jit}: {error}") from error

    def resolves(self, reference: str) -> bool:
        """True when jit resolves the address; only jit's not-found codes count as unresolved."""
        parts = reference.split("/")
        if parts[1] == "issue" and len(parts) == 3:
            result = self.run_jit(self.root, "issue", "show", parts[2], "--json")
        else:
            result = self.run_jit(self.root, "item", "show", reference, "--json")
        try:
            payload = json.loads(result.stdout)
        except json.JSONDecodeError:
            payload = None
        if result.returncode == 0 and isinstance(payload, dict) and "error" not in payload:
            return True
        error = payload.get("error") if isinstance(payload, dict) else None
        if result.returncode != 0 and isinstance(error, dict) and error.get("code") in UNRESOLVED_CODES:
            return False
        raise EnvironmentFailure(f"jit failed resolving {reference} ({failure_text(result)})")

    def check_projections(self, targets: list[str]):
        if not targets:
            return
        with tempfile.TemporaryDirectory(prefix="gf2-docs-mechanical-") as scratch:
            copy = Path(scratch)
            data_copy = copy / ".jit"
            files = [(self.data_root / relative, data_copy / relative)
                     for relative in self.unignored_files(".", cwd=self.data_root)]
            files += [(self.repository_path(target), copy / target) for target in targets]
            for source, destination in files:
                if source.is_file() and not destination.exists():
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, destination)
            result = self.run_jit(copy, "project", "render", "--json", data_root=data_copy)
            if result.returncode != 0:
                raise EnvironmentFailure(f"jit project render failed ({failure_text(result)})")
            for target in targets:
                if not (copy / target).is_file():
                    raise EnvironmentFailure(f"jit project render produced no {target}")
                rendered = (copy / target).read_bytes().splitlines(keepends=True)
                current_path = self.repository_path(target)
                current = current_path.read_bytes().splitlines(keepends=True) if current_path.is_file() else []
                if current != rendered:
                    line = next((i for i, (a, b) in enumerate(zip(current, rendered), 1) if a != b),
                                min(len(current), len(rendered)) + 1)
                    self.report(self.root / target, line, "stale-projection", target)


def collect(root: Path, patterns: list[str], excluded: list[Path]) -> list[Path]:
    seen: dict[Path, Path] = {}
    for pattern in patterns:
        for path in sorted(root.glob(pattern)):
            if path.is_file() and not any(path.is_relative_to(prefix) for prefix in excluded):
                seen.setdefault(path.resolve(), path)
    return list(seen.values())


def load_baseline(path: Path | None) -> set[tuple[str, int, str, str]]:
    """Reads `| `file:line` | `class` | `target` |` rows from the baseline record."""
    if path is None:
        return set()
    if not path.is_file():
        raise EnvironmentFailure(f"baseline record {path} is missing")
    rows = (BASELINE_ROW.match(line) for line in path.read_text(encoding="utf-8").splitlines())
    return {(m.group(1), int(m.group(2)), m.group(3), m.group(4)) for m in rows if m}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, help="code checkout root (default: git top level)")
    parser.add_argument("--jit", default="jit")
    args = parser.parse_args()
    try:
        root = args.root.resolve() if args.root else Path(subprocess.run(
            ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True).stdout.strip())
        checker = Checker(root, args.jit)
        config = tomllib.loads((checker.data_root / "config.toml").read_text(encoding="utf-8"))
        settings = tomllib.loads((root / SETTINGS).read_text(encoding="utf-8"))
        excluded = [root / prefix for prefix in settings.get("exclude", [])]
        baseline_path = root / settings["baseline"] if "baseline" in settings else None
        markdown = [p for p in collect(root, settings.get("markdown", []), excluded) if p != baseline_path]
        cited = set(collect(root, settings.get("citations", []), excluded))
        rustdoc = collect(root, settings.get("rustdoc", []), excluded)
        checker.check_projections([table["target"] for table in config.get("projection", {}).values()])
        for path in markdown:
            checker.check_markdown(path, path in cited)
        for path in rustdoc:
            checker.check_rustdoc(path)
        checker.report_missing()
        for reference, sites in sorted(checker.references.items()):
            if not checker.resolves(reference):
                for path, line in sites:
                    checker.report(path, line, "unresolved-item", reference)
        baseline = load_baseline(baseline_path)
    except (OSError, KeyError, tomllib.TOMLDecodeError, subprocess.CalledProcessError, EnvironmentFailure) as error:
        print(f"docs-mechanical: environment error: {error!r}", file=sys.stderr)
        return 2
    findings = sorted(checker.findings - baseline)
    for file, line, kind, target in findings:
        print(f"{file}:{line}: {kind}: {target}")
    print(f"docs-mechanical: {'FAIL' if findings else 'PASS'}: {len(findings)} finding(s), "
          f"{len(checker.findings & baseline)} baselined; {len(markdown)} Markdown files, "
          f"{len(rustdoc)} Rust sources, {len(checker.references)} item references")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
