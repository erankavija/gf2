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
script fails when a changed production path is `code-differs`.

Usage: make-production-drift.py
"""

import hashlib
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


def classify(path, measured, snapshot):
    current = ROOT / path
    if not current.is_file():
        return {"path": path, "class": "removed", "measured_sha256": measured}
    data = current.read_bytes()
    entry = {"path": path, "measured_sha256": measured, "current_sha256": sha256(data)}
    if entry["current_sha256"] == measured:
        return entry | {"class": "identical"}
    held = (snapshot / path).read_bytes()
    if sha256(held) != measured:
        raise SystemExit(f"{snapshot / path} does not hold the digest its receipt records")
    if path.endswith(".rs") and rust_code_text.code_text(held.decode()) == rust_code_text.code_text(data.decode()):
        return entry | {"class": "comment-or-blank-only"}
    return entry | {"class": "code-differs", "changed_by": changing_commits(path, measured)}


def task_class(path):
    """The class of a production path this task changes, against its anchor bytes."""
    if path.endswith(".asm.txt"):
        return {"path": path, "class": "assembly-listing"}
    current = ROOT / path
    comment_only = (
        path.endswith(".rs")
        and path in ANCHOR.digests()
        and current.is_file()
        and rust_code_text.code_text(ANCHOR.bytes(path).decode()) == rust_code_text.code_text(current.read_text())
    )
    return {"path": path, "class": "comment-or-blank-only" if comment_only else "code-differs"}


def main():
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

    tracked = [
        path
        for package in directories
        for path in git("ls-files", "--", f"{package}/src").splitlines()
    ]
    baselines = []
    for measured in measured_sets.values():
        snapshot = ROOT / measured["receipts"][0]["snapshot"]
        files = [
            classify(path, digest, snapshot) for path, digest in sorted(measured["digests"].items())
        ]
        files += [
            {"path": path, "class": "added", "current_sha256": sha256((ROOT / path).read_bytes())}
            for path in tracked
            if path not in measured["digests"]
        ]
        counts = {}
        for entry in files:
            counts[entry["class"]] = counts.get(entry["class"], 0) + 1
        baselines.append({"receipts": measured["receipts"], "class_counts": counts, "files": files})

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


if __name__ == "__main__":
    main()
