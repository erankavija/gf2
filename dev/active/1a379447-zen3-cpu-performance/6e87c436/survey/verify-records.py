#!/usr/bin/env python3
"""Verify the verdict's drift and assembly records against the content they pin (jit:66698c3c).

The records describe the tree `dense-verdict-end-state.json` pins. Each is
re-derived from content only: the receipts' snapshotted producing inputs, the
task anchor's snapshots, and committed blobs that git holds, found by the
digest a record or the end state names. The working tree is never read for
source bytes, so the check holds on any later tree while those blobs exist. It
writes nothing and exits non-zero listing every record entry that does not
re-derive.

Usage: verify-records.py [--drift PATH] [--assembly PATH]

The options name record files to verify instead of the committed ones.
"""

import argparse
import json
import pathlib
import subprocess

from locate import (
    ANCHOR,
    CAMPAIGNS,
    HERE,
    PACKAGES,
    ROOT,
    asm_listing,
    byte_class,
    content_anchor,
    end_state,
    repository_files,
    rust_code_text,
)

DIGEST_MAPS = ("behavior_sha256", "build_inputs_sha256")


def git(*arguments):
    done = subprocess.run(["git", "-C", str(ROOT), *arguments], capture_output=True, text=True)
    return done.stdout.strip() if done.returncode == 0 else None


def changes_path(commit, path):
    """Whether `path` differs between `commit` and its first parent."""
    parent = git("rev-parse", f"{commit}^:{path}")
    return git("rev-parse", f"{commit}:{path}") != parent


class Verifier:
    def __init__(self):
        self.end = end_state().digests()
        self.faults = []

    def expect(self, holds, message):
        if not holds:
            self.faults.append(message)

    def end_bytes(self, path):
        """The committed bytes of `path` at the pinned end state, or `None`."""
        digest = self.end.get(path)
        return None if digest is None else content_anchor.history_blob(ROOT, path, digest)

    def changed_by(self, entry):
        path, listed = entry["path"], entry["changed_by"]
        for change in listed:
            commit = change["commit"]
            self.expect(
                git("log", "-1", "--format=%s", commit) == change["subject"]
                and changes_path(commit, path),
                f"{path}: {commit} is not a commit of that subject changing the path",
            )
        digests = set()
        for change in listed:
            held = subprocess.run(
                ["git", "-C", str(ROOT), "show", f"{change['commit']}:{path}"], capture_output=True
            )
            digests.add(content_anchor.sha256(held.stdout))
        self.expect(
            entry["current_sha256"] in digests and entry["measured_sha256"] not in digests,
            f"{path}: changed_by does not end at the end-state content or reaches the measured one",
        )

    def baseline(self, baseline, directories):
        measured = {}
        for receipt in baseline["receipts"]:
            producing = json.loads((ROOT / receipt["receipt"]).read_bytes())["source"]["producing"]
            digests = {
                path: digest
                for name in DIGEST_MAPS
                for path, digest in producing[name].items()
                if any(path.startswith(f"{package}/") for package in directories)
            }
            measured = measured or digests
            self.expect(digests == measured, f"{receipt['receipt']}: producing digests differ")
        snapshot = ROOT / baseline["receipts"][0]["snapshot"]
        rows = {entry["path"]: entry for entry in baseline["files"]}
        expected = set(measured) | {
            path
            for path in self.end
            if any(path.startswith(f"{package}/src/") for package in directories)
        }
        self.expect(set(rows) == expected, "baseline file rows are not measured and end-state src paths")
        counts = {}
        for path, entry in sorted(rows.items()):
            counts[entry["class"]] = counts.get(entry["class"], 0) + 1
            if entry["class"] == "removed":
                self.expect(path not in self.end, f"{path}: recorded removed but pinned")
                continue
            self.expect(self.end.get(path) == entry["current_sha256"], f"{path}: current digest is not the end state's")
            if entry["class"] in ("added", "identical"):
                self.expect(
                    (entry["class"] == "identical") == (entry["current_sha256"] == measured.get(path)),
                    f"{path}: class {entry['class']} disagrees with the digests",
                )
                continue
            held = (snapshot / path).read_bytes()
            current = self.end_bytes(path)
            self.expect(content_anchor.sha256(held) == entry["measured_sha256"], f"{snapshot / path}: not the measured digest")
            self.expect(current is not None, f"{path}: no blob holds the end-state digest")
            if current is None:
                continue
            self.expect(byte_class(path, held, current) == entry["class"], f"{path}: class is not {entry['class']}")
            if entry["class"] == "code-differs":
                self.changed_by(entry)
        self.expect(counts == baseline["class_counts"], "class_counts do not total the rows")

    def drift(self, record, directories):
        self.expect(record["classifier_rule"] == rust_code_text.RULE, "classifier rule differs from rust_code_text.RULE")
        self.expect(record["packages"] == directories, "packages differ")
        for baseline in record["baselines"]:
            self.baseline(baseline, directories)
        task = record["task_change"]
        self.expect(task["anchor"] == ANCHOR.identity(), "anchor baseline digest differs from the pinned one")
        anchor = ANCHOR.digests()
        changed = sorted(path for path in set(anchor) | set(self.end) if anchor.get(path) != self.end.get(path))
        self.expect(task["changed_package_paths"] == changed, "changed_package_paths differ from anchor against end state")
        listed = {entry["path"]: entry["class"] for entry in task["changed_production_paths"]}
        sources = {path for path in changed if any(path.startswith(f"{p}/src/") for p in directories)}
        self.expect(sources <= set(listed) <= set(changed), "changed_production_paths do not cover the changed src paths")
        for path, recorded in listed.items():
            if path.endswith(".asm.txt"):
                derived = "assembly-listing"
            else:
                current = self.end_bytes(path)
                derived = "code-differs" if current is None else byte_class(path, ANCHOR.bytes(path), current)
            self.expect(derived == recorded, f"{path}: task class is not {recorded}")
        return changed

    def assembly(self, record, changed, directories):
        self.expect(record["comparison_rule"] == asm_listing.RULE, "comparison rule differs from asm_listing.RULE")
        self.expect(record["anchor"] == ANCHOR.identity(), "asm anchor baseline digest differs from the pinned one")
        package = repository_files.package_directory(ROOT, "gf2-kernels-simd")
        sources = [path for path in changed if path.startswith(f"{package}/src/") and path.endswith(".rs")]
        self.expect([m["source"] for m in record["modules"]] == sources, "modules are not the changed kernel sources")
        for module in record["modules"]:
            artefact = module["artefact"]
            current = self.end_bytes(artefact)
            self.expect(current is not None, f"{artefact}: no blob holds the end-state digest")
            if current is not None:
                rows = asm_listing.compare(ANCHOR.bytes(artefact).decode(), current.decode())
                self.expect(rows == module["symbols"], f"{artefact}: symbol rows do not re-derive")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--drift", type=pathlib.Path, default=HERE / "production-drift.json")
    parser.add_argument("--assembly", type=pathlib.Path, default=HERE / "asm-comparison.json")
    arguments = parser.parse_args()
    directories = [repository_files.package_directory(ROOT, name) for name in PACKAGES]
    verifier = Verifier()
    changed = verifier.drift(json.loads(arguments.drift.read_bytes()), directories)
    verifier.assembly(json.loads(arguments.assembly.read_bytes()), changed, directories)
    if verifier.faults:
        raise SystemExit("records do not verify:\n" + "\n".join(verifier.faults))
    print("production-drift.json and asm-comparison.json verify against the pinned end state")


if __name__ == "__main__":
    main()
