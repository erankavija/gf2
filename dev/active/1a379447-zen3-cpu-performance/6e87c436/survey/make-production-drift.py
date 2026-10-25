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
`code-differs`: by the same file classes over the files under `crates/` its
`inputs/producing/producing-snapshot.json` pins, or else by its arms' executable
digests against the live arm build records of `record-peak-memory.py`.
Receipts with one measured identity share a row; the record defines each class
and basis. This mode lists no commits, and any tree may be compared.

Usage:
  make-production-drift.py
  make-production-drift.py --all-receipts OUTPUT
"""

import argparse
import hashlib
import importlib.util
import json
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
    """The class of `path` against the first of `snapshots` holding its `measured` bytes."""
    current = ROOT / path
    if not current.is_file():
        return {"path": path, "class": "removed", "measured_sha256": measured}
    data = current.read_bytes()
    entry = {"path": path, "measured_sha256": measured, "current_sha256": sha256(data)}
    if entry["current_sha256"] == measured:
        return entry | {"class": "identical"}
    held = held_bytes(path, measured, snapshots)
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


def added_files(directories, digests):
    """Tracked `src/` files of `directories` that `digests` does not pin."""
    return [
        {"path": path, "class": "added", "current_sha256": sha256((ROOT / path).read_bytes())}
        for package in directories
        for path in git("ls-files", "--", f"{package}/src").splitlines()
        if path not in digests
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
    "code-identical": "the row's basis proves the measured content equal to the current tree",
    "code-differs": "the row's basis does not prove it; `differing_files` names each "
    "file or executable that differs or could not be resolved",
}
FILES = "production files"
EXECUTABLES = "executable identity"
UNAVAILABLE = "measured bytes unavailable"
UNPINNED = "measured content not pinned"
BASES = {
    FILES: "the pinned files of the measured packages under `crates/` against the tree: "
    "identical or comment-or-blank-only is equal; code-differs, removed, and a tracked "
    "`src/` file the receipt does not pin (added) differ",
    EXECUTABLES: "the receipt pins no file under `crates/`; each arm's pinned executable "
    "digest against the same arm's digest in the live arm build records, where a "
    "different or unrecorded digest differs",
    UNAVAILABLE: "no snapshot of the row holds the measured bytes of a pinned file "
    "whose current bytes differ",
    UNPINNED: "the receipt pins neither a file under `crates/` nor an executable digest "
    "for every arm; `differing_files` names its unresolved producing inputs",
}
FILE_CLASSES_THAT_DIFFER = ("code-differs", "removed", "added")


def sibling_module(name):
    """The live script called `name`, loaded as a module."""
    path = ROOT / repository_files.live_file(ROOT, name)
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def live_executables():
    """Arm name, as a receipt records it, to the digests the live arm build records hold."""
    peak = sibling_module("record-peak-memory.py")
    digests = {}
    for _, record in peak.build_records():
        for arm, row in record["arms"].items():
            for key in peak.smoke.CODES:
                digests.setdefault(f"{arm}-{key}", set()).add(row["executable_sha256"])
    return digests


def file_row(receipts, digests, directories):
    snapshots = [ROOT / receipt["snapshot"] for receipt in receipts]
    packages = [
        directory
        for directory in directories
        if any(path.startswith(f"{directory}/") for path in digests)
    ]
    files, unavailable = [], []
    for path, digest in sorted(digests.items()):
        try:
            files.append(classify(path, digest, snapshots, history=False))
        except UnavailableSnapshot:
            unavailable.append({"path": path, "class": "unavailable"})
    files += added_files(packages, digests)
    row = {"packages": packages}
    if unavailable:
        return row | {"class": "code-differs", "basis": UNAVAILABLE, "differing_files": unavailable}
    differing = [
        {"path": entry["path"], "class": entry["class"]}
        for entry in files
        if entry["class"] in FILE_CLASSES_THAT_DIFFER
    ]
    return row | {
        "class": "code-differs" if differing else "code-identical",
        "basis": FILES,
        "class_counts": class_counts(files),
        "differing_files": differing,
    }


def executable_row(arms, live):
    differing = []
    for arm, (path, digest) in sorted(arms.items()):
        held = live.get(arm)
        if held != {digest}:
            differing.append(
                {
                    "arm": arm,
                    "path": path,
                    "executable_sha256": digest,
                    "class": "executable-differs" if held else "arm-unrecorded",
                }
            )
    return {
        "class": "code-differs" if differing else "code-identical",
        "basis": EXECUTABLES,
        "differing_files": differing,
    }


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
    arms = json.loads((ROOT / receipt).read_bytes()).get("arms") or {}
    if arms and all("executable_sha256" in arm for arm in arms.values()):
        return EXECUTABLES, {
            name: (arm.get("executable_path"), arm["executable_sha256"])
            for name, arm in arms.items()
        }
    return UNPINNED, sorted({path for name in DIGEST_MAPS for path in producing[name]})


def all_receipts(output):
    """Classifies every committed receipt and writes the record to `output`."""
    selected, skipped = sibling_module("select-receipts.py").select(ROOT)
    directories = sorted(
        str(pathlib.Path(manifest).parent)
        for manifest in repository_files.tracked_files(ROOT, "Cargo.toml")
        if pathlib.Path(manifest).parent.parent == PRODUCTION_ROOT
    )
    measured_sets = {}
    for receipt in sorted(selected + [path for path, _ in skipped]):
        snapshot = pathlib.Path(receipt).parent / SNAPSHOT
        producing = json.loads((ROOT / snapshot / "producing-snapshot.json").read_bytes())
        basis, content = measured_identity(receipt, producing, directories)
        key = json.dumps([basis, content], sort_keys=True)
        measured_sets.setdefault(key, {"basis": basis, "content": content, "receipts": []})[
            "receipts"
        ].append({"receipt": receipt, "snapshot": str(snapshot)})
    live = None
    rows = []
    for measured in measured_sets.values():
        basis, content = measured["basis"], measured["content"]
        if basis == FILES:
            row = file_row(measured["receipts"], content, directories)
        elif basis == EXECUTABLES:
            live = live_executables() if live is None else live
            row = executable_row(content, live)
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
