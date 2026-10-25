#!/usr/bin/env python3
"""Compare the production sources the dense campaigns measured with this tree (jit:6e87c436).

A dense receipt records the SHA-256 of every producing input and snapshots its
bytes. For the files of the measured production packages this script classifies
each one against the working tree and writes `production-drift.json` beside
itself:

  identical              the digests agree
  comment-or-blank-only  a Rust source whose bytes differ and whose code text
                         agrees, by `rust_code_text.RULE`
  code-differs           every other differing file, with the commits that
                         changed the path after the newest commit holding the
                         measured bytes
  removed                a measured file this tree lacks
  added                  a tracked `src/` file of a package the receipt does
                         not list

The record also lists the package paths whose bytes differ from the task
anchor, which `anchor-baseline.json` identifies by per-path digest. Each one
that is a producing input or lies under `src/` is classified against its
snapshotted anchor bytes by the same rule; an annotated assembly listing is classed
`assembly-listing`, since `make-asm-comparison.py` compares it per symbol. The
script fails when a changed production path is `code-differs`, and refuses to
write on a tree that differs from the one `dense-verdict-end-state.json` pins.

With `--all-receipts OUTPUT` it instead classifies every committed receipt the
`select-receipts.py` selector enumerates, of any schema, as `code-identical`,
`code-differs` or `unresolvable`: by the same file classes over the compiled files under `crates/`
its `inputs/producing/producing-snapshot.json` pins, or else over the compiled
files at the `source_revision` it records, read from git as bytes; a receipt
with neither is `unresolvable`. Receipts with
one measured identity share a row; the record defines each class and basis.
This mode lists no commits, and any tree may be compared.

Usage:
  make-production-drift.py
  make-production-drift.py --all-receipts OUTPUT
"""

import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import subprocess

from locate import (
    ANCHOR,
    CAMPAIGNS,
    HERE,
    PACKAGES,
    ROOT,
    byte_class,
    end_state,
    repo_artifacts,
    repository_files,
    rust_code_text,
)


def git(*arguments):
    return subprocess.run(
        ["git", "-C", str(ROOT), *arguments], capture_output=True, check=True, text=True
    ).stdout


SNAPSHOT = pathlib.Path("inputs") / "producing"
DIGEST_MAPS = ("behavior_sha256", "build_inputs_sha256")

def sha256(data):
    return hashlib.sha256(data).hexdigest()


def changing_commits(path, measured):
    """Commits that changed `path` after the newest one holding the `measured` digest."""
    history = git("log", "--format=%H%x09%s", "--", path).splitlines()
    after = []
    for entry in history:
        commit, subject = entry.split("\t", 1)
        blob = subprocess.run(
            ["git", "-C", str(ROOT), "show", f"{commit}:{path}"], capture_output=True
        )
        if blob.returncode == 0 and sha256(blob.stdout) == measured:
            return after
        issue = re.search(r"\(jit:([0-9a-f]{8})\)", subject)
        after.append({"commit": commit, "issue": issue[1] if issue else None, "subject": subject})
    raise SystemExit(f"{path}: no commit holds the measured digest {measured}")


class UnavailableSnapshot(LookupError):
    """No snapshot holds the measured bytes of a differing file."""


def held_bytes(path, measured, snapshots):
    for snapshot in snapshots:
        held = snapshot / path
        if held.is_file() and sha256(data := held.read_bytes()) == measured:
            return data
    raise UnavailableSnapshot(
        f"{snapshots[0] / path} does not hold the digest its receipt records"
    )


def classify(path, measured, snapshots, history=True):
    """The class of `path` against its `measured` bytes.

    `snapshots` lists directories holding measured bytes at their paths, or is a
    callable `(path, measured)` returning them.
    """
    current = ROOT / path
    if not current.is_file():
        return {"path": path, "class": "removed", "measured_sha256": measured}
    data = current.read_bytes()
    entry = {"path": path, "measured_sha256": measured, "current_sha256": sha256(data)}
    if entry["current_sha256"] == measured:
        return entry | {"class": "identical"}
    held = (
        snapshots(path, measured) if callable(snapshots) else held_bytes(path, measured, snapshots)
    )
    if byte_class(path, held, data) == "comment-or-blank-only":
        return entry | {"class": "comment-or-blank-only"}
    if not history:
        return entry | {"class": "code-differs"}
    return entry | {"class": "code-differs", "changed_by": changing_commits(path, measured)}


def class_counts(files):
    counts = {}
    for entry in files:
        counts[entry["class"]] = counts.get(entry["class"], 0) + 1
    return counts


def added_files(directories, digests, keep=None):
    """Tracked files of `directories` that `digests` does not pin: under `src/`, or
    anywhere in the package that `keep` accepts when it is given."""
    return [
        {"path": path, "class": "added", "current_sha256": sha256((ROOT / path).read_bytes())}
        for package in directories
        for path in git("ls-files", "--", package if keep else f"{package}/src").splitlines()
        if path not in digests and (keep is None or keep(path))
    ]


def task_class(path):
    """The class of a production path this task changes, against its anchor bytes."""
    if path.endswith(".asm.txt"):
        return {"path": path, "class": "assembly-listing"}
    current = ROOT / path
    comparable = path in ANCHOR.digests() and current.is_file()
    return {
        "path": path,
        "class": byte_class(path, ANCHOR.bytes(path), current.read_bytes())
        if comparable
        else "code-differs",
    }


def main():
    end_state().require_matching_tree("production-drift.json")
    directories = [repository_files.package_directory(ROOT, name) for name in PACKAGES]
    measured_sets = {}
    for campaign in CAMPAIGNS:
        directory = repo_artifacts.receipt(campaign)
        producing = json.loads((ROOT / directory / "receipt.json").read_bytes())["source"][
            "producing"
        ]
        digests = {
            path: digest
            for name in DIGEST_MAPS
            for path, digest in producing[name].items()
            if any(path.startswith(f"{package}/") for package in directories)
        }
        key = json.dumps(digests, sort_keys=True)
        measured_sets.setdefault(key, {"receipts": [], "digests": digests})["receipts"].append(
            {
                "campaign_id": campaign,
                "receipt": str(directory / "receipt.json"),
                "snapshot": str(directory / SNAPSHOT),
            }
        )

    baselines = []
    for measured in measured_sets.values():
        snapshot = ROOT / measured["receipts"][0]["snapshot"]
        try:
            files = [
                classify(path, digest, [snapshot])
                for path, digest in sorted(measured["digests"].items())
            ]
        except UnavailableSnapshot as error:
            raise SystemExit(str(error)) from None
        files += added_files(directories, measured["digests"])
        baselines.append(
            {"receipts": measured["receipts"], "class_counts": class_counts(files), "files": files}
        )

    producing_inputs = set()
    manifest = json.loads(
        (ROOT / repository_files.live_file(ROOT, "dense-producing-inputs.json")).read_bytes()
    )
    for value in manifest.values():
        if isinstance(value, list):
            producing_inputs.update(value)
    changed = ANCHOR.changed()
    production = [
        task_class(path)
        for path in changed
        if path in producing_inputs
        or any(path.startswith(f"{package}/src/") for package in directories)
    ]

    record = {
        "schema": "dense-production-drift-v1",
        "issue": "6e87c436",
        "packages": directories,
        "classifier_rule": rust_code_text.RULE,
        "task_change": {
            "anchor": ANCHOR.identity(),
            "changed_package_paths": changed,
            "changed_production_paths": production,
        },
        "baselines": baselines,
    }
    output = HERE / "production-drift.json"
    output.write_text(json.dumps(record, indent=2) + "\n")
    for baseline in baselines:
        print([r["campaign_id"] for r in baseline["receipts"]], baseline["class_counts"])
    print(f"{output.relative_to(ROOT)}: task changes {len(production)} production paths")
    code = [entry["path"] for entry in production if entry["class"] == "code-differs"]
    if code:
        raise SystemExit(f"production code differs from the task anchor: {code}")


# Packages under this directory are the production crates.
PRODUCTION_ROOT = pathlib.Path("crates")

SCOPE = (
    "A row's class applies to every cell of each receipt it lists: the measured "
    "content is a receipt-level producing input."
)
RECEIPT_CLASSES = {
    "code-identical": "the row's basis proves the measured production code equal to the "
    "current tree",
    "code-differs": "the row's basis shows a difference; `differing_files` names each "
    "differing file",
    "unresolvable": "the receipt pins neither production files nor a resolvable source "
    "revision; `unresolved_inputs` names its pinned producing inputs",
}
COMPILED = (
    "only compiled production files take part: Rust and native-kernel sources "
    "(`.rs`, `.hip`, C and C++ sources and headers), `Cargo.toml`, and the "
    "non-Markdown files a current `.rs` file names in a literal `include!`, "
    "`include_str!` or `include_bytes!`; a package's `tests/`, `benches/` and "
    "`examples/` targets, modules loaded only under `#[cfg(test)]` and files "
    "included only from test code are not production"
)
SOURCE_SUFFIXES = (".rs", ".hip", ".c", ".cc", ".cpp", ".h", ".hpp", ".cuh")
NON_PRODUCTION_TARGETS = ("tests", "benches", "examples")
FILES = "production files"
REVISION = "production files at the source revision"
UNAVAILABLE = "measured bytes unavailable"
UNPINNED = "measured content not pinned"
BASES = {
    FILES: "the compiled files under `crates/` the producing snapshot pins, against the "
    "tree: identical or comment-or-blank-only is equal; code-differs, removed, and a "
    "tracked `src/` file the receipt does not pin (added) differ",
    REVISION: "the snapshot pins no file under `crates/`; the compiled files of every "
    "package under `crates/` at the `source_revision` the receipt records, read as "
    "bytes, against the tree by the same file classes",
    UNAVAILABLE: "no snapshot of the row holds the measured bytes of a pinned file "
    "whose current bytes differ",
    UNPINNED: "the snapshot pins no file under `crates/` and no recorded "
    "`source_revision` resolves",
}
FILE_CLASSES_THAT_DIFFER = ("code-differs", "removed", "added")
INCLUDE = re.compile(r'include(?:_str|_bytes)?!\s*\(\s*"([^"]+)"')
_VISIBILITY = r"(?:pub(?:\([^)]*\))?\s+)?"
MODULE_DECLARATION = re.compile(r"((?:#\[[^\]]*\]\s*)*)" + _VISIBILITY + r"mod\s+(\w+)\s*;")
PATH_ATTRIBUTE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')
INLINE_TEST_MODULE = re.compile(r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*" + _VISIBILITY + r"mod\s+\w+\s*\{")
# Files of a receipt directory that may record the revision its arms were built from.
REVISION_RECORDS = ("receipt.json", "checkpoints/manifest.json")
BUILD_RECORD = "build-identity.json"


def sibling_module(name):
    """The live script called `name`, loaded as a module."""
    path = ROOT / repository_files.live_file(ROOT, name)
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_spans(text):
    """Character spans of the inline `#[cfg(test)]` modules of a Rust source."""
    spans = []
    for match in INLINE_TEST_MODULE.finditer(text):
        depth, end = 1, match.end()
        while depth and end < len(text):
            depth += {"{": 1, "}": -1}.get(text[end], 0)
            end += 1
        spans.append((match.start(), end))
    return spans


def module_file(parent, name, listed):
    """The file a `mod name;` in `parent` loads, if `listed` holds it."""
    base = pathlib.PurePath(parent)
    directory = base.parent if base.name in ("lib.rs", "main.rs", "mod.rs") else base.with_suffix("")
    for candidate in (directory / f"{name}.rs", directory / name / "mod.rs"):
        if str(candidate) in listed:
            return str(candidate)
    return None


def production_filter(listed, read):
    """Whether a root-relative path among `listed` is a compiled production file.

    Production files are the native and Rust sources, `Cargo.toml` and the literal
    include targets of the package's library, binaries and build script; a package's
    `tests/`, `benches/` and `examples/` targets, Rust modules loaded only by
    `#[cfg(test)]` declarations and files included only from test code are not.
    """
    listed = set(listed)
    sources = {path: read(path).decode(errors="replace") for path in listed if path.endswith(".rs")}
    loads = {}
    for path, text in sources.items():
        for match in MODULE_DECLARATION.finditer(text):
            attributes = match.group(1)
            explicit = PATH_ATTRIBUTE.search(attributes)
            if explicit:
                child = os.path.normpath(os.path.join(os.path.dirname(path), explicit.group(1)))
                child = child if child in listed else None
            else:
                child = module_file(path, match.group(2), listed)
            if child:
                loads.setdefault(child, []).append((path, "#[cfg(test)]" in attributes))
    test_only = set()
    changed = True
    while changed:
        changed = False
        for child, parents in loads.items():
            if child not in test_only and all(
                gated or parent in test_only for parent, gated in parents
            ):
                test_only.add(child)
                changed = True
    targets = set()
    for path, text in sources.items():
        if path in test_only or not_production_target(path):
            continue
        spans = test_spans(text)
        for match in INCLUDE.finditer(text):
            if not any(start <= match.start() < end for start, end in spans):
                targets.add(os.path.normpath(os.path.join(os.path.dirname(path), match.group(1))))

    def compiled(path):
        if not_production_target(path) or path in test_only:
            return False
        name = pathlib.PurePath(path).name
        return name.endswith(SOURCE_SUFFIXES) or name == "Cargo.toml" or path in targets

    return compiled


def not_production_target(path):
    parts = pathlib.PurePath(path).parts
    return len(parts) > 3 and parts[2] in NON_PRODUCTION_TARGETS


def compiled_filter(directories):
    """Whether a root-relative path is a compiled production file of the current tree."""
    tracked = [
        path for directory in directories for path in git("ls-files", "--", directory).splitlines()
    ]
    return production_filter(tracked, lambda path: (ROOT / path).read_bytes())


def file_row(receipts, digests, directories, compiled):
    snapshots = [ROOT / receipt["snapshot"] for receipt in receipts]
    packages = [
        directory
        for directory in directories
        if any(path.startswith(f"{directory}/") for path in digests)
    ]
    digests = {path: digest for path, digest in digests.items() if compiled(path)}
    files, unavailable = [], []
    for path, digest in sorted(digests.items()):
        try:
            files.append(classify(path, digest, snapshots, history=False))
        except UnavailableSnapshot:
            unavailable.append({"path": path, "class": "unavailable"})
    files += added_files(packages, digests, compiled)
    row = {"packages": packages}
    if unavailable:
        return row | {"class": "code-differs", "basis": UNAVAILABLE, "differing_files": unavailable}
    return row | verdict(files, FILES)


def verdict(files, basis):
    differing = [
        {"path": entry["path"], "class": entry["class"]}
        for entry in files
        if entry["class"] in FILE_CLASSES_THAT_DIFFER
    ]
    return {
        "class": "code-differs" if differing else "code-identical",
        "basis": basis,
        "class_counts": class_counts(files),
        "differing_files": differing,
    }


def revision_row(revision, directories, compiled):
    """The compiled production files at `revision` against the tree."""

    def show(path, measured=None):
        return subprocess.run(
            ["git", "-C", str(ROOT), "show", f"{revision}:{path}"], capture_output=True, check=True
        ).stdout

    listed = git("ls-tree", "-r", "--name-only", revision, "--", *directories).splitlines()
    at_revision = production_filter(listed, show)
    measured = {path: sha256(show(path)) for path in listed if at_revision(path)}
    packages = [
        directory
        for directory in directories
        if any(path.startswith(f"{directory}/") for path in measured)
    ]
    files = [classify(path, digest, show, history=False) for path, digest in sorted(measured.items())]
    files += added_files(packages, measured, compiled)
    return {"packages": packages, "source_revision": revision} | verdict(files, REVISION)


def source_revision(receipt):
    """The one commit the receipt's records name as `source_revision`, or `None`."""
    directory = ROOT / pathlib.PurePath(receipt).parent
    records = [directory / name for name in REVISION_RECORDS]
    records += sorted((directory / "inputs").rglob(BUILD_RECORD))
    named = set()

    def walk(node):
        if isinstance(node, dict):
            for key, value in node.items():
                if key == "source_revision" and isinstance(value, str) and value:
                    named.add(value)
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)

    for record in records:
        if record.is_file():
            walk(json.loads(record.read_bytes()))
    if len(named) != 1:
        return None
    (revision,) = named
    resolved = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "--verify", "--quiet", f"{revision}^{{commit}}"],
        capture_output=True,
        text=True,
    )
    return resolved.stdout.strip() if resolved.returncode == 0 else None


def measured_identity(receipt, producing, directories):
    """The basis and content by which `receipt` is compared, as a hashable key and value."""
    digests = {
        path: digest
        for name in DIGEST_MAPS
        for path, digest in producing[name].items()
        if any(path.startswith(f"{directory}/") for directory in directories)
    }
    if digests:
        return FILES, digests
    revision = source_revision(receipt)
    if revision is not None:
        return REVISION, revision
    return UNPINNED, sorted({path for name in DIGEST_MAPS for path in producing[name]})


def all_receipts(output):
    """Classifies every committed receipt and writes the record to `output`."""
    selected, skipped = sibling_module("select-receipts.py").select(ROOT)
    directories = sorted(
        str(pathlib.Path(manifest).parent)
        for manifest in repository_files.tracked_files(ROOT, "Cargo.toml")
        if pathlib.Path(manifest).parent.parent == PRODUCTION_ROOT
    )
    compiled = compiled_filter(directories)
    measured_sets = {}
    for receipt in sorted(selected + [path for path, _ in skipped]):
        snapshot = pathlib.Path(receipt).parent / SNAPSHOT
        producing = json.loads((ROOT / snapshot / "producing-snapshot.json").read_bytes())
        basis, content = measured_identity(receipt, producing, directories)
        key = json.dumps([basis, content], sort_keys=True)
        measured_sets.setdefault(key, {"basis": basis, "content": content, "receipts": []})[
            "receipts"
        ].append({"receipt": receipt, "snapshot": str(snapshot)})
    rows = []
    for measured in measured_sets.values():
        basis, content = measured["basis"], measured["content"]
        if basis == FILES:
            row = file_row(measured["receipts"], content, directories, compiled)
        elif basis == REVISION:
            row = revision_row(content, directories, compiled)
        else:
            row = {
                "class": "unresolvable",
                "basis": UNPINNED,
                "unresolved_inputs": content,
            }
        rows.append({"receipts": measured["receipts"]} | row)
    record = {
        "issue": "ecad1724",
        "scope": SCOPE,
        "receipt_selector": repository_files.live_file(ROOT, "select-receipts.py"),
        "production_packages": directories,
        "compiled_files": COMPILED,
        "receipt_classes": RECEIPT_CLASSES,
        "bases": BASES,
        "classifier_rule": rust_code_text.RULE,
        "rows": rows,
    }
    output.write_text(json.dumps(record, indent=2) + "\n")
    counts = {}
    for row in rows:
        label = f"{row['class']} ({row['basis']})"
        counts[label] = counts.get(label, 0) + len(row["receipts"])
    print(f"{output}: receipts by class and basis {counts}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--all-receipts", type=pathlib.Path, metavar="OUTPUT")
    arguments = parser.parse_args()
    if arguments.all_receipts:
        all_receipts(arguments.all_receipts)
    else:
        main()
