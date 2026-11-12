#!/usr/bin/env python3
"""Refuses host-local paths and repository paths that do not resolve.

Enforces `@/inv/resolvable-references` over tracked text. Text is searched for
paths under the repository's top-level directories, relative and inline paths,
links, Cargo path dependencies and Rust includes. A JSON, TOML or YAML file is
also parsed: a string under a path-named member (`DATA_PATH_MEMBER`) shaped
like a relative path must resolve from the repository root or the file's
directory, be ignored by git, or be a template. A Markdown link or `../` path
resolves from its own file, a `./` path from its own file or the root, an
inline file name with directories from its own directory or one enclosing it,
and a bare file name by its path suffix. Host-local means a user's
home or a host mount; the toolchain's own home directories and device paths
such as `/dev/null` are neither host-local nor repository paths.

A file name of an evidence kind (`EVIDENCE_NAME`: a log, a captured stream,
a receipt, or a table, patch or text dump of a kind the tree once held), bare
or with directories, in prose, in an issue description, or in any data
string, must name a tracked file or be written at run time: a name under
`target/` passes when git ignores it, a bare name passes when its line or the
line before names a `target/` location, the statement that the run writes the
file there, and a name passes that tracked code (`WRITER_SUFFIXES`) writes
(`written_names`). In Markdown and issue prose the same holds for every
backticked bare file name (`BARE_INLINE`), whatever its kind, and so it does
in a JSON string under `dev/` or `experiments/` (`DATA_PROSE`), where a string
that is one file name of a known kind counts as backticked.

An invariant citation `@/inv/<id>` or `@/invariant/<id>` must name an
invariant of `.jit/invariants.toml`.

Every exception is an entry of `reference-exceptions.toml` beside this script,
with its reason; `path` is a glob or a list of globs. An entry with `planned`
excuses only paths that never existed: a path some commit deleted or moved
away is a removed path, not a planned one. An entry with `history = "closed"` excuses nothing by
itself; it lets a closed issue's prose name evidence and invariants the tree no
longer holds. The code holds two limits of its own. A file over
`MAX_TEXT_BYTES` (4 MiB) is not read, since no tracked text file approaches
it. A fault whose target git ignores is run output and passes, except a bare
evidence name, which has no location for the ignore rules to match.
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
    r"(?<![\w.-])/home/[a-z_][\w-]*/|/Users/[A-Za-z]\w*/|/run/user/\d|/tmp/claude-"
    r"|(?<![\w.-])/(?:mnt|root|data|scratch|var/tmp)/\w"
    # The toolchain's own directories exist wherever the toolchain does.
    r"|(?<![\w.])(?:~|\$HOME|\$\{HOME\})/(?!\.cargo\b|\.rustup\b|\.elan\b)[\w.]"
)
# A file under /tmp that prose cites is host-local evidence unless the same line
# is the command that writes it there.
TEMP_PATH = re.compile(r"(?<![\w.-])/tmp/([\w.+=-]+(?:/[\w.+=-]+)*)")
TEMP_WRITER = re.compile(r">|\s-o\s|--(?:output|out|receipt|checkpoint|log)\b|\btee\b|\bmktemp\b")
ROOTS = r"(?:\.agents|\.cargo|\.config|\.github|\.jit|benchmarks|contrib|crates|dev|docs|experiments|packages|proofs|scripts)"
REPOSITORY_PATH = re.compile(rf"(?<![\w./+~}}-])/?({ROOTS}/[\w.+=-]+(?:/[\w.+=-]+)*)")
RELATIVE_PATH = re.compile(r"(?<![\w./+-])((?:\.\./)+[\w.+=-]+(?:/[\w.+=-]+)*)")
INLINE_PATH = re.compile(r"`((?!10\.\d)[\w.+=-]+(?:/[\w.+=-]+)*/[\w+=-]+\.\w+)`")
MARKDOWN_LINK = re.compile(r"\]\((?!https?:|mailto:|#)([^)#\s]+)")
CARGO_PATH = re.compile(r"""\bpath\s*=\s*"([^"]+)\"""")
RUST_INCLUDE = re.compile(r"""include(?:_str|_bytes)?!\(\s*"([^"]+)"\s*\)""")
DOT_PATH = re.compile(r"(?<![\w./+-])\./([\w.+=-]+(?:/[\w.+=-]+)+)")
TEMPLATE = re.compile(r"[<>{}*$]")
DATA_SUFFIXES = (".json", ".toml", ".yaml", ".yml")
DATA_PATH_MEMBER = re.compile(
    r"(?i:(?:^|[_-])(?:path|dir|directory|directories|file|root|location)s?)$"
    r"|[a-z](?:Path|Dir|Directory|Directories|File|Root|Location)s?$"
)
DATA_PATH_VALUE = re.compile(r"(?:\.{1,2}/)*[\w.+=-]+(?:/[\w.+=-]+)*/?")
YAML_MEMBER = re.compile(r"""^\s*(?:-\s+)?([\w.-]+)\s*:\s*(?:(["']?)([^"'\s#]*)\2)\s*(?:#.*)?$""")
YAML_ITEM = re.compile(r"""^\s*-\s+(["']?)([^"'\s#]+)\1\s*(?:#.*)?$""")
FILE_TOKEN = re.compile(r"(?<![\w./+~$}}@:-])((?:[\w.+=-]+/)*[\w+=-][\w.+=-]*\.[A-Za-z]\w*)(?![\w/+=-])")
EVIDENCE_NAME = re.compile(r"\.(?:log|out|err|stdout|stderr|jsonl|csv|patch)$|receipt", re.IGNORECASE)
# Evidence kinds read only in backticks: prose names Markdown and text files
# of other repositories and of the author's host freely.
INLINE_EVIDENCE_NAME = re.compile(r"\.(?:md|txt)$", re.IGNORECASE)
FILE_EXTENSIONS = (
    "c|cfg|cpp|csv|css|dat|data|err|g|gp|gz|h|hip|html|ini|ipynb|js|json|jsonl|lean|lock|log|md"
    "|npy|out|patch|pdf|png|py|rs|sage|sh|sql|stderr|stdout|svg|tar|tex|toml|tsv|txt|xml|yaml|yml|zst"
)
BARE_INLINE = re.compile(rf"`([\w+=-][\w.+=-]*\.(?:{FILE_EXTENSIONS}))`")
# JSON strings under these roots carry prose and lists of file names.
DATA_PROSE = re.compile(r"(?:dev|experiments)/.*\.json$")
DATA_FILE_NAME = re.compile(rf"\.(?:{FILE_EXTENSIONS})$")
INVARIANT = re.compile(r"@/(?:inv|invariant)/([\w-]+)")
INVARIANTS = ".jit/invariants.toml"
TARGET_DIR = re.compile(r"(?<![\w.-])target/")
WRITER_SUFFIXES = (".rs", ".py", ".sh", ".c", ".cpp", ".h", ".hip")
# A string literal whose last path component is a file name.
DOUBLE_QUOTED = re.compile(r'"((?:[^"\\\n]|\\.)*)"')
SINGLE_QUOTED = re.compile(r"'([^'\\\n]*)'")
LITERAL_FILE = re.compile(r"(?:^|(?P<dir>.*)/)(?P<name>[\w+=-][\w.+=-]*\.[A-Za-z]\w*)$")
# The literal's directory is interpolated: `{dir}/name`, `$DIR/name`.
INTERPOLATED_DIR = re.compile(r"(?:\}|\$\w+|\$\{\w+\}|\))$")
# Text before a bare literal that joins it to a directory or opens it.
JOIN_BEFORE = re.compile(
    r"(?:\.join\(\s*&?|\bjoin\([^()]*,\s*|[\w)\]]\s*/\s*"
    r"|\b(?:open|File::create|fs::write|write_text|write_bytes)\(\s*&?)$"
)
NAME_CONSTANT = re.compile(r"\b([A-Z][A-Z0-9_]*)\s*(?::\s*&(?:'static\s+)?str\s*)?=\s*$")
DEVICE = re.compile(r"dev/(?:null|zero|shm|stdin|stdout|stderr|fd|tty|kfd|dri|urandom|random|cpu|mem)\b")


OPEN_STATES = {"backlog", "ready", "in_progress"}
# Code resolves `../` from its working directory at run time, so the text search
# reads it in prose only; `data_faults` reads it in path-named data members.
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


def evidence_faults(
    line: str,
    before: str,
    known: set[str],
    written: frozenset[str],
    markdown: bool = False,
    base: str = "",
    files: bool = False,
) -> list[str]:
    """Evidence-kind file names on `line` that name no tracked file.

    `written` holds the base names tracked code writes (`written_names`).
    `base` is the directory a `./` name resolves against. A name under `target/`
    is returned for the caller's ignore check. A bare name is excused when
    `line` or `before` names a `target/` location. `markdown` adds every
    backticked bare file name and the backticked-only evidence kinds. `files`
    reads `line` as a data string: the Markdown names, and the whole string
    when it is one file name of a known kind (`DATA_FILE_NAME`).
    """
    tokens = [(token, EVIDENCE_NAME) for token in FILE_TOKEN.findall(line)]
    if files and FILE_TOKEN.fullmatch(line.strip()):
        tokens.append((line.strip(), DATA_FILE_NAME))
    if markdown or files:
        tokens += [(token, None) for token in BARE_INLINE.findall(line)]
        tokens += [
            (token, INLINE_EVIDENCE_NAME)
            for token in INLINE_PATH.findall(line)
            if ".." not in token
        ]
    names = []
    for token, kind in tokens:
        name = token.removeprefix("./")
        # `RELATIVE_PATH` resolves a `../` name against its file.
        if ".." in name.split("/"):
            continue
        if kind is not None and not kind.search(Path(name).name):
            continue
        if _resolves(token, base, known) or Path(name).name in written:
            continue
        if "/" not in name and (TARGET_DIR.search(line) or TARGET_DIR.search(before)):
            continue
        if name not in names:
            names.append(name)
    return names


def faults_in(
    path: str,
    text: str,
    known: set[str],
    base: str | None = None,
    evidence: bool = True,
    written: frozenset[str] = frozenset(),
    invariants: frozenset[str] | None = None,
) -> list[str]:
    """`base` is the directory relative references resolve against.

    Passing it marks `text` as Markdown prose whatever the file's suffix.
    `evidence` false skips `evidence_faults` and the invariant check.
    `invariants` None skips the invariant check.
    """
    faults = []
    markdown = base is not None or path.endswith(".md")
    prose = markdown or not path.endswith(CODE_SUFFIXES)
    base = str(Path(path).parent) if base is None else base
    before = ""
    for number, line in enumerate(text.splitlines(), 1):
        if prose and evidence:
            faults += [
                f"{path}:{number}: {name}"
                for name in evidence_faults(line, before, known, written, markdown, base)
            ]
        if evidence and invariants is not None:
            faults += [
                f"{path}:{number}: @/inv/{name}"
                for name in INVARIANT.findall(line)
                if name not in invariants
            ]
        before = line
        if HOST_LOCAL.search(line):
            faults.append(f"{path}:{number}: host-local path")
        elif prose and not TEMP_WRITER.search(line) and any(
            EVIDENCE_NAME.search(m.group(1)) for m in TEMP_PATH.finditer(line)
        ):
            faults.append(f"{path}:{number}: host-local path")
        for match in REPOSITORY_PATH.finditer(line):
            end = match.end()
            if line[end : end + 1] in ("*", "{", "<", "$"):
                continue
            # A trailing slash names the directory itself.
            target = match.group(1).rstrip(".,:;").rstrip("/")
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
                if ".." not in inline and not _resolves(inline, base, known):
                    faults.append(f"{path}:{number}: {inline}")
            for link in MARKDOWN_LINK.findall(line):
                target = link[1:] if link.startswith("/") else f"{base}/{link}"
                normal = str(Path(*_normal(target)))
                if not TEMPLATE.search(link) and normal not in known:
                    faults.append(f"{path}:{number}: {link}")
    return faults


def data_faults(
    path: str,
    text: str,
    known: set[str],
    is_ignored=lambda target: False,
    written: frozenset[str] = frozenset(),
) -> list[str]:
    """Path-named members of a structured data file that resolve nowhere.

    Its reader may resolve a value from the repository root or from the file's
    directory, so either reading resolves it, and so does a reading git
    ignores. The fault names the file-relative reading of a `../` value and
    the root-relative reading of any other.
    """
    try:
        members = list(_data_members(path, text))
    except (ValueError, tomllib.TOMLDecodeError):
        return [f"{path}:0: unparseable {Path(path).suffix[1:]}"]
    base = str(Path(path).parent)
    lines = text.splitlines()
    faults = []
    files = bool(DATA_PROSE.match(path))
    for number, member, value in members:
        for name in evidence_faults(value, "", known, written, base=base, files=files):
            line = number or next((i for i, text in enumerate(lines, 1) if name in text), 0)
            faults.append(f"{path}:{line}: {name}")
        if not DATA_PATH_MEMBER.search(member) or "/" not in value:
            continue
        if not DATA_PATH_VALUE.fullmatch(value) or TEMPLATE.search(value):
            continue
        readings = ["/".join(_normal(value)), "/".join(_normal(f"{base}/{value}"))]
        if value.startswith("../"):
            readings.reverse()
        if any(reading in known or is_ignored(reading) for reading in readings):
            continue
        if number is None:
            number = next((i for i, line in enumerate(lines, 1) if value in line), 0)
        faults.append(f"{path}:{number}: {readings[0]}")
    return faults


def _data_members(path: str, text: str):
    """Yields `(line or None, member name, string value)` of a data file."""
    if path.endswith((".yaml", ".yml")):
        # Line-wise: a `name: value` member, or a list item under `name:`.
        owner = None
        for number, line in enumerate(text.splitlines(), 1):
            if member := YAML_MEMBER.match(line):
                owner = member.group(1) if not member.group(3) else None
                yield number, member.group(1), member.group(3)
            elif (item := YAML_ITEM.match(line)) and owner is not None:
                yield number, owner, item.group(2)
        return
    document = json.loads(text) if path.endswith(".json") else tomllib.loads(text)
    stack = [(None, document)]
    while stack:
        name, value = stack.pop()
        if isinstance(value, dict):
            stack.extend(reversed([(str(key), member) for key, member in value.items()]))
        elif isinstance(value, list):
            stack.extend(reversed([(name, member) for member in value]))
        elif isinstance(value, str) and name is not None:
            yield None, name, value


def issue_faults(
    path: str,
    text: str,
    known: set[str],
    written: frozenset[str] = frozenset(),
    invariants: frozenset[str] | None = None,
    history: bool = False,
) -> list[str]:
    """An issue's references resolve from the repository root; its links must exist.

    `history` skips the evidence-name and invariant checks of its prose.
    """
    issue = json.loads(text)
    prose = "\n".join(filter(None, [issue.get("description"), issue.get("context") if isinstance(issue.get("context"), str) else None]))
    faults = faults_in(
        path, prose, known, base="", evidence=not history, written=written, invariants=invariants
    )
    faults += [
        f"{path}:0: {document['path']}"
        for document in issue.get("documents", [])
        if document["path"] not in known
    ]
    return faults


def _evidence_kind(name: str) -> bool:
    return bool(EVIDENCE_NAME.search(name) or INLINE_EVIDENCE_NAME.search(name))


def _resolves(name: str, base: str, known: set[str]) -> bool:
    """Whether a file name of prose resolves.

    A `./` name resolves from `base` or the root. Another name with
    directories resolves from `base` or a directory enclosing it, as a crate's
    `src/lib.rs` or a skill's `references/x.md` does; a bare name resolves by
    path suffix.
    """
    if name.startswith("./"):
        name = name[2:]
        return name in known or "/".join(_normal(f"{base}/{name}")) in known
    if "/" in name:
        parts = _normal(base) if base else []
        return any(
            "/".join([*parts[:depth], *_normal(name)]) in known
            for depth in range(len(parts), -1, -1)
        )
    return name in known or any(path.endswith("/" + name) for path in known)


def written_names(texts: dict[str, str]) -> frozenset[str]:
    """Base names tracked code writes.

    A string literal of a writer source counts when it names a file under
    `target/`, interpolates its directory (`{dir}/name`, `$DIR/name`), is
    joined to a directory or opened, or is a constant whose identifier some
    writer source joins to a directory.
    """
    names: set[str] = set()
    constants: dict[str, set[str]] = {}
    sources = {path: text for path, text in texts.items() if path.endswith(WRITER_SUFFIXES)}
    for path, text in sources.items():
        quoted = (DOUBLE_QUOTED, SINGLE_QUOTED) if path.endswith((".py", ".sh")) else (DOUBLE_QUOTED,)
        for line in text.splitlines():
            for pattern in quoted:
                for literal in pattern.finditer(line):
                    file = LITERAL_FILE.match(literal.group(1))
                    if file is None:
                        continue
                    name, directory = file.group("name"), file.group("dir")
                    prefix = line[: literal.start()]
                    if TARGET_DIR.search(literal.group(1)):
                        names.add(name)
                    elif directory is not None:
                        if INTERPOLATED_DIR.search(directory):
                            names.add(name)
                    elif JOIN_BEFORE.search(prefix):
                        names.add(name)
                    elif constant := NAME_CONSTANT.search(prefix):
                        constants.setdefault(constant.group(1), set()).add(name)
    if constants:
        joined = re.compile(
            rf"(?:\.join\(\s*&?|[\w)\]]\s*/\s*|/\{{)({'|'.join(map(re.escape, constants))})\b"
        )
        for text in sources.values():
            for identifier in joined.findall(text):
                names |= constants[identifier]
    return frozenset(names)


def _normal(target: str) -> list[str]:
    parts: list[str] = []
    for part in target.split("/"):
        if part == "..":
            if parts:
                parts.pop()
        elif part not in ("", "."):
            parts.append(part)
    return parts or ["."]


def excepted(
    root: Path, fault: str, exceptions: list[dict[str, str]], removed: frozenset[str] = frozenset()
) -> bool:
    """An entry exempts its files; with `planned`, only paths of files to come.

    `removed` holds every path a commit deleted and its directories; such a
    path is not to come. A `history` entry exempts no fault (`history`).
    """
    path, target = fault.split(":", 1)[0], fault.rsplit(": ", 1)[1]
    for entry in exceptions:
        if "history" in entry:
            continue
        if _matches(path, entry):
            planned = entry.get("planned")
            if planned is None:
                return True
            if (
                target.startswith(tuple(planned))
                and target.rstrip("/") not in removed
                and _open_issue(root, path)
            ):
                return True
    return False


def _matches(path: str, entry: dict) -> bool:
    """`path` matches the entry's glob, or one of its list of globs."""
    globs = entry["path"]
    return any(fnmatch.fnmatch(path, glob) for glob in ([globs] if isinstance(globs, str) else globs))


def removed_paths(root: Path) -> frozenset[str]:
    """Paths any commit of any ref deleted or moved away, with their directories."""
    deleted = git(root, "log", "--all", "--no-renames", "--diff-filter=D", "--name-only", "--format=").split("\n")
    return frozenset(resolvable_set([path for path in deleted if path]))


def invariant_ids(root: Path) -> frozenset[str] | None:
    config = root / INVARIANTS
    if not config.exists():
        return None
    return frozenset(entry["id"] for entry in tomllib.loads(config.read_text())["invariants"])


def history(root: Path, path: str, exceptions: list[dict[str, str]]) -> bool:
    """Whether an entry with `history = "closed"` marks the issue at `path` as history.

    A closed issue's prose then may name evidence and invariants the
    repository no longer holds; its repository paths and links still resolve.
    """
    return any(
        entry.get("history") == "closed" and _matches(path, entry)
        for entry in exceptions
    ) and not _open_issue(root, path)


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


def scan(
    path: str,
    text: str,
    known: set[str],
    is_ignored=lambda target: False,
    written: frozenset[str] = frozenset(),
    invariants: frozenset[str] | None = None,
    is_history: bool = False,
) -> list[str]:
    if fnmatch.fnmatch(path, ".jit/issues/*.json"):
        return issue_faults(path, text, known, written, invariants, is_history)
    faults = faults_in(path, text, known, written=written, invariants=invariants)
    if path.endswith(DATA_SUFFIXES):
        faults += [
            fault for fault in data_faults(path, text, known, is_ignored, written) if fault not in faults
        ]
    return faults


def check(root: Path) -> list[str]:
    tracked = git(root, "ls-files", "-z").split("\0")[:-1]
    known = resolvable_set(tracked)
    config = root / EXCEPTIONS
    exceptions = (
        tomllib.loads(config.read_text())["exception"] if config.exists() else []
    )
    texts = {}
    for path in tracked:
        file = root / path
        if file.is_symlink() or not file.is_file():
            continue
        if file.stat().st_size > MAX_TEXT_BYTES:
            continue
        try:
            texts[path] = file.read_text()
        except UnicodeDecodeError:
            continue
    written = written_names(texts)
    invariants = invariant_ids(root)
    removed = removed_paths(root)
    faults = []
    for path, text in texts.items():
        found = scan(
            path,
            text,
            known,
            lambda target: ignored(root, target),
            written,
            invariants,
            fnmatch.fnmatch(path, ".jit/issues/*.json") and history(root, path, exceptions),
        )
        # A bare evidence name has no location, so git's ignore rules do not
        # excuse it; any other bare name git ignores is run output.
        faults += [
            fault for fault in dict.fromkeys(found)
            if not excepted(root, fault, exceptions, removed)
            and not (
                not (target := fault.rsplit(": ", 1)[1]).startswith("@/")
                and ("/" in target or not _evidence_kind(target))
                and ignored(root, target)
            )
        ]
    return faults


def self_test() -> None:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        git(root, "init", "-q")
        (root / "docs").mkdir()
        (root / "docs/good.md").write_text("[a](bad-link.md) docs/good.md\n")
        (root / "docs/host.md").write_text("see /home/alice/x\n")
        (root / "docs/temp.md").write_text(
            "results in /tmp/bench/run.csv\nscratch at /var/tmp/alice/run.json\n"
            "python3 x.py > /tmp/out.csv\nrun --receipt /tmp/osd/receipt.json\nclone into /tmp/charon\n"
        )
        (root / "docs/gone.md").write_text("see dev/gone/file.json\n")
        (root / "docs/relative.md").write_text("see ../missing/file.txt and ../docs/good.md\n")
        (root / "docs/inline.md").write_text("see `gone/file.json` and `docs/good.md`\n")
        (root / "docs/home.md").write_text("see $HOME/.local/share/x and crates/a/data/b and ../data/c\n")
        (root / "docs/tilde.md").write_text("see ~/vectors/x; ~/.cargo/bin is fine\n")
        (root / "docs/dot-root.md").write_text("see .jit/absent.toml; populate .jit/.\n")
        (root / "docs/listed-a.md").write_text("see dev/listed/a\n")
        (root / "docs/listed-b.md").write_text("see dev/listed/b\n")
        (root / "pkg").mkdir()
        (root / "pkg/Cargo.toml").write_text('a = { path = "../absent-crate" }\nb = { path = "../docs" }\n')
        (root / ".jit/issues").mkdir(parents=True)
        (root / ".jit/issues/a.json").write_text(json.dumps({
            "state": "done",
            "description": "first\ndev/gone/after-newline.md and [x](../issue-link.md) and `gone/issue.md`",
            "documents": [{"path": "dev/gone-document.md"}],
        }))
        (root / ".jit/archive-evidence/container").mkdir(parents=True)
        (root / ".jit/archive-evidence/container/c.json").write_text(json.dumps({
            "source": "dev/active/archived-away/plan.md",
        }))
        (root / ".jit/archive-evidence/unlisted.json").write_text(json.dumps({
            "source": "dev/active/unlisted-away/plan.md",
        }))
        (root / "docs/dot.md").write_text("run ./scripts/absent.sh\n")
        (root / "docs/rooted.md").write_text("[a](/docs/absent.md) [b](/docs/good.md)\n")
        (root / "src").mkdir()
        (root / "src/lib.rs").write_text('const A: &str = include_str!("../docs/absent.txt");\n')
        (root / ".gitignore").write_text("dev/ignored/\ntarget/\n")
        (root / "data").mkdir()
        (root / "data/fixture.json").write_text(json.dumps({"oracle": {
            "guava_path": "gap-packages/guava-3.21/", "output_dir": "target/oracle",
            "source_file": "docs/good.md", "note": "gap-packages/absent", "log_path": "target/<run>/x",
            "prompt_file": "./docs/good.md", "cacheDir": "absent/camel", "profile": "absent/feature",
        }}, indent=2))
        (root / "data/config.toml").write_text(
            'files = ["../docs/good.md", "absent/listed.txt"]\n[run]\nresults-dir = "dev/gone-results"\n'
        )
        (root / "data/workflow.yml").write_text(
            "steps:\n  - path: missing/cache\n    other: missing/value\n  - path: target/cache\npaths:\n  - 'crates/**'\n  - absent/listed-yaml\n"
        )
        (root / "docs/ignored.md").write_text("see dev/ignored/file\n")
        (root / "logs").mkdir()
        (root / "logs/kept.log").write_text("")
        (root / "docs/evidence.md").write_text(
            "stderr is `f5_wave_resource_usage.log`\n"
            "raw_log: hip/architecture_compile_evidence-v1.log\n"
            "the probe left gap.out and run.stdout behind\n"
            "see make-receipt.py and receipt-inventory.md\n"
            "kept: `kept.log` and logs/kept.log\n"
            "writes `target/run/out.log` and target/gone.log\n"
            "The run writes its receipt under `target/oracle/`:\n"
            "`oracle-receipt.md`, beside `oracle.log`.\n"
            "the tool writes `written.log`; see `{run}/x.log` and $OUT/y.log\n"
        )
        (root / "src/writer.py").write_text('open(out / "written.log", "w")\n')
        (root / "src/writer.rs").write_text(
            'const RUN_LOG: &str = "const-run.log";\nconst READ_LOG: &str = "const-read.log";\n'
            'fn f(dir: &Path) { write(dir.join(RUN_LOG)); read(dir.join("joined.log")); }\n'
            'let a = format!("{dir}/interpolated.log"); let b = "target/run/targeted.log";\n'
            'let c = ("tuple.log", 3); let d = "fixed/dir/fixed.log";\n'
        )
        (root / "docs/written.md").write_text(
            "`const-run.log`, `joined.log`, `interpolated.log`, `targeted.log`\n"
            "`const-read.log`, `tuple.log`, `fixed.log`\n"
        )
        (root / "docs/sub").mkdir()
        (root / "docs/sub/local.md").write_text("`./good.md`, `./scripts/kept.sh` and `./local.md`\n")
        (root / "data/evidence.json").write_text(json.dumps({"inputs": ["notes/kept-receipt.md", "gone-receipt.md"]}))
        (root / ".jit/issues/b.json").write_text(json.dumps({"state": "ready", "description": "see `open-gap.out`"}))
        (root / ".jit/issues/c.json").write_text(json.dumps({"state": "done", "description": "see `done-gap.out`"}))
        (root / ".jit/issues/f.json").write_text(json.dumps({"state": "done", "description": "see `unlisted.out`"}))
        (root / "notes").mkdir()
        (root / "notes/kept-receipt.md").write_text("")
        (root / ".jit/invariants.toml").write_text('[[invariants]]\nid = "kept-rule"\n')
        (root / "docs/invariants.md").write_text("`@/inv/kept-rule`, @/invariant/kept-rule, @/inv/gone-rule\n")
        (root / ".jit/issues/d.json").write_text(json.dumps({"state": "ready", "description": "@/invariant/gone-open"}))
        (root / ".jit/issues/e.json").write_text(json.dumps({"state": "done", "description": "@/inv/gone-closed"}))
        (root / ".gitignore").write_text("dev/ignored/\ntarget/\n*.profraw\n")
        (root / "docs/bare.md").write_text(
            "`absent-name.rs`, `good.md`, `self.id`, `run.profraw` and `notes/absent-note.txt`\n"
            "the probe left table.csv and fix.patch; plain absent-prose.md and absent.txt\n"
        )
        (root / "scripts").mkdir()
        (root / "scripts/kept.sh").write_text("")
        (root / "scripts/reference-exceptions.toml").write_text(
            '[[exception]]\npath = ".jit/issues/p*"\nplanned = ["crates/"]\nreason = "plans"\n'
            '[[exception]]\npath = ".jit/archive-evidence/container/*"\nreason = "history"\n'
            '[[exception]]\npath = ".jit/issues/[a-e].json"\nhistory = "closed"\nreason = "history"\n'
            '[[exception]]\npath = ["docs/listed-a.md", "docs/listed-b.md"]\nreason = "listed"\n'
        )
        (root / "crates").mkdir()
        (root / "crates/removed.rs").write_text("")
        (root / "crates/moved.rs").write_text("")
        (root / "docs/sub/deep.md").write_text("`sub/local.md` from an enclosing directory\n")
        (root / "notes/suffix.md").write_text("`sub/local.md` only by suffix\n")
        (root / "dev").mkdir()
        (root / "dev/plan.json").write_text(json.dumps({"touches": [
            "absent_module.rs", "kept.sh", "a `gone.rs` mention", "prose about lib.rs here",
        ]}))
        (root / "data/plan.json").write_text(json.dumps({"touches": ["absent_outside.rs"]}))
        git(root, "add", ".")
        git(root, "-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-qm", "a", "--no-verify")
        git(root, "rm", "-q", "crates/removed.rs")
        git(root, "mv", "crates/moved.rs", "crates/elsewhere.rs")
        (root / ".jit/issues/p.json").write_text(json.dumps({
            "state": "ready",
            "description": "creates crates/planned.rs, restores crates/removed.rs, edits crates/moved.rs",
        }))
        git(root, "add", ".")
        git(root, "-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-qm", "b", "--no-verify")
        faults = check(root)
    assert any("bad-link.md" in f for f in faults), faults
    assert sum("host-local" in f for f in faults) == 5, faults
    assert {f for f in faults if f.startswith("docs/temp.md")} == {
        "docs/temp.md:1: host-local path", "docs/temp.md:2: host-local path"}, faults
    assert sum("tilde.md" in f for f in faults) == 1, faults
    assert [f for f in faults if "dot-root.md" in f] == ["docs/dot-root.md:1: .jit/absent.toml"], faults
    assert not any(f.startswith("docs/listed-") for f in faults), faults
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
    archive = [f for f in faults if f.startswith(".jit/archive-evidence/")]
    assert archive == [".jit/archive-evidence/unlisted.json:1: dev/active/unlisted-away/plan.md"], archive
    unwritten = [f.split(": ", 1)[1] for f in faults if f.startswith("docs/written.md:")]
    assert unwritten == ["const-read.log", "tuple.log", "fixed.log"], unwritten
    local = [f.split(": ", 1)[1] for f in faults if f.startswith("docs/sub/local.md:")]
    assert local == ["good.md", "./good.md"], local
    data = [f for f in faults if f.startswith("data/")]
    for expected in (
        "data/fixture.json:3: gap-packages/guava-3.21",
        "data/fixture.json:9: absent/camel",
        "data/config.toml:1: absent/listed.txt",
        "data/config.toml:3: dev/gone-results",
        "data/workflow.yml:2: missing/cache",
        "data/workflow.yml:7: absent/listed-yaml",
    ):
        assert expected in data, (expected, data)
    assert len(data) == 7, data
    evidence = [f.split(":", 1)[1] for f in faults if f.startswith("docs/evidence.md:")]
    assert evidence == [
        "1: f5_wave_resource_usage.log",
        "2: hip/architecture_compile_evidence-v1.log",
        "3: gap.out",
        "3: run.stdout",
        "4: make-receipt.py",
        "4: receipt-inventory.md",
    ], evidence
    assert "data/evidence.json:1: gone-receipt.md" in faults, faults
    assert not any("kept-receipt" in f for f in faults), faults
    assert any(f.startswith(".jit/issues/b.json") and "open-gap.out" in f for f in faults), faults
    assert not any(f.startswith(".jit/issues/c.json") for f in faults), faults
    assert ".jit/issues/f.json:1: unlisted.out" in faults, faults
    assert not any("src/writer.py" in f for f in faults), faults
    assert [f for f in faults if "invariants.md" in f] == ["docs/invariants.md:1: @/inv/gone-rule"], faults
    assert ".jit/issues/d.json:1: @/inv/gone-open" in faults, faults
    assert not any(f.startswith(".jit/issues/e.json") for f in faults), faults
    bare = [f.split(": ", 1)[1] for f in faults if f.startswith("docs/bare.md:")]
    assert bare == ["absent-name.rs", "notes/absent-note.txt", "table.csv", "fix.patch"], bare
    assert ".jit/issues/p.json:1: crates/removed.rs" in faults, faults
    assert not any("crates/planned.rs" in f for f in faults), faults
    assert ".jit/issues/p.json:1: crates/moved.rs" in faults, faults
    assert "notes/suffix.md:1: sub/local.md" in faults, faults
    assert not any(f.startswith("docs/sub/deep.md") for f in faults), faults
    assert [f for f in faults if f.startswith("dev/plan.json")] == [
        "dev/plan.json:1: absent_module.rs", "dev/plan.json:1: gone.rs",
    ], faults
    assert not any("absent_outside.rs" in f for f in faults), faults


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
