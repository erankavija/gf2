#!/usr/bin/env python3
"""Select this family's confirmatory cells and invoke the canonical freezer."""

import json
import math
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ACTIVE = Path("dev/active/00dd43c3")
RESULTS = Path("dev/bench_results/00dd43c3")
PILOT = RESULTS / "v4-r1-pilot"
LEDGER = RESULTS / "residual-shift-family-ledger.jsonl"
ADDENDUM = ACTIVE / "pilot-addendum.json"
FREEZER = Path("dev/active/c7113c5a/survey/freeze-confirmation.py")
RECEIPT_SOURCE = Path("dev/tools/tuning-campaign-support/src/receipt.rs")
TRIAL_LEDGER_SOURCE = Path("dev/tools/tuning-campaign-support/src/trial_ledger.rs")


def committed(path):
    subprocess.run(["git", "ls-files", "--error-unmatch", str(path)],
                   check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--", str(path)], check=True)


def tail_requirement():
    source = RECEIPT_SOURCE.read_text()
    match = re.search(r"bootstrap_resamples\) \* corrected_alpha / 2\.0 < ([0-9]+)\.0", source)
    if not match:
        raise SystemExit("cannot locate the P-20 tail-support condition in receipt.rs")
    return int(match.group(1))


def priority(cell):
    cell_id = cell["cell_id"]
    shapes = ("resident", "streaming", "byte-residual", "lane-crossing")
    return (next(index for index, shape in enumerate(shapes) if shape in cell_id),
            0 if cell_id.startswith("left-") else 1)


def main():
    for path in (PILOT / "receipt.json", PILOT / "acceptance-summary.json", ADDENDUM,
                 LEDGER, FREEZER, RECEIPT_SOURCE, TRIAL_LEDGER_SOURCE):
        committed(path)
    receipt = json.loads((PILOT / "receipt.json").read_text())
    summary = json.loads((PILOT / "acceptance-summary.json").read_text())
    addendum = json.loads(ADDENDUM.read_text())
    if summary["verdict"] != "accepted" or summary["label"] != "pilot":
        raise SystemExit("confirmation needs an accepted committed pilot")
    entries = [json.loads(line) for line in LEDGER.read_text().splitlines() if line]
    if not entries or any(entry["family"] != addendum["family"]["id"] for entry in entries):
        raise SystemExit("the family ledger has no matching pilot reservation")
    if any(entry["comparisons"] > 0 for entry in entries):
        raise SystemExit("the family's one-candidate confirmation attempt is already reserved")

    # trial_ledger::attempt_alpha counts non-exploratory reservations, including
    # the reservation this freeze prepares; receipt.rs divides by cumulative
    # comparisons and requires tail support for each endpoint.
    attempt = max(1, 1 + sum(entry["comparisons"] > 0 for entry in entries))
    family_alpha = addendum["family_wise"]["alpha"]
    attempt_alpha = family_alpha / (attempt * (attempt + 1))
    resamples = receipt["settings"]["bootstrap_resamples"]
    minimum = tail_requirement()
    previous = sum(entry["comparisons"] for entry in entries)
    capacity = max(0, math.floor(resamples * attempt_alpha / (2 * minimum)) - previous)
    cells = sorted(addendum["cells"], key=priority)
    retained = cells[:capacity]
    dropped = cells[capacity:]
    arithmetic = (
        f"P-20: {resamples} resamples * corrected_alpha / 2 >= {minimum}; "
        f"trial_ledger.rs attempt {attempt} alpha = {family_alpha} / "
        f"({attempt} * {attempt + 1}) = {attempt_alpha}; prior comparisons = {previous}; "
        f"capacity = floor({resamples} * {attempt_alpha} / (2 * {minimum})) "
        f"- {previous} = {capacity}. The pilot addendum's frozen priority keeps "
        "paired resident, streaming and byte-residual cells before the "
        "lane-crossing boundary. Retained: "
        + ", ".join(cell["cell_id"] for cell in retained)
        + ". Dropped: "
        + (", ".join(cell["cell_id"] for cell in dropped) or "none")
        + "."
    )
    if not retained:
        (ACTIVE / "no-confirmatory-budget.txt").write_text(arithmetic + "\n")
        print("no confirmatory cell fits the family ledger; arithmetic recorded")
        return

    command = [
        "python3", "-B", str(FREEZER), "--pilot-addendum", str(ADDENDUM),
        "--pilot", str(PILOT),
        "--frozen-utc", datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "--output", str(ACTIVE / "confirmation-addendum.json"),
        "--record", str(ACTIVE / "confirmation-derivation.txt"),
    ]
    for cell in retained:
        command += ["--cell", cell["cell_id"]]
    command += [
        "--selection-rationale", arithmetic,
        "--family-description",
        "Confirmatory stage of issue 00dd43c3. Both arms are the same shipped "
        "BitVec shift executable at each retained residual case: the baseline "
        "holds the scalar funnel through the test-build switch and the candidate "
        "releases it for the BMI2-gated route. The committed pilot receipt supplies "
        "fresh-sample resolution, and the adjacent derivation record names every "
        "retained and dropped cell under the pilot's frozen priority. The frozen "
        "retention rule remains the pilot's: acceptance must verify this campaign, "
        "no cell may record fail or not-confirmatory, and at least one cell in "
        "each direction must record pass; otherwise the gated route is removed."
    ]
    subprocess.run(command, check=True)


if __name__ == "__main__":
    main()
