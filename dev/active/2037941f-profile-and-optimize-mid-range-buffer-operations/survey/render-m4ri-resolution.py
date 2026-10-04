#!/usr/bin/env python3
"""Render the frozen M4RI comparator resolution rule from its pilot.

The record states the outcome the rule fixes: `eligible` when the pilot-derived
resolution is at or below the family ceiling, `resolution-insufficient` above
it. The standard output line carries the same two values for the freeze script.
"""

import hashlib
import json
import pathlib
import re
import sys

from resolution_rule import ROOT, accepted_pilot, ceiling as family_ceiling, half_widths, rounded

STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
PILOT = pathlib.Path("dev/bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot")
ADDENDUM = STORY / "campaigns/dense-matvec-vs-m4ri.json"
RULES = STORY / "dense-parity-addendum.md"
SUPPORT = pathlib.Path("dev/tools/tuning-campaign-support/src")
# The evaluator's P-20 tail-support test and the ledger's sequential attempt
# budget, as the sources state them; the arithmetic below follows both.
TAIL_SUPPORT = re.compile(r"bootstrap_resamples\) \* corrected_alpha / 2\.0 < (\d+)\.0")
ATTEMPT_ALPHA = "family.family_wise.alpha / (attempts * (attempts + 1.0))"
CONFIRMATORY_ROW = re.compile(r"\| `(m4ri-gap-[^`]+)` \|.*\| confirmatory \|")


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: render-m4ri-resolution.py SURVEY_ANALYSIS OUTPUT")
    receipt, receipt_record = accepted_pilot(PILOT)
    addendum = json.loads((ROOT / ADDENDUM).read_bytes())
    rules = (ROOT / RULES).read_text()
    ids = [match.group(1) for match in map(CONFIRMATORY_ROW.fullmatch, rules.splitlines()) if match]
    measured = {cell["cell_id"]: len(cell["pairs"]) for cell in receipt_record["cells"]}
    if not ids or any(cell_id not in measured for cell_id in ids):
        raise ValueError("the pilot lacks a confirmatory cell of the frozen addendum")

    tail = TAIL_SUPPORT.search((ROOT / SUPPORT / "receipt.rs").read_text())
    if not tail or ATTEMPT_ALPHA not in (ROOT / SUPPORT / "trial_ledger.rs").read_text():
        raise ValueError("the P-20 tail-support test or the attempt budget has changed")
    minimum_draws = int(tail.group(1))
    ledger = ROOT / addendum["family_wise"]["ledger_path"]
    entries = [json.loads(line) for line in ledger.read_text().splitlines()]
    # The chain the confirmation reserves on ends at the pilot's own line, so a
    # later reservation leaves this record unchanged.
    campaigns = [entry["campaign"] for entry in entries]
    entries = entries[: campaigns.index(receipt_record["campaign_id"]) + 1]
    attempt = sum(entry["comparisons"] > 0 for entry in entries) + 1
    reserved = sum(entry["comparisons"] for entry in entries)
    attempt_alpha = addendum["family_wise"]["alpha"] / (attempt * (attempt + 1))
    resamples = receipt_record["settings"]["bootstrap_resamples"]
    capacity = int(resamples * attempt_alpha / (2 * minimum_draws)) - reserved
    alpha = attempt_alpha / (reserved + len(ids))

    widths = half_widths(sys.argv[1], PILOT, alpha)
    if any(cell_id not in widths for cell_id in ids):
        raise ValueError("canonical resolution output lacks a confirmatory cell")
    maximum = max(widths[cell_id] for cell_id in ids)
    resolution = rounded(maximum)
    ceiling = family_ceiling(RULES, "M4RI comparator")
    if len(ids) > capacity:
        raise ValueError("P-20 admits fewer comparisons than the family's confirmatory cells")
    eligible = resolution <= ceiling
    outcome = "eligible" if eligible else "resolution-insufficient"
    consequence = (
        "The pilot resolves the family within its frozen ceiling. The canonical freezer "
        "pins this resolution in the confirmation addendum."
        if eligible
        else "The frozen family rule ends this comparison after its exploratory pilot. "
        "No confirmatory reservation is made."
    )
    rows = "\n".join(f"| `{cell_id}` | {measured[cell_id]} | {widths[cell_id]:.6f} |" for cell_id in ids)
    (ROOT / sys.argv[2]).write_text(
        "# M4RI comparator pilot resolution\n\n"
        "> **Diátaxis Type:** Reference\n\n"
        f"Source: `{PILOT}/receipt.json` (SHA-256 `{hashlib.sha256(receipt).hexdigest()}`); "
        f"frozen rule: `{RULES}`.\n\n"
        "The canonical `survey-analysis resolution` command recomputes whole-pair "
        "bootstrap intervals at the first confirmation's corrected alpha.\n\n"
        f"Command: `survey-analysis resolution {PILOT} {alpha:g}`\n\n"
        f"Ledger `{addendum['family_wise']['ledger_path']}` through the pilot: "
        f"{len(entries)} reservation(s), "
        f"{reserved} reserved comparison(s), so the confirmation is attempt $t={attempt}$ with "
        f"$\\alpha_t={attempt_alpha:g}$ (`{SUPPORT / 'trial_ledger.rs'}`). P-20 requires "
        f"{minimum_draws} expected bootstrap draws per tail (`{SUPPORT / 'receipt.rs'}`), which "
        f"admits {capacity} further comparison(s) at {resamples} resamples. The family's "
        f"$m={len(ids)}$ confirmatory cells give $\\alpha_c={alpha:g}$ and "
        f"{resamples * alpha / 2:g} expected draws per tail.\n\n"
        "| Confirmatory cell | Pilot pairs | Relative half-width |\n|---|---:|---:|\n"
        f"{rows}\n\n"
        f"Largest confirmatory width: {maximum:.6f}; frozen upward rounding to 0.001: "
        f"$r={resolution:.3f}$. Family ceiling: {ceiling:.3f}.\n\n"
        f"**Outcome: `{outcome}`.** {consequence}\n",
        encoding="utf-8",
    )
    print(f"resolution={resolution:.3f} outcome={outcome}")


if __name__ == "__main__":
    main()
