#!/usr/bin/env python3
"""Re-derive committed confirmations from their own derivation records.

Usage: rederive-confirmation.py <fixture-dir> <record> ..., from the repository root

A derivation record names its pilot receipt, its addendum and every option the
freeze took, and the addendum carries the restated prose. Each record is frozen
again by `freeze-confirmation.py` in a fixture tree holding only the files the
freeze reads; the addendum and the record must equal the committed bytes. The
command writes nothing outside the fixture and exits non-zero on a difference.

A record names its files by the paths its freeze recorded. A recorded path that
no longer exists is read from the one live file of that name whose directories
match the recorded ones furthest back from the name; the fixture keeps the
recorded layout, so the frozen bytes name the recorded paths.
"""

import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FREEZER = os.path.join(HERE, "freeze-confirmation.py")
ROOT = subprocess.run(["git", "-C", HERE, "rev-parse", "--show-toplevel"],
                      capture_output=True, check=True, text=True).stdout.strip()


def live(recorded):
    """The existing path holding the file a record names as `recorded`."""
    if os.path.exists(recorded):
        return recorded
    listing = subprocess.run(
        ["git", "-C", ROOT, "ls-files", "--", f":(glob)**/{os.path.basename(recorded)}"],
        capture_output=True, check=True, text=True).stdout.splitlines()
    wanted = recorded.split("/")[::-1]

    def shared(path):
        return len(os.path.commonprefix([wanted, path.split("/")[::-1]]))

    # Receipt input snapshots hold byte copies under an `inputs` directory.
    ranked = sorted((shared(path) for path in listing if "inputs" not in path.split("/")[:-1]),
                    reverse=True)
    if not ranked or ranked[1:2] == ranked[:1]:
        raise OSError(f"no single live file for {recorded}")
    return next(path for path in listing
                if "inputs" not in path.split("/")[:-1] and shared(path) == ranked[0])


RATIONALE = {"worthwhile_speedup": ("--worthwhile-speedup", "--worthwhile-rationale", "rationale"),
             "equivalence_margin": ("--equivalence-margin", "--equivalence-rationale", "equivalence_rationale"),
             "material_gap_threshold": ("--material-gap-threshold", "--material-gap-rationale", "material_gap_rationale")}

def arguments(record_path):
    text = open(record_path).read()
    def line(prefix):
        found = re.search(rf"^{prefix}\s+(.*)$", text, re.M)
        return found.group(1) if found else None
    receipt = line("pilot receipt")
    output = line("confirmation addendum")
    if not receipt or not output:
        return None, "not a freezer record with a committed addendum"
    pilot = os.path.dirname(receipt)
    addendum = json.load(open(live(output)))
    pilot_addendum = json.load(open(live(receipt)))["addendum"]["path"]
    inputs = [receipt, f"{pilot}/acceptance-summary.json", pilot_addendum]
    args = ["--pilot-addendum", pilot_addendum, "--pilot", pilot,
            "--frozen-utc", addendum["frozen"]["frozen_utc"], "--output", output, "--record", record_path]
    frozen = re.search(r"^frozen measurement resolution \d+\.(\d+)$", text, re.M)
    if not frozen:
        return None, "no frozen resolution line"
    if len(frozen.group(1)) != 2:
        args += ["--resolution-decimals", str(len(frozen.group(1)))]
    if line("supplied resolution"):
        derivation = line("derivation")
        args += ["--resolution", line("supplied resolution"), "--resolution-derivation", derivation]
        inputs.append(derivation)
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
        inputs.append(line("holdout declaration"))
    return (args, inputs, output), None

def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    fixture, *records = sys.argv[1:]
    failed = False
    for record in records:
        try:
            plan, why = arguments(record)
        except (OSError, KeyError, ValueError) as error:
            plan, why = None, f"unreadable: {error!r}"
        if plan is None:
            print(f"differs {record}: {why}")
            failed = True
            continue
        args, inputs, output = plan
        root = os.path.join(fixture, re.sub(r"\W", "_", record))
        try:
            sources = {path: live(path) for path in inputs + [output]}
        except OSError as error:
            print(f"differs {record}: {error}")
            failed = True
            continue
        for path in inputs + [output, record]:
            os.makedirs(os.path.join(root, os.path.dirname(path)), exist_ok=True)
        for path in inputs:
            shutil.copy(sources[path], os.path.join(root, path))
        run = subprocess.run(
            [sys.executable, "-B", FREEZER, *args], cwd=root, capture_output=True, text=True
        )
        if run.returncode:
            print(f"differs {record}: freezer refused: {run.stderr.strip()}")
            failed = True
            continue
        same = all(
            open(committed, "rb").read() == open(os.path.join(root, path), "rb").read()
            for committed, path in ((sources[output], output), (record, record))
        )
        print(f"{'same' if same else 'differs'} {record}")
        failed |= not same
    raise SystemExit(1 if failed else 0)


if __name__ == "__main__":
    main()
