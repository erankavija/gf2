#!/usr/bin/env python3
"""Refuses host-local paths and repository paths that do not resolve.

Enforces `@/inv/resolvable-references` over tracked text. Text is searched for
paths under the repository's top-level directories, relative and inline paths,
links, Cargo path dependencies and Rust includes. A JSON, TOML or YAML file is
also parsed: a string under a path-named member (`DATA_PATH_MEMBER`) shaped
like a relative path must resolve from the repository root or the file's
directory, be ignored by git, or be a template. A Markdown link or `../` path
resolves from its own file, a `./` path from its own file or the root, an
inline file name (bare or with directories) from its own directory or one
enclosing it. Issue prose has no directory: bare names need repository paths.
Every `/tmp` path in prose is host-local unless its own line writes or locks
that path. Host-local also means a user's
home or a host mount; the toolchain's own home directories and device paths
such as `/dev/null` are neither host-local nor repository paths.

A file name of an evidence kind (`EVIDENCE_NAME`: a log, a captured stream,
a receipt, or a table, patch or text dump of a kind the tree once held), bare
or with directories, in prose, in an issue description, or in any data
string, must name a tracked file or be written at run time: a name under
`target/` passes when git ignores it, a bare name passes when its line or the
line before names a `target/` location, the statement that the run writes the
file there, and a name passes that tracked code (`WRITER_SUFFIXES`) writes
(`written_names`): the name must reach a write-mode open, file create/write,
shell redirect, `tee` or `-o` in the same function or script, directly or through
a local binding or named constant. Read-only joins excuse nothing.
Calls to the named `writer_helpers` in the checker configuration count as
writes of their configured zero-based path argument by the same local rule,
without cross-function flow. Each listed helper
must have a function definition in tracked code; a missing definition is a
checker fault. Writers outside this rule need a reasoned exception.
In Markdown and issue prose the same holds for every
backticked bare file name (`BARE_INLINE`), whatever its kind, and so it does
in a JSON string under `dev/` or `experiments/` (`DATA_PROSE`), where a string
that is one file name of a known kind counts as backticked.
A bare file name used as a local Markdown link's label, with or without
backticks, resolves through that target when their base names match. A
mismatched file-name label faults; the target is checked independently.

An invariant citation `@/inv/<id>` or `@/invariant/<id>` must name an
identifier of the projection between the `jit:invariants` markers of
`AGENTS.md`, with or without the tracker store.

A tracker reference is a path under `.jit/` named in text. The data root is
`JIT_DATA_DIR` when set, otherwise `.jit` under the repository root, and the
store is mounted when the data root holds `index.json`. The files of the store
are the tracked and the untracked unignored files of the git repository that
contains the data root, whether that is the code repository or the store's own.
With the store mounted a tracker reference must name one of them or a path the
ignore rules of the store's repository cover; the code repository's ignore
rules excuse no tracker reference. Without a store, as on a fresh clone or a
CI runner, tracker references pass and the checker prints how many it left
unchecked; every other check runs.

When the data root is `.jit` under the repository root, the files of the store
are scanned as `.jit/` paths, so issue records and archive records are checked
under their exception entries whichever repository tracks them. When the data
root lies elsewhere, as for a linked worktree sharing the primary checkout's
store, that prose is written against other branches: issue records and archive
records are skipped with a printed count. In an issue record a document link
pinned to a commit resolves in that commit's tree; an unpinned link, a
document asset and a path of the prose resolve in the checkout or in the tree
of a local branch head, which all checkouts of the host share.

Every exception is an entry of `reference-exceptions.toml` beside this script,
with its reason; `path` is a glob or a list of globs. An entry with `planned`
excuses only paths that never existed: a path some commit deleted or moved
away is a removed path, not a planned one. An entry with `history = "closed"` excuses nothing by
itself; it lets a closed issue's prose name evidence and invariants the tree no
longer holds. The code holds two limits of its own. A file over
`MAX_TEXT_BYTES` (4 MiB) is not read, since no tracked text file approaches
it. A fault whose target git ignores is run output and passes, except a bare
evidence name, which has no location for the ignore rules to match; a tracker
reference answers to the ignore rules of the store's repository.
"""

from __future__ import annotations

import argparse
import ast
import fnmatch
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

EXCEPTIONS = "scripts/reference-exceptions.toml"
MAX_TEXT_BYTES = 4 * 1024 * 1024
HOST_LOCAL = re.compile(
    r"(?<![\w.*?\]}>.-])(?:/home/[a-z_][\w-]*/|/Users/[A-Za-z]\w*/|/run/user/\d"
    r"|/(?:mnt|root|data|scratch|var/tmp)/\w)"
    # The toolchain's own directories exist wherever the toolchain does.
    r"|(?<![\w.])(?:~|\$HOME|\$\{HOME\})/(?!\.cargo\b|\.rustup\b|\.elan\b)[\w.]"
)
# Every /tmp citation needs a writer or lock on its own line.
TEMP_PATH = re.compile(r"(?<![\w.-])/tmp(?![\w.-])(?:/[\w.+=-]+(?:/[\w.+=-]+)*)?/?")
TEMP_WRITER = re.compile(
    r"(?:>>?|\s-o\s+|--(?:output|out|receipt|checkpoint|log)[=\s]+"
    r"|\b(?:tee|mktemp|flock|mkdir|touch)\s+(?:-[\w]+\s+)*)\s*[`\"']?$"
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
MARKDOWN_FILE_LINK = re.compile(
    rf"\[(`?)([\w+=-][\w.+=-]*\.(?:{FILE_EXTENSIONS}))\1\]"
    r"\((?!https?:|mailto:|#)([^)#\s]+)"
)
# JSON strings under these roots carry prose and lists of file names.
DATA_PROSE = re.compile(r"(?:dev|experiments)/.*\.json$")
DATA_FILE_NAME = re.compile(rf"\.(?:{FILE_EXTENSIONS})$")
INVARIANT = re.compile(r"@/(?:inv|invariant)/([\w-]+)")
INVARIANT_PROJECTION = "AGENTS.md"
PROJECTION = re.compile(r"<!-- jit:invariants:begin -->(.*?)<!-- jit:invariants:end -->", re.DOTALL)
PROJECTED_ID = re.compile(r"^- \*\*([\w-]+)\*\*", re.MULTILINE)
STORE = ".jit"
STORE_INDEX = "index.json"
# Issue records and archive records: the prose a shared store holds for every branch.
STORE_RECORD = re.compile(r"\.jit/(?:issues/[^/]+\.json|archive-evidence/.+)$")
TARGET_DIR = re.compile(r"(?<![\w.-])target/")
WRITER_SUFFIXES = (".rs", ".py", ".sh", ".c", ".cpp", ".h", ".hip")
# A string literal whose last path component is a file name.
LITERAL_FILE = re.compile(r"(?:^|(?P<dir>.*)/)(?P<name>[\w+=-][\w.+=-]*\.[A-Za-z]\w*)$")
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


def _temp_written(line: str, match: re.Match) -> bool:
    before, after = line[:match.start()], line[match.end():]
    return bool(
        TEMP_WRITER.search(before)
        or re.search(r'''\b(?:File::create|fs::write)\(\s*&?["']$''', before)
        or (
            re.search(r'''\bopen\(\s*["']$''', before)
            and re.match(r'''["']\s*,\s*(?:mode\s*=\s*)?["'][wax+][^"']*["']''', after)
        )
        or (
            re.search(r'''\bPath\(\s*["']$''', before)
            and re.match(r'''["']\)\.(?:write_text|write_bytes|touch)\(''', after)
        )
    )


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
    names = []
    if markdown:
        def label(match: re.Match) -> str:
            name, target = match.group(2, 3)
            if name != Path(target).name and name not in names:
                names.append(name)
            return match.group().replace(name, " " * len(name), 1)

        # Only the label is covered by the link; a separate citation of the
        # same name on this line still needs its own resolution.
        line = MARKDOWN_FILE_LINK.sub(label, line)
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
    prose = markdown or not (
        path.endswith(CODE_SUFFIXES)
        or Path(path).name in {"Dockerfile", "Containerfile", ".gitignore"}
    )
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
        elif prose and any(
            not _temp_written(line, m) for m in TEMP_PATH.finditer(line)
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
    trees: Trees | None = None,
) -> list[str]:
    """An issue's references resolve from the repository root; its links must exist.

    A link is a document entry's `path` and the `resolved_path` of each of its
    non-external assets; git-ignored build output such as `target/doc` passes.
    With `trees`, a link pinned to a commit resolves in that commit's tree, and
    an unpinned link or a path of the prose also in a local branch head's tree.

    `history` skips the evidence-name and invariant checks of its prose.
    """
    issue = json.loads(text)
    prose = "\n".join(filter(None, [issue.get("description"), issue.get("context") if isinstance(issue.get("context"), str) else None]))
    faults = [
        fault
        for fault in faults_in(
            path, prose, known, base="", evidence=not history, written=written, invariants=invariants
        )
        if trees is None
        or (target := fault.rsplit(": ", 1)[1]).startswith("@/")
        or target == "host-local path"
        or not trees.on_branch(target)
    ]

    def resolves(target: str, commit: str | None) -> bool:
        if trees is None:
            return target in known
        return target in known or trees.on_branch(target) or bool(commit and trees.holds([commit], target))

    for document in issue.get("documents", []):
        commit = document.get("commit")
        if trees is not None and commit:
            linked = trees.holds([commit], document["path"])
        else:
            linked = resolves(document["path"], None)
        if not linked:
            faults.append(f"{path}:0: {document['path']}")
    # Every local link a document entry carries: its scanned assets, local,
    # directory or missing; an external asset is a URL.
    faults += [
        f"{path}:0: {target}"
        for document in issue.get("documents", [])
        for asset in document.get("assets", [])
        if asset.get("asset_type") != "external"
        and (target := str(asset.get("resolved_path", "")).rstrip("/"))
        and not TEMPLATE.search(target)
        and not resolves(target, document.get("commit"))
    ]
    return faults


class Trees:
    """Path lookups in commit trees of the git objects every checkout shares."""

    def __init__(self, root: Path):
        self.root = root
        self._heads: list[str] | None = None
        self._branch: dict[str, bool] = {}

    def holds(self, revisions: list[str], target: str) -> bool:
        """Whether the tree of any of `revisions` holds `target`."""
        target = "/".join(_normal(target))
        if not revisions or target == ".":
            return False
        lines = subprocess.run(
            ["git", "-C", str(self.root), "cat-file", "--batch-check"],
            input="".join(f"{revision}:{target}\n" for revision in revisions),
            check=True, capture_output=True, text=True,
        ).stdout.splitlines()
        # A present object prints its size last; a missing one `missing`.
        return any(line.rsplit(" ", 1)[-1].isdigit() for line in lines)

    def on_branch(self, target: str) -> bool:
        if self._heads is None:
            refs = git(self.root, "for-each-ref", "--format=%(objectname)", "refs/heads")
            self._heads = sorted(set(refs.split()))
        if target not in self._branch:
            self._branch[target] = self.holds(self._heads, target)
        return self._branch[target]


def _evidence_kind(name: str) -> bool:
    return bool(EVIDENCE_NAME.search(name) or INLINE_EVIDENCE_NAME.search(name))


def _resolves(name: str, base: str, known: set[str]) -> bool:
    """Whether a file name of prose resolves.

    A `./` name resolves from `base` or the root. Another name with
    directories resolves from `base` or a directory enclosing it, as a crate's
    `src/lib.rs` or a skill's `references/x.md` does. Bare names use those
    same directories; issue prose uses only the root.
    """
    if name.startswith("./"):
        name = name[2:]
        return name in known or "/".join(_normal(f"{base}/{name}")) in known
    parts = _normal(base) if base else []
    return any(
        "/".join([*parts[:depth], *_normal(name)]) in known
        for depth in range(len(parts), -1, -1)
    )


def _literal_names(value: str) -> set[str]:
    file = LITERAL_FILE.search(value)
    return {file['name']} if file else set()


def _python_written(text: str, helpers: dict[str, int]) -> set[str]:
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return set()
    names = set()

    def expression(node, bindings):
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return _literal_names(node.value)
        if isinstance(node, ast.Name):
            return bindings.get(node.id, set())
        if isinstance(node, ast.JoinedStr):
            return expression(node.values[-1], bindings) if node.values else set()
        if isinstance(node, ast.BinOp):
            return expression(node.right, bindings)
        if isinstance(node, ast.Call):
            # Only path constructors and joins, never arbitrary helpers.
            if isinstance(node.func, ast.Name) and node.func.id in {'Path', 'str'}:
                return expression(node.args[-1], bindings) if node.args else set()
            if isinstance(node.func, ast.Attribute) and node.func.attr in {'join', 'joinpath'}:
                return expression(node.args[-1], bindings) if node.args else set()
        return set()

    constants = {}
    for node in tree.body:
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                if isinstance(target, ast.Name) and target.id.isupper():
                    constants[target.id] = expression(node.value, constants)

    class Writes(ast.NodeVisitor):
        def __init__(self):
            self.bindings = dict(constants)

        def visit_FunctionDef(self, node):
            visitor = Writes()
            for statement in node.body:
                visitor.visit(statement)

        visit_AsyncFunctionDef = visit_FunctionDef

        def visit_Assign(self, node):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    self.bindings[target.id] = expression(node.value, self.bindings)
            self.generic_visit(node)

        def visit_AnnAssign(self, node):
            if isinstance(node.target, ast.Name):
                self.bindings[node.target.id] = expression(node.value, self.bindings)
            self.generic_visit(node)

        def visit_Call(self, node):
            function = node.func
            name = function.id if isinstance(function, ast.Name) else (
                function.attr if isinstance(function, ast.Attribute) else ''
            )
            if name in helpers and len(node.args) > helpers[name]:
                names.update(expression(node.args[helpers[name]], self.bindings))
            opened = isinstance(function, ast.Name) and function.id == 'open'
            method = function.attr if isinstance(function, ast.Attribute) else ''
            if opened or method == 'open':
                offset = 1 if opened else 0
                mode = next((kw.value for kw in node.keywords if kw.arg == 'mode'),
                            node.args[offset] if len(node.args) > offset else None)
                if isinstance(mode, ast.Constant) and isinstance(mode.value, str) and any(
                    flag in mode.value for flag in 'wax+'
                ):
                    if opened and node.args:
                        names.update(expression(node.args[0], self.bindings))
                    elif not opened:
                        names.update(expression(function.value, self.bindings))
            elif method in {'write_text', 'write_bytes', 'touch'}:
                names.update(expression(function.value, self.bindings))
            self.generic_visit(node)

    Writes().visit(tree)
    return names


# Tokenize strings and comments before looking for balanced calls and functions.
SOURCE_TOKEN = re.compile(
    r'//[^\n]*|/\*.*?\*/|r(?P<hashes>\#*)".*?"(?P=hashes)|"(?:[^"\\]|\\.)*"'
    r"|'(?:[^'\\\n]|\\.)'|[A-Za-z_]\w*|::|[^\s]", re.DOTALL
)


def _matching(tokens, start):
    pairs = {'(': ')', '[': ']', '{': '}'}
    stack = []
    for index in range(start, len(tokens)):
        token = tokens[index]
        if token in pairs:
            stack.append(pairs[token])
        elif stack and token == stack[-1]:
            stack.pop()
            if not stack:
                return index
    return len(tokens) - 1


def _native_scopes(tokens):
    """Function bodies, with nested functions removed from their parent scope."""
    scopes = []
    outside = []
    i = 0
    while i < len(tokens):
        # Rust fn, or a C-style function declarator followed by a body.
        start = i if tokens[i] == 'fn' else None
        if (tokens[i] == '(' and i and re.fullmatch(r'\w+', tokens[i - 1])
                and tokens[i - 1] not in {'if', 'for', 'while', 'switch', 'catch'}):
            closing = _matching(tokens, i)
            if closing + 1 < len(tokens) and tokens[closing + 1] == '{':
                start = i
        if start is not None:
            opening = next((j for j in range(i, len(tokens)) if tokens[j] in {'{', ';'}), None)
            if opening is not None and tokens[opening] == '{':
                end = _matching(tokens, opening)
                scopes.extend(_native_scopes(tokens[opening + 1:end]))
                i = end + 1
                continue
        outside.append(tokens[i])
        i += 1
    return scopes + [outside]


def _native_constants(tokens):
    constants = {}
    for i, token in enumerate(tokens):
        if token != 'const' or i + 1 == len(tokens):
            continue
        end = next((j for j in range(i, len(tokens)) if tokens[j] == ';'), len(tokens))
        if '=' not in tokens[i:end]:
            continue
        value = tokens[tokens.index('=', i, end) + 1:end]
        if len(value) == 1 and re.fullmatch(r'r?\#*".*"\#*', value[0], re.DOTALL):
            constants[tokens[i + 1]] = _literal_names(re.sub(r'^r?\#*"|"\#*$', '', value[0]))
    return constants


def _native_written(
    text: str, constants: dict[str, set[str]], helpers: dict[str, int]
) -> set[str]:
    tokens = [m[0] for m in SOURCE_TOKEN.finditer(text) if not m[0].startswith(('//', '/*'))]
    names = set()

    def expression(parts, bindings):
        found = set()
        i = 0
        while i < len(parts):
            part = parts[i]
            if part == '(' and i:
                callee = parts[i - 1]
                if callee == '!' and i > 1:
                    callee = parts[i - 2]
                if re.fullmatch(r'\w+', callee) and callee not in {
                    'join', 'from', 'new', 'format', 'to_path_buf', 'as_path', 'clone',
                }:
                    i = _matching(parts, i) + 1
                    continue
            if part.startswith(('"', 'r"', 'r#"')):
                value = re.sub(r'^r?\#*"|"\#*$', '', part)
                found.update(_literal_names(value))
            elif part in bindings:
                found.update(bindings[part])
            i += 1
        return found

    scopes = _native_scopes(tokens)
    constants = {**constants, **_native_constants(scopes[-1])}
    for scope in scopes:
        bindings = {**constants, **_native_constants(scope)}
        for i, token in enumerate(scope):
            if token == '=' and i and re.fullmatch(r'\w+', scope[i - 1]):
                end = next((j for j in range(i, len(scope)) if scope[j] == ';'), len(scope))
                bindings[scope[i - 1]] = expression(scope[i + 1:end], bindings)
            if token != '(' or not i:
                continue
            end = _matching(scope, i)
            # The first argument ends at its first top-level comma.
            arg_end = i + 1
            while arg_end < end:
                if scope[arg_end] == ',':
                    break
                arg_end = (_matching(scope, arg_end) + 1
                           if scope[arg_end] in {'(', '[', '{'} else arg_end + 1)
            callee = ''.join(scope[max(0, i - 4):i])
            receiver = ''
            if scope[i - 1] == 'open':
                statement = next((j + 1 for j in range(i - 1, -1, -1) if scope[j] == ';'), 0)
                receiver = ''.join(scope[statement:i])
            write_open = (
                receiver.endswith('.open')
                and re.search(r'(?:OpenOptions::new|File::options)\(\)', receiver)
                and re.search(r'\.(?:write|append|create|create_new|truncate)\(true\)', receiver)
            )
            if (re.search(r'(?<!\w)(?:File::create|fs::write|fopen)$', callee)
                    or write_open or scope[i - 1] in helpers):
                if not callee.endswith('fopen') or any(
                    re.fullmatch(r'"(?:[wax][^"\n]*|r[^"\n]*\+[^"\n]*)"', part)
                    for part in scope[arg_end + 1:end]
                ):
                    argument = i + 1
                    for _ in range(helpers.get(scope[i - 1], 0)):
                        argument = arg_end + 1
                        arg_end = argument
                        while arg_end < end and scope[arg_end] != ',':
                            arg_end = (_matching(scope, arg_end) + 1
                                       if scope[arg_end] in {'(', '[', '{'} else arg_end + 1)
                    names.update(expression(scope[argument:arg_end], bindings))
    return names


def _shell_written(text: str) -> set[str]:
    names = set()
    bindings = {}
    parents = []
    for line in text.splitlines():
        if re.match(r'\s*(?:function\s+\w+(?:\(\))?|\w+\s*\(\s*\))\s*\{', line):
            parents.append(bindings)
            bindings = {key: value for key, value in bindings.items() if key.isupper()}
        try:
            lexer = shlex.shlex(line, posix=True, punctuation_chars='><|;&')
            lexer.whitespace_split = True
            tokens = list(lexer)
        except ValueError:
            continue
        for token in tokens:
            if match := re.fullmatch(r'(\w+)=(.*)', token):
                bindings[match[1]] = _literal_names(match[2])
        for i, token in enumerate(tokens[:-1]):
            if token not in {'>', '>>', '-o', 'tee'}:
                continue
            if token == 'tee' and i and tokens[i - 1] not in {'|', ';', '&&', '||'}:
                continue
            output = i + 1
            while token == 'tee' and output < len(tokens) and tokens[output].startswith('-'):
                output += 1
            if output == len(tokens):
                continue
            value = tokens[output]
            names.update(_literal_names(value))
            variable = re.fullmatch(r'\$\{?(\w+)\}?', value)
            if variable:
                names.update(bindings.get(variable[1], set()))
        if parents and re.search(r'(?:^|;)\s*}\s*;?\s*$', line):
            bindings = parents.pop()
    return names


def writer_helper_faults(texts: dict[str, str], helpers: dict[str, int]) -> list[str]:
    """Configured helpers must still have a tracked function definition."""
    defined = set()
    for path, text in texts.items():
        if path.endswith('.py'):
            try:
                tree = ast.parse(text)
            except SyntaxError:
                continue
            defined.update(node.name for node in ast.walk(tree)
                           if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)))
        elif path.endswith('.sh'):
            defined.update(re.findall(
                r'^\s*(?:function\s+)?(\w+)\s*\(\s*\)\s*\{', text, re.MULTILINE
            ))
        elif path.endswith(WRITER_SUFFIXES):
            tokens = [m[0] for m in SOURCE_TOKEN.finditer(text)
                      if not m[0].startswith(('//', '/*'))]
            for i, token in enumerate(tokens):
                if token == 'fn' and i + 1 < len(tokens):
                    body = next((part for part in tokens[i + 2:] if part in {'{', ';'}), None)
                    if body == '{':
                        defined.add(tokens[i + 1])
                elif (path.endswith(('.c', '.cpp', '.h', '.hip')) and token == '('
                      and i and re.fullmatch(r'\w+', tokens[i - 1])):
                    end = _matching(tokens, i)
                    if end + 1 < len(tokens) and tokens[end + 1] == '{':
                        defined.add(tokens[i - 1])
    return [f'{EXCEPTIONS}:1: writer helper {name} has no function definition'
            for name in sorted(helpers.keys() - defined)]


def written_names(
    texts: dict[str, str], helpers: dict[str, int] | None = None
) -> frozenset[str]:
    """Names reaching a syntactic write in one function or script.

    Only named constants enter function scopes. Python uses its AST;
    native sources use balanced tokens; shell scripts use redirects, tee and
    -o. Listed writer helpers count as writes of their configured path argument.
    """
    helpers = helpers or {}
    names = set()
    candidates: dict[str, set[str]] = {}
    for path, text in texts.items():
        if path.endswith(WRITER_SUFFIXES) and not path.endswith(('.py', '.sh')):
            tokens = [m[0] for m in SOURCE_TOKEN.finditer(text) if not m[0].startswith(('//', '/*'))]
            for name, values in _native_constants(_native_scopes(tokens)[-1]).items():
                candidates.setdefault(name, set()).update(values)
    constants = {name: values for name, values in candidates.items() if len(values) == 1}
    for path, text in texts.items():
        if path.endswith('.py'):
            names.update(_python_written(text, helpers))
        elif path.endswith('.sh'):
            names.update(_shell_written(text))
        elif path.endswith(WRITER_SUFFIXES):
            names.update(_native_written(text, constants, helpers))
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


def invariant_ids(root: Path) -> frozenset[str]:
    """Identifiers of the invariant projection between the `jit:invariants` markers."""
    file = root / INVARIANT_PROJECTION
    text = file.read_text() if file.is_file() else ""
    return frozenset(name for block in PROJECTION.findall(text) for name in PROJECTED_ID.findall(block))


def data_root(root: Path, environ=os.environ) -> Path:
    """`JIT_DATA_DIR` when set, otherwise `.jit` under the repository root."""
    value = environ.get("JIT_DATA_DIR")
    return Path(value).absolute() if value else root / STORE


def _tracker(target: str) -> bool:
    return target == STORE or target.startswith(STORE + "/")


class Store:
    """The files of the store at a data root and its repository's ignore rules.

    Its files are the tracked and the untracked unignored files, under the data
    root, of the git repository that contains it; outside any repository, every
    file under the data root.
    """

    def __init__(self, data: Path):
        self.mounted = (data / STORE_INDEX).is_file()
        self.top: Path | None = None
        self.prefix = ""
        self.files: list[str] = []
        if not self.mounted:
            return
        try:
            self.top = Path(git(data, "rev-parse", "--show-toplevel").strip())
        except subprocess.CalledProcessError:
            self.files = sorted(str(p.relative_to(data)) for p in data.rglob("*") if p.is_file())
            return
        prefix = os.path.relpath(data.resolve(), self.top.resolve())
        self.prefix = "" if prefix == "." else prefix + "/"
        listed = git(
            self.top, "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", prefix
        ).split("\0")[:-1]
        self.files = [p[len(self.prefix):] for p in dict.fromkeys(listed) if p.startswith(self.prefix)]

    def ignored(self, target: str) -> bool:
        """Whether the store's repository ignores the tracker reference `target`."""
        return self.top is not None and target != STORE and ignored(
            self.top, self.prefix + target[len(STORE) + 1:]
        )


class Unchecked(set):
    """Known paths with every tracker reference passing; `tracker` records them."""

    def __init__(self, known: set[str]):
        super().__init__(known)
        self.tracker: set[str] = set()

    def __contains__(self, target) -> bool:
        if isinstance(target, str) and _tracker(target):
            self.tracker.add(target)
            return True
        return super().__contains__(target)


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
    trees: Trees | None = None,
) -> list[str]:
    if fnmatch.fnmatch(path, ".jit/issues/*.json"):
        return issue_faults(path, text, known, written, invariants, is_history, trees)
    faults = faults_in(path, text, known, written=written, invariants=invariants)
    if path.endswith(DATA_SUFFIXES):
        faults += [
            fault for fault in data_faults(path, text, known, is_ignored, written) if fault not in faults
        ]
    return faults


def check(root: Path, data: Path | None = None, notes: list[str] | None = None) -> list[str]:
    """Faults of the checkout at `root` with the store at the data root `data`.

    `notes` receives the counts of what the checker left unchecked.
    """
    data = root / STORE if data is None else data
    notes = [] if notes is None else notes
    store = Store(data)
    local = data.absolute() == (root / STORE).absolute()
    tracked = git(root, "ls-files", "-z").split("\0")[:-1]
    files = {path: root / path for path in tracked}
    if store.mounted:
        known = resolvable_set([p for p in tracked if not _tracker(p)] + [f"{STORE}/{p}" for p in store.files])
        known.add(STORE)
    else:
        known = Unchecked(resolvable_set(tracked))
    skipped = set()
    if local and store.mounted:
        files = {path: file for path, file in files.items() if not _tracker(path)}
        files.update({f"{STORE}/{path}": data / path for path in store.files})
    elif not local:
        skipped = {path for path in [*files, *(f"{STORE}/{p}" for p in store.files)] if STORE_RECORD.match(path)}
        files = {path: file for path, file in files.items() if path not in skipped}
    config = root / EXCEPTIONS
    configuration = tomllib.loads(config.read_text()) if config.exists() else {}
    exceptions = configuration.get('exception', [])
    helpers = configuration.get('writer_helpers', {})
    texts = {}
    for path, file in files.items():
        if file.is_symlink() or not file.is_file():
            continue
        if file.stat().st_size > MAX_TEXT_BYTES:
            continue
        try:
            texts[path] = file.read_text()
        except UnicodeDecodeError:
            continue
    written = written_names(texts, helpers)
    invariants = invariant_ids(root)
    removed = removed_paths(root)
    trees = Trees(root)
    faults = writer_helper_faults(texts, helpers)

    def is_ignored(target: str) -> bool:
        # The code repository's rules never excuse a tracker reference.
        return store.ignored(target) if _tracker(target) else ignored(root, target)
    for path, text in texts.items():
        found = scan(
            path,
            text,
            known,
            is_ignored,
            written,
            invariants,
            fnmatch.fnmatch(path, ".jit/issues/*.json") and history(root, path, exceptions),
            trees,
        )
        # A bare evidence name has no location, so git's ignore rules do not
        # excuse it; any other bare name git ignores is run output.
        faults += [
            fault for fault in dict.fromkeys(found)
            if not excepted(root, fault, exceptions, removed)
            and not (
                not (target := fault.rsplit(": ", 1)[1]).startswith("@/")
                and ("/" in target or not _evidence_kind(target))
                and is_ignored(target)
            )
        ]
    if not store.mounted:
        notes.append(
            f"check-references: {len(known.tracker)} tracker reference(s) left unchecked: "
            "no store index at the data root"
        )
    if skipped:
        notes.append(
            f"check-references: skipped the prose of {len(skipped)} issue and archive record(s): "
            "the data root lies outside the checkout"
        )
    return faults


def self_test() -> None:
    helpers = {'publish_atomic': 0}
    sources = {'src/helpers.rs': (
        'fn publish_atomic(path: &Path) {}\n'
        'fn write(dir: &Path) { let path = dir.join("listed.log"); '
        'publish_atomic(&path); unlisted(dir.join("unlisted.log")); }'
    )}
    assert written_names(sources, helpers) == {'listed.log'}
    assert faults_in('docs/helpers.md', '`listed.log`\n`unlisted.log`', set(),
                     written=written_names(sources, helpers)) == [
        'docs/helpers.md:2: unlisted.log'
    ]
    assert not writer_helper_faults(sources, helpers)
    assert writer_helper_faults(sources, {'missing': 0}) == [
        f'{EXCEPTIONS}:1: writer helper missing has no function definition'
    ]
    assert writer_helper_faults({'src/declaration.rs': 'fn missing(path: &Path);'},
                                {'missing': 0})
    assert written_names({'src/helpers.rs': (
        'fn write(dir: &Path) { write_csv("payload.log", dir.join("second.log")); }'
    )}, {'write_csv': 1}) == {'second.log'}
    assert written_names({'src/helpers.py': (
        'def write(out):\n'
        '    path = out / "python.log"\n'
        '    publish_atomic(path, "payload.log")\n'
        '    unlisted(out / "unlisted.log")\n'
    )}, helpers) == {'python.log'}
    assert written_names({"src/write.py": (
        'OUTPUT = "constant.log"\n'
        'def write(out):\n'
        '    path = out / "assigned.log"\n'
        '    path.write_text("payload.log")\n'
        '    open(out / OUTPUT, "a")\n'
        '    (out / "created.log").touch()\n'
        '    (out / "method.log").open(mode="w")\n'
        'def read(out):\n'
        '    path = out / "other-scope.log"\n'
        '    open(path)\n'
        '    helper(out / "helper.log")\n'
    )}) == {"assigned.log", "constant.log", "created.log", "method.log"}
    assert written_names({"src/write.rs": (
        'const OUTPUT: &str = "constant.log";\n'
        'fn write(out: &Path) {\n'
        'let path = out.join("assigned.log"); File::create(&path);\n'
        'fs::write(out.join(OUTPUT), "payload.log");\n'
        'fs::write(helper("helper.log"), "data");\n'
        '}\n'
        'fn read(out: &Path) { let path = out.join("other-scope.log"); read(path); }\n'
    )}) == {"assigned.log", "constant.log"}
    assert not written_names({"src/nested.rs": (
        'fn outer() { let path = dir.join("nested.log"); '
        'fn inner() { File::create(path); } }'
    )})
    assert written_names({"src/open.rs": (
        'fn f(dir: &Path) { '
        'OpenOptions::new().write(true).open(dir.join("options.log")); '
        'File::open(dir.join("read.log")); }'
    )}) == {"options.log"}
    assert written_names({"write.sh": (
        'OUT="$dir/assigned.log"\n'
        'cat "$dir/read.log" > "$OUT"\n'
        'run | tee "$dir/tee.log"\n'
        'run -o "$dir/output.log"\n'
    )}) == {"assigned.log", "tee.log", "output.log"}
    assert not written_names({"read.sh": (
        'source_path() {\n  path="$dir/other-function.log"\n}\n'
        'write_path() {\n  echo data > "$path"\n}\n'
        'printf \'tee "quoted.log"\'\n'
    )})
    assert "read-only.log" not in written_names({
        "src/read.rs": 'fn load(dir: &Path) { read(dir.join("read-only.log")); }',
    }), "a read-only join must not excuse a citation"
    assert "read-open.log" not in written_names({
        "src/read.py": 'open(out / "read-open.log", "r")',
    }), "a read-mode open must not excuse a citation"
    assert faults_in("docs/page.md", "see /tmp/cache/config.json", set()) == [
        "docs/page.md:1: host-local path"
    ], "all cited /tmp paths are host-local"
    for line in ("crates/*/data/tuning-profiles/", "experiments/<name>/data/x.json"):
        assert not any("host-local path" in fault
                       for fault in faults_in("docs/page.md", line, set())), line
    for line in ("/data/run/x.log", "`/mnt/scratch/a`", "see /root/x"):
        assert any("host-local path" in fault
                   for fault in faults_in("docs/page.md", line, set())), line
    assert faults_in("docs/page.md", "see /tmp/source", set())
    assert faults_in("docs/page.md", "run > /tmp/out; see /tmp/input", set())
    for line in ('run > /tmp/out', 'run | tee /tmp/out', 'run -o /tmp/out',
                 'flock -n /tmp/lock run', 'open("/tmp/out", "w")',
                 'Path("/tmp/out").write_text("data")', 'File::create("/tmp/out")'):
        assert not faults_in("docs/page.md", line, set()), line
    assert faults_in("docs/page.md", "see `/tmp`", set())
    assert not faults_in("Containerfile", 'RUN cat /tmp/input', set())
    assert not faults_in(".jit/.gitignore", '/tmp/', set())
    known = resolvable_set(["elsewhere/unique.rs", "docs/local.rs", "root.rs"])
    assert faults_in("docs/page.md", "`unique.rs`", known) == [
        "docs/page.md:1: unique.rs"
    ], "a bare name must not resolve in an unrelated directory"
    assert not faults_in("docs/sub/page.md", "`local.rs` and `root.rs`", known)
    for label in ("unique.rs", "`unique.rs`"):
        assert not faults_in("docs/page.md", f"[{label}](../elsewhere/unique.rs)", known)
    for label in ("different.rs", "`different.rs`"):
        assert faults_in("docs/page.md", f"[{label}](../elsewhere/unique.rs)", known) == [
            "docs/page.md:1: different.rs"
        ]
    assert faults_in("docs/page.md", "[`unique.rs`](../elsewhere/unique.rs) and `unique.rs`", known) == [
        "docs/page.md:1: unique.rs"
    ], "a link resolves only its own label"
    assert faults_in("docs/page.md", "[`gone.rs`](../elsewhere/gone.rs)", known) == [
        "docs/page.md:1: elsewhere/gone.rs", "docs/page.md:1: ../elsewhere/gone.rs"
    ], "a matching label does not excuse an unresolved target"
    assert issue_faults(".jit/issues/test.json", json.dumps({
        "description": "`local.rs`", "state": "ready",
    }), known) == [".jit/issues/test.json:1: local.rs"]
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
            "documents": [{"path": "dev/gone-document.md"}, {"path": "docs/a.md", "assets": [
                {"asset_type": "local", "resolved_path": "docs/gone-asset.json"},
                {"asset_type": "missing", "resolved_path": "dev/gone-missing.md"},
                {"asset_type": "directory", "resolved_path": "docs/"},
                {"asset_type": "local", "resolved_path": "dev/$template"},
                {"asset_type": "external", "resolved_path": "https://example.invalid/x.pdf"},
            ]}],
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
            'fn f(dir: &Path) { fs::write(dir.join(RUN_LOG), "data"); read(dir.join("joined.log")); }\n'
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
        (root / "AGENTS.md").write_text(PROJECTION_TEXT.format(ids="- **kept-rule** — kept\n"))
        (root / ".jit/index.json").write_text("{}")
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
        notes = []
        faults = check(root, root / ".jit", notes)
    assert not notes, notes
    assert any("bad-link.md" in f for f in faults), faults
    assert not any(f.endswith((": docs", ": dev/$template", "x.pdf")) for f in faults), faults
    assert sum("host-local" in f for f in faults) == 6, faults
    assert {f for f in faults if f.startswith("docs/temp.md")} == {
        "docs/temp.md:1: host-local path", "docs/temp.md:2: host-local path",
        "docs/temp.md:5: host-local path"}, faults
    assert sum("tilde.md" in f for f in faults) == 1, faults
    assert [f for f in faults if "dot-root.md" in f] == ["docs/dot-root.md:1: .jit/absent.toml"], faults
    assert not any(f.startswith("docs/listed-") for f in faults), faults
    assert any("pkg/Cargo.toml" in f and "absent-crate" in f for f in faults), faults
    assert not any("pkg/Cargo.toml" in f and "docs" in f for f in faults), faults
    for target in ("dev/gone/after-newline.md", "issue-link.md", "gone/issue.md", "dev/gone-document.md",
                   "docs/gone-asset.json", "dev/gone-missing.md"):
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
    assert unwritten == ["joined.log", "interpolated.log", "targeted.log",
                         "const-read.log", "tuple.log", "fixed.log"], unwritten
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
        "5: kept.log",
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
        "dev/plan.json:1: absent_module.rs", "dev/plan.json:1: kept.sh", "dev/plan.json:1: gone.rs",
    ], faults
    assert not any("absent_outside.rs" in f for f in faults), faults
    layout_self_test()


PROJECTION_TEXT = "# Agents\n<!-- jit:invariants:begin -->\n{ids}<!-- jit:invariants:end -->\n"


def _commit(root: Path, message: str) -> str:
    git(root, "add", "-A")
    git(root, "-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-qm", message, "--no-verify")
    return git(root, "rev-parse", "HEAD").strip()


def layout_self_test() -> None:
    """The store tracked by its own repository, outside the root, and absent."""
    references = (
        "see .jit/issues/kept.json, .jit/absent.json and .jit/tmp/run.txt\n"
        "@/inv/kept-rule and @/invariant/gone-rule\n"
    )

    def code(root: Path, ignore: str = "") -> None:
        git(root, "init", "-q", "-b", "main")
        (root / "docs").mkdir()
        (root / "docs/refs.md").write_text(references)
        (root / "AGENTS.md").write_text(PROJECTION_TEXT.format(ids="- **kept-rule** — kept\n"))
        (root / ".gitignore").write_text(ignore)

    def store(data: Path, issues: dict[str, dict]) -> None:
        data.mkdir(parents=True, exist_ok=True)
        git(data, "init", "-q", "-b", "main")
        (data / "index.json").write_text("{}")
        (data / ".gitignore").write_text("/tmp/\n")
        (data / "issues").mkdir()
        for name, issue in issues.items():
            (data / f"issues/{name}.json").write_text(json.dumps(issue))
        (data / "archive-evidence").mkdir()
        (data / "archive-evidence/c.json").write_text(json.dumps({"source": "dev/archived-away.md"}))
        _commit(data, "store")
        # Untracked and unignored, so a file of the store.
        (data / "issues/untracked.json").write_text(json.dumps({"state": "done"}))

    with tempfile.TemporaryDirectory() as directory:
        # The store in its own repository, mounted where the code ignores it.
        root = Path(directory) / "own"
        root.mkdir()
        code(root, "/.jit/\n")
        main = _commit(root, "main")
        git(root, "checkout", "-q", "-b", "feature")
        (root / "dev").mkdir()
        (root / "dev/branch-only.md").write_text("")
        feature = _commit(root, "feature")
        git(root, "checkout", "-q", "main")
        document = "dev/branch-only.md"
        store(root / ".jit", {
            "kept": {"state": "ready", "description": "names docs/missing.md and dev/branch-only.md"},
            "pinned": {"state": "done", "documents": [{"path": document, "commit": feature}]},
            "unpinned": {"state": "done", "documents": [{"path": document, "commit": None}]},
            "stale-pin": {"state": "done", "documents": [{"path": document, "commit": main}]},
            "nowhere": {"state": "done", "documents": [{"path": "dev/nowhere.md", "commit": None}]},
        })
        notes = []
        faults = check(root, root / ".jit", notes)
        assert sorted(faults) == sorted([
            "docs/refs.md:1: .jit/absent.json",
            "docs/refs.md:2: @/inv/gone-rule",
            ".jit/issues/kept.json:1: docs/missing.md",
            ".jit/issues/stale-pin.json:0: dev/branch-only.md",
            ".jit/issues/nowhere.json:0: dev/nowhere.md",
            ".jit/archive-evidence/c.json:1: dev/archived-away.md",
        ]), faults
        assert not notes, notes

        # The store outside the root, named by the data root.
        root = Path(directory) / "outside"
        root.mkdir()
        code(root)
        (root / ".jit/issues").mkdir(parents=True)
        (root / ".jit/issues/tracked.json").write_text(json.dumps({"description": "dev/gone.md"}))
        _commit(root, "main")
        data = Path(directory) / "shared/.jit"
        store(data, {"kept": {"state": "ready", "description": "names docs/missing.md"}})
        notes = []
        faults = check(root, data, notes)
        assert sorted(faults) == ["docs/refs.md:1: .jit/absent.json", "docs/refs.md:2: @/inv/gone-rule"], faults
        assert notes == ["check-references: skipped the prose of 4 issue and archive record(s): "
                         "the data root lies outside the checkout"], notes

        # No store: tracker references pass and are counted.
        root = Path(directory) / "bare"
        root.mkdir()
        code(root)
        _commit(root, "main")
        notes = []
        faults = check(root, root / ".jit", notes)
        assert faults == ["docs/refs.md:2: @/inv/gone-rule"], faults
        assert notes == ["check-references: 3 tracker reference(s) left unchecked: "
                         "no store index at the data root"], notes
    root = Path("/checkout")
    assert data_root(root, {}) == root / ".jit"
    assert data_root(root, {"JIT_DATA_DIR": "/shared/.jit"}) == Path("/shared/.jit")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    if parser.parse_args().self_test:
        self_test()
        print("check-references: self-test ok")
        return 0
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
    notes = []
    faults = check(root, data_root(root), notes)
    for fault in faults:
        print(fault)
    for note in notes:
        print(note)
    print(f"check-references: {len(faults)} fault(s)" if faults else "check-references: ok")
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
