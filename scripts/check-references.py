#!/usr/bin/env python3
"""Refuses host-local paths and repository paths that do not resolve.

Enforces `@/inv/resolvable-references` over tracked text. Reasoned exceptions
live in `reference-exceptions.toml` beside this script.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

EXCEPTIONS = "scripts/reference-exceptions.toml"
MAX_TEXT_BYTES = 4 * 1024 * 1024
HOST_LOCAL = re.compile(
    r"(?<![\w.-])/home/(?!example\b)[a-z_][\w-]*/|/Users/[A-Za-z]\w*/|/run/user/\d|/tmp/claude-"
    r"|(?<![\w.-])/(?:mnt|root|data|scratch)/\w"
    # The toolchain's own directories exist wherever the toolchain does.
    r"|(?<![\w.])(?:~|\$HOME|\$\{HOME\})/(?!\.cargo\b|\.rustup\b|\.elan\b)[\w.]"
)
ROOTS = r"(?:\.agents|\.cargo|\.config|\.github|\.jit|benchmarks|contrib|crates|dev|docs|experiments|packages|proofs|scripts)"
REPOSITORY_PATH = re.compile(rf"(?<![\w./+~}}-])/?({ROOTS}/[\w.+=-]+(?:/[\w.+=-]+)*)")
RELATIVE_PATH = re.compile(r"(?<![\w./+-])((?:\.\./)+[\w.+=-]+(?:/[\w.+=-]+)*)")
INLINE_PATH = re.compile(r"`((?!10\.\d)[\w.+=-]+(?:/[\w.+=-]+)*/[\w+=-]+\.\w+)`")
MARKDOWN_LINK = re.compile(r"\]\((?!https?:|mailto:|#)([^)#\s]+)")
CARGO_PATH = re.compile(r"""\bpath\s*=\s*"([^"]+)\"""")
RUST_INCLUDE = re.compile(r"""include(?:_str|_bytes)?!\(\s*"([^"]+)"\s*\)""")
DOT_PATH = re.compile(r"(?<![\w./+-])\./([\w.+=-]+(?:/[\w.+=-]+)+)")
TEMPLATE = re.compile(r"[<>{}*$]")
DEVICE = re.compile(r"dev/(?:null|zero|shm|stdin|stdout|stderr|fd|tty|kfd|dri|urandom|random|cpu|mem)\b")


OPEN_STATES = {"backlog", "ready", "in_progress"}
# Code resolves `../` from its working directory at run time, so only prose is read.
CODE_SUFFIXES = (".rs", ".py", ".sh", ".toml", ".c", ".cpp", ".h", ".lean", ".json", ".jsonl", ".yml")


def git(root: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(root), *args], check=True, capture_output=True, text=True
    ).stdout


def resolvable_set(tracked: list[str]) -> set[str]:
    known = set(tracked)
    for path in tracked:
        parts = path.split("/")
        known.update("/".join(parts[:i]) for i in range(1, len(parts)))
    return known


def faults_in(path: str, text: str, known: set[str], base: str | None = None) -> list[str]:
    """`base` is the directory relative references resolve against.

    Passing it marks `text` as Markdown prose whatever the file's suffix.
    """
    faults = []
    markdown = base is not None or path.endswith(".md")
    prose = markdown or not path.endswith(CODE_SUFFIXES)
    base = str(Path(path).parent) if base is None else base
    for number, line in enumerate(text.splitlines(), 1):
        if HOST_LOCAL.search(line):
            faults.append(f"{path}:{number}: host-local path")
        for match in REPOSITORY_PATH.finditer(line):
            end = match.end()
            if line[end : end + 1] in ("*", "{", "<", "$"):
                continue
            target = match.group(1).rstrip(".,:;")
            if DEVICE.match(target):
                continue
            if not TEMPLATE.search(target) and target not in known:
                faults.append(f"{path}:{number}: {target}")
        if path.endswith("Cargo.toml"):
            for dependency in CARGO_PATH.findall(line):
                target = str(Path(*_normal(f"{base}/{dependency}")))
                if target not in known:
                    faults.append(f"{path}:{number}: {target}")
        if path.endswith(".rs"):
            for included in RUST_INCLUDE.findall(line):
                target = str(Path(*_normal(f"{base}/{included}")))
                if target not in known:
                    faults.append(f"{path}:{number}: {target}")
        for dotted in DOT_PATH.findall(line):
            if dotted not in known and prose:
                faults.append(f"{path}:{number}: {dotted}")
        for relative in RELATIVE_PATH.findall(line):
            target = str(Path(*_normal(f"{base}/{relative}")))
            if prose and target not in known:
                faults.append(f"{path}:{number}: {target}")
        if markdown:
            for inline in INLINE_PATH.findall(line):
                if ".." not in inline and not _suffix_known(inline.removeprefix("./"), known):
                    faults.append(f"{path}:{number}: {inline}")
            for link in MARKDOWN_LINK.findall(line):
                target = link[1:] if link.startswith("/") else f"{base}/{link}"
                normal = str(Path(*_normal(target)))
                if not TEMPLATE.search(link) and normal not in known:
                    faults.append(f"{path}:{number}: {link}")
    return faults


def issue_faults(path: str, text: str, known: set[str]) -> list[str]:
    """An issue's references resolve from the repository root; its links must exist."""
    issue = json.loads(text)
    prose = "\n".join(filter(None, [issue.get("description"), issue.get("context") if isinstance(issue.get("context"), str) else None]))
    faults = faults_in(path, prose, known, base="")
    faults += [
        f"{path}:0: {document['path']}"
        for document in issue.get("documents", [])
        if document["path"] not in known
    ]
    return faults


def _suffix_known(inline: str, known: set[str]) -> bool:
    return inline in known or any(path.endswith("/" + inline) for path in known)


def _normal(target: str) -> list[str]:
    parts: list[str] = []
    for part in target.split("/"):
        if part == "..":
            if parts:
                parts.pop()
        elif part not in ("", "."):
            parts.append(part)
    return parts or ["."]


def excepted(root: Path, fault: str, exceptions: list[dict[str, str]]) -> bool:
    """An entry exempts its files; with `planned`, only paths of files to come."""
    path, target = fault.split(":", 1)[0], fault.rsplit(": ", 1)[1]
    for entry in exceptions:
        if fnmatch.fnmatch(path, entry["path"]):
            planned = entry.get("planned")
            if planned is None or (target.startswith(tuple(planned)) and _open_issue(root, path)):
                return True
    return False


def _open_issue(root: Path, path: str) -> bool:
    return json.loads((root / path).read_text()).get("state") in OPEN_STATES


def ignored(root: Path, target: str) -> bool:
    # A directory rule matches only with the trailing slash.
    return any(
        subprocess.run(
            ["git", "-C", str(root), "check-ignore", "-q", "--no-index", candidate],
            capture_output=True,
        ).returncode
        == 0
        for candidate in (target, target + "/")
    )


def scan(path: str, text: str, known: set[str]) -> list[str]:
    if fnmatch.fnmatch(path, ".jit/issues/*.json"):
        return issue_faults(path, text, known)
    return faults_in(path, text, known)


def check(root: Path) -> list[str]:
    tracked = git(root, "ls-files", "-z").split("\0")[:-1]
    known = resolvable_set(tracked)
    config = root / EXCEPTIONS
    exceptions = (
        tomllib.loads(config.read_text())["exception"] if config.exists() else []
    )
    faults = []
    for path in tracked:
        file = root / path
        if file.is_symlink() or not file.is_file():
            continue
        if file.stat().st_size > MAX_TEXT_BYTES:
            continue
        try:
            text = file.read_text()
        except UnicodeDecodeError:
            continue
        faults += [
            fault for fault in scan(path, text, known)
            if not excepted(root, fault, exceptions)
            and not ignored(root, fault.rsplit(": ", 1)[1])
        ]
    return faults


def self_test() -> None:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        git(root, "init", "-q")
        (root / "docs").mkdir()
        (root / "docs/good.md").write_text("[a](bad-link.md) docs/good.md\n")
        (root / "docs/host.md").write_text("see /home/alice/x and /home/example/y\n")
        (root / "docs/gone.md").write_text("see dev/gone/file.json\n")
        (root / "docs/relative.md").write_text("see ../missing/file.txt and ../docs/good.md\n")
        (root / "docs/inline.md").write_text("see `gone/file.json` and `docs/good.md`\n")
        (root / "docs/home.md").write_text("see $HOME/.local/share/x and crates/a/data/b and ../data/c\n")
        (root / "docs/tilde.md").write_text("see ~/vectors/x; ~/.cargo/bin is fine\n")
        (root / "docs/dot-root.md").write_text("see .jit/absent.toml\n")
        (root / "pkg").mkdir()
        (root / "pkg/Cargo.toml").write_text('a = { path = "../absent-crate" }\nb = { path = "../docs" }\n')
        (root / ".jit/issues").mkdir(parents=True)
        (root / ".jit/issues/a.json").write_text(json.dumps({
            "state": "done",
            "description": "first\ndev/gone/after-newline.md and [x](../issue-link.md) and `gone/issue.md`",
            "documents": [{"path": "dev/gone-document.md"}],
        }))
        (root / "docs/dot.md").write_text("run ./scripts/absent.sh\n")
        (root / "docs/rooted.md").write_text("[a](/docs/absent.md) [b](/docs/good.md)\n")
        (root / "src").mkdir()
        (root / "src/lib.rs").write_text('const A: &str = include_str!("../docs/absent.txt");\n')
        (root / ".gitignore").write_text("dev/ignored/\n")
        (root / "docs/ignored.md").write_text("see dev/ignored/file\n")
        git(root, "add", ".")
        faults = check(root)
    assert any("bad-link.md" in f for f in faults), faults
    assert sum("host-local" in f for f in faults) == 3, faults
    assert sum("tilde.md" in f for f in faults) == 1, faults
    assert any("dot-root.md" in f and ".jit/absent.toml" in f for f in faults), faults
    assert any("pkg/Cargo.toml" in f and "absent-crate" in f for f in faults), faults
    assert not any("pkg/Cargo.toml" in f and "docs" in f for f in faults), faults
    for target in ("dev/gone/after-newline.md", "issue-link.md", "gone/issue.md", "dev/gone-document.md"):
        assert any(f.startswith(".jit/issues/a.json") and target in f for f in faults), (target, faults)
    assert any("missing/file.txt" in f for f in faults), faults
    assert any("scripts/absent.sh" in f for f in faults), faults
    assert any("rooted.md" in f and "absent.md" in f for f in faults), faults
    assert not any("rooted.md" in f and "good.md" in f for f in faults), faults
    assert any("src/lib.rs" in f and "docs/absent.txt" in f for f in faults), faults
    assert not any("../docs/good.md" in f for f in faults), faults
    assert any("gone/file.json" in f and "inline" in f for f in faults), faults
    assert not any("docs/good.md" in f and "inline" in f for f in faults), faults
    assert any("dev/gone/file.json" in f for f in faults), faults
    assert not any("ignored.md" in f for f in faults), faults


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    if parser.parse_args().self_test:
        self_test()
        print("check-references: self-test ok")
        return 0
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
    faults = check(root)
    for fault in faults:
        print(fault)
    print(f"check-references: {len(faults)} fault(s)" if faults else "check-references: ok")
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
