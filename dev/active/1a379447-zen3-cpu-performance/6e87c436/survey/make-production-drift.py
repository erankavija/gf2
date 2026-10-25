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
`select-receipts.py` selector enumerates, of any schema, as `code-identical` or
`code-differs`: by the same file classes over the compiled files under `crates/`
its `inputs/producing/producing-snapshot.json` pins, or else over the compiled
files at the `source_revision` it records, read from git as bytes. Receipts with
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


def added_files(directories, digests, keep=lambda path: True):
    """Tracked `src/` files of `directories` that `digests` does not pin and `keep` accepts."""
    return [
        {"path": path, "class": "added", "current_sha256": sha256((ROOT / path).read_bytes())}
        for package in directories
        for path in git("ls-files", "--", f"{package}/src").splitlines()
        if path not in digests and keep(path)
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
    "code-differs": "the row's basis does not prove it; `differing_files` names each "
    "file that differs or could not be resolved",
}
COMPILED = (
    "only compiled production files take part: `.rs` files, `Cargo.toml`, and the "
    "non-Markdown files a current `.rs` file names in a literal `include!`, "
    "`include_str!` or `include_bytes!`"
)
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
    "`source_revision` resolves; `differing_files` names the producing inputs",
}
FILE_CLASSES_THAT_DIFFER = ("code-differs", "removed", "added")
INCLUDE = re.compile(r'include(?:_str|_bytes)?!\s*\(\s*"([^"]+)"')
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


def included(paths, read):
    """Root-relative targets of the literal includes in the `.rs` files among `paths`."""
    targets = set()
    for path in paths:
        if path.endswith(".rs"):
            for name in INCLUDE.findall(read(path).decode(errors="replace")):
                targets.add(os.path.normpath(os.path.join(os.path.dirname(path), name)))
    return targets


def compiled_filter(directories):
    """Whether a root-relative path is a compiled production file of the current tree."""
    tracked = [
        path for directory in directories for path in git("ls-files", "--", directory).splitlines()
    ]
    targets = included(tracked, lambda path: (ROOT / path).read_bytes())

    def compiled(path):
        name = pathlib.PurePath(path).name
        if name.endswith(".md"):
            return False
        return name.endswith(".rs") or name == "Cargo.toml" or path in targets

    return compiled


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
    targets = included(listed, show)
    measured = {
        path: sha256(show(path))
        for path in listed
        if not path.endswith(".md")
        and (path.endswith(".rs") or pathlib.PurePath(path).name == "Cargo.toml" or path in targets)
    }
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
                "class": "code-differs",
                "basis": UNPINNED,
                "differing_files": [{"path": path, "class": "unresolved"} for path in content],
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
