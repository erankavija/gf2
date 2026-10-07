#!/usr/bin/env python3
"""Re-derive committed confirmations from their own derivation records.

Usage: rederive-confirmation.py <fixture-dir> <record> ..., from the repository root

A derivation record names its pilot receipt, its addendum and every option the
freeze took, and the addendum carries the restated prose. Each record is frozen
again by `freeze-confirmation.py` in a fixture tree holding only the files the
freeze reads; the addendum and the record must equal the committed bytes. The
command writes nothing outside the fixture and exits non-zero on a difference.

Every input is taken by content identity from the committed files, never from
the path a record names, so a relocated file is found and a different file at
a recorded path is not read:

- the pilot receipt and a holdout declaration by the SHA-256 the record
  states, and the pilot addendum by the SHA-256 the receipt pins;
- the acceptance summary beside a file holding the receipt's bytes, accepted
  when it names the receipt's digest;
- a supplied derivation from the record, which carries it verbatim;
- the committed addendum among the JSON files that pin the receipt's digest
  as resolution evidence, accepted when the freeze reproduces its bytes.

An input no committed file holds is a refusal naming its digest. The fixture
keeps the recorded layout, so the frozen bytes name the recorded paths.
"""

import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FREEZER = os.path.join(HERE, "freeze-confirmation.py")
ROOT = subprocess.run(["git", "-C", HERE, "rev-parse", "--show-toplevel"],
                      capture_output=True, check=True, text=True).stdout.strip()

RATIONALE = {"worthwhile_speedup": ("--worthwhile-speedup", "--worthwhile-rationale", "rationale"),
             "equivalence_margin": ("--equivalence-margin", "--equivalence-rationale", "equivalence_rationale"),
             "material_gap_threshold": ("--material-gap-threshold", "--material-gap-rationale", "material_gap_rationale")}


class Refusal(Exception):
    """A record whose inputs the committed files do not hold."""


def _git(*arguments, stdin=None):
    return subprocess.run(["git", "-C", ROOT, *arguments], input=stdin, capture_output=True,
                          check=True).stdout


class Committed:
    """The files of the index, addressed by the SHA-256 of their bytes."""

    def __init__(self):
        self.paths = {}
        for line in _git("ls-files", "-s", "-z").decode().split("\0"):
            if line:
                self.paths.setdefault(line.split()[1], []).append(line.split("\t", 1)[1])
        self.blobs = {}
        stream = _git("cat-file", "--batch", stdin="\n".join(self.paths).encode() + b"\n")
        at = 0
        while at < len(stream):
            end = stream.index(b"\n", at)
            blob, _, size = stream[at:end].split()
            content = stream[end + 1:end + 1 + int(size)]
            self.blobs[hashlib.sha256(content).hexdigest()] = (blob.decode(), content)
            at = end + 2 + int(size)

    def content(self, sha256, what):
        if sha256 not in self.blobs:
            raise Refusal(f"no committed file has sha256 {sha256} ({what})")
        return self.blobs[sha256][1]

    def summary(self, receipt_sha256):
        """The acceptance summary beside the receipt, which names the receipt's digest."""
        self.content(receipt_sha256, "pilot receipt")
        for receipt in sorted(self.paths[self.blobs[receipt_sha256][0]]):
            beside = os.path.join(os.path.dirname(receipt), "acceptance-summary.json")
            try:
                content = _git("show", f":{beside}")
            except subprocess.CalledProcessError:
                continue
            if json.loads(content).get("receipt_sha256") == receipt_sha256:
                return content
        raise Refusal(f"no acceptance summary beside a receipt with sha256 {receipt_sha256}")

    def addenda(self, receipt_sha256):
        """Contents of the JSON files that pin the receipt as resolution evidence."""
        listing = subprocess.run(
            ["git", "-C", ROOT, "grep", "--cached", "-l", "-z", "-F", "-e", receipt_sha256, "--",
             "*.json"], capture_output=True).stdout.decode()
        found = {}
        for path in filter(None, listing.split("\0")):
            content = _git("show", f":{path}")
            try:
                evidence = json.loads(content)["effect"]["resolution_evidence"]["sha256"]
            except (KeyError, TypeError, ValueError):
                continue
            if evidence == receipt_sha256:
                found.setdefault(content, path)
        return found


def plan(text, addendum_bytes, receipt):
    """The freezer's options for the record `text` and the files beside them."""
    def line(prefix):
        found = re.search(rf"^{prefix}\s+(.*)$", text, re.M)
        return found.group(1) if found else None
    addendum = json.loads(addendum_bytes)
    files = {}
    args = ["--pilot-addendum", receipt["addendum"]["path"],
            "--frozen-utc", addendum["frozen"]["frozen_utc"]]
    frozen = re.search(r"^frozen measurement resolution \d+\.(\d+)$", text, re.M)
    if not frozen:
        raise Refusal("no frozen resolution line")
    if len(frozen.group(1)) != 2:
        args += ["--resolution-decimals", str(len(frozen.group(1)))]
    if line("supplied resolution"):
        derivation = line("derivation")
        args += ["--resolution", line("supplied resolution"), "--resolution-derivation", derivation]
        opening = text.index(f"{derivation}\n\n") + len(derivation) + 2
        files[derivation] = text[opening:text.rindex("\n\nfrozen measurement resolution ")].encode() + b"\n"
    for name, (value_flag, rationale_flag, field) in RATIONALE.items():
        replaced = re.search(rf"^replaced {name} \S+ -> (\S+)$", text, re.M)
        if replaced:
            args += [value_flag, replaced.group(1), rationale_flag, addendum["effect"][field]]
        elif re.search(rf"^restated {name} rationale", text, re.M):
            args += [rationale_flag, addendum["effect"][field]]
    if "restated the family description" in text:
        args += ["--family-description", addendum["family"]["description"]]
    if line("rationale     "):
        args += ["--selection-rationale", re.search(r"^rationale      (.*)$", text, re.M).group(1)]
        for cell in re.findall(r"^retained       (\S+)$", text, re.M):
            args += ["--cell", cell]
    if line("restated purpose"):
        args += ["--purpose", addendum["family"]["purpose"]]
    if line("holdout declaration"):
        args += ["--holdout-cells", line("holdout declaration")]
        files[line("holdout declaration")] = line("declaration sha256")
    return args, files


def rederive(committed, fixture, record):
    """Freezes `record` again; returns whether a committed addendum and the record reproduce."""
    with open(record) as handle:
        text = handle.read()
    def line(prefix):
        found = re.search(rf"^{prefix}\s+(.*)$", text, re.M)
        return found.group(1) if found else None
    receipt_path, receipt_sha, output = line("pilot receipt"), line("receipt sha256"), line("confirmation addendum")
    if not receipt_path or not receipt_sha or not output:
        raise Refusal("not a freezer record")
    receipt_bytes = committed.content(receipt_sha, "pilot receipt")
    receipt = json.loads(receipt_bytes)
    pilot = os.path.dirname(receipt_path)
    candidates = committed.addenda(receipt_sha)
    if not candidates:
        raise Refusal(f"no committed addendum pins the pilot receipt with sha256 {receipt_sha}")
    refusals = []
    for number, addendum_bytes in enumerate(candidates):
        args, files = plan(text, addendum_bytes, receipt)
        files = {path: committed.content(source, "holdout declaration") if isinstance(source, str)
                 else source for path, source in files.items()}
        files[receipt_path] = receipt_bytes
        files[f"{pilot}/acceptance-summary.json"] = committed.summary(receipt_sha)
        files[receipt["addendum"]["path"]] = committed.content(
            receipt["addendum"]["sha256"], "pilot addendum")
        root = os.path.join(fixture, re.sub(r"\W", "_", record), str(number))
        for path, content in files.items():
            os.makedirs(os.path.join(root, os.path.dirname(path)), exist_ok=True)
            with open(os.path.join(root, path), "wb") as handle:
                handle.write(content)
        for path in (output, "record.txt"):
            os.makedirs(os.path.join(root, os.path.dirname(path)), exist_ok=True)
        run = subprocess.run(
            [sys.executable, "-B", FREEZER, *args, "--pilot", pilot, "--output", output,
             "--record", "record.txt"], cwd=root, capture_output=True, text=True)
        if run.returncode:
            refusals.append(run.stderr.strip())
            continue
        with open(os.path.join(root, output), "rb") as frozen, \
                open(os.path.join(root, "record.txt")) as written:
            if frozen.read() == addendum_bytes and written.read() == text:
                return True
    if refusals and len(refusals) == len(candidates):
        raise Refusal(f"freezer refused: {refusals[0]}")
    return False


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    fixture, *records = sys.argv[1:]
    committed = Committed()
    failed = False
    for record in records:
        try:
            same, why = rederive(committed, fixture, record), ""
        except Refusal as refusal:
            same, why = False, f": {refusal}"
        except (OSError, KeyError, ValueError) as error:
            same, why = False, f": unreadable: {error!r}"
        print(f"{'same' if same else 'differs'} {record}{why}")
        failed |= not same
    raise SystemExit(1 if failed else 0)


if __name__ == "__main__":
    main()
