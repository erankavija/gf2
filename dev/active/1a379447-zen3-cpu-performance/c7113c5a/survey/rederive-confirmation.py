#!/usr/bin/env python3
"""Re-derive committed confirmations from their own derivation records.

Usage: rederive-confirmation.py <fixture-dir> <record> ..., from the repository root

A derivation record names its pilot receipt, its addendum and every option the
freeze took, and the addendum carries the restated prose. Each record is frozen
again by `freeze-confirmation.py` in a fixture tree holding only the files the
freeze reads; the addendum and the record must equal the committed bytes. The
command writes nothing outside the fixture and exits non-zero on a difference.
"""

import json
import os
import re
import shutil
import subprocess
import sys

FREEZER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "freeze-confirmation.py")

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
    if not receipt or not output or not os.path.exists(output):
        return None, "not a freezer record with a committed addendum"
    pilot = os.path.dirname(receipt)
    addendum = json.load(open(output))
    pilot_addendum = json.load(open(receipt))["addendum"]["path"]
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
    rule = re.search(r"^frozen rule {18}(\S+)$", text, re.M)
    if rule:
        args += ["--frozen-rule", rule.group(1)]
        inputs.append(rule.group(1))
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
        missing = [path for path in inputs if not os.path.exists(path)]
        if missing:
            print(f"differs {record}: missing {missing}")
            failed = True
            continue
        for path in inputs + [output, record]:
            os.makedirs(os.path.join(root, os.path.dirname(path)), exist_ok=True)
        for path in inputs:
            shutil.copy(path, os.path.join(root, path))
        run = subprocess.run(
            [sys.executable, "-B", FREEZER, *args], cwd=root, capture_output=True, text=True
        )
        if run.returncode:
            print(f"differs {record}: freezer refused: {run.stderr.strip()}")
            failed = True
            continue
        same = all(
            open(path, "rb").read() == open(os.path.join(root, path), "rb").read()
            for path in (output, record)
        )
        print(f"{'same' if same else 'differs'} {record}")
        failed |= not same
    raise SystemExit(1 if failed else 0)


if __name__ == "__main__":
    main()
