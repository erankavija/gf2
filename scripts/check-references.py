#!/usr/bin/env python3
"""Refuses host-local paths and repository paths that do not resolve.

Enforces `@/inv/resolvable-references` over tracked text. Reasoned exceptions
live in `reference-exceptions.toml` beside this script.
"""

from __future__ import annotations

import argparse
import fnmatch
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

EXCEPTIONS = "scripts/reference-exceptions.toml"
MAX_TEXT_BYTES = 4 * 1024 * 1024
HOST_LOCAL = re.compile(
    r"/home/(?!example\b)[a-z_][\w-]*/|/Users/[A-Za-z]\w*/|/run/user/\d|/tmp/claude-|~/(?:\.local|\.cache|Projects)\b"
)
ROOTS = r"(?:benchmarks|contrib|crates|dev|docs|experiments|packages|proofs|scripts)"
REPOSITORY_PATH = re.compile(rf"(?<![\w./+-])/?({ROOTS}/[\w.+=-]+(?:/[\w.+=-]+)*)")
MARKDOWN_LINK = re.compile(r"\]\((?!https?:|mailto:|#|/)([^)#\s]+)")
TEMPLATE = re.compile(r"[<>{}*$]")
DEVICE = re.compile(r"dev/(?:null|zero|shm|stdin|stdout|stderr|fd|tty|kfd|dri|urandom|random|cpu|mem)\b")


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


def faults_in(path: str, text: str, known: set[str]) -> list[str]:
    faults = []
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
        if path.endswith(".md"):
            for link in MARKDOWN_LINK.findall(line):
                target = str(Path(path).parent / link)
                normal = str(Path(*_normal(target)))
                if not TEMPLATE.search(link) and normal not in known:
                    faults.append(f"{path}:{number}: {link}")
    return faults


def _normal(target: str) -> list[str]:
    parts: list[str] = []
    for part in target.split("/"):
        if part == "..":
            if parts:
                parts.pop()
        elif part not in ("", "."):
            parts.append(part)
    return parts or ["."]


def excepted(fault: str, exceptions: list[dict[str, str]]) -> bool:
    """An entry exempts its files; with `planned`, only paths of files to come."""
    path, target = fault.split(":", 1)[0], fault.rsplit(": ", 1)[1]
    for entry in exceptions:
        if fnmatch.fnmatch(path, entry["path"]):
            planned = entry.get("planned")
            if planned is None or target.startswith(tuple(planned)):
                return True
    return False


def ignored(root: Path, target: str) -> bool:
    check = subprocess.run(
        ["git", "-C", str(root), "check-ignore", "-q", "--no-index", target],
        capture_output=True,
    )
    return check.returncode == 0


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
            fault for fault in faults_in(path, text, known)
            if not excepted(fault, exceptions)
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
        (root / ".gitignore").write_text("dev/ignored/\n")
        (root / "docs/ignored.md").write_text("see dev/ignored/file\n")
        git(root, "add", ".")
        faults = check(root)
    assert any("bad-link.md" in f for f in faults), faults
    assert sum("host-local" in f for f in faults) == 1, faults
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
