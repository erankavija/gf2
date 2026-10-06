#!/usr/bin/env python3
"""Render the frozen ISA-L confirmation stopping rule from its pilot."""

import hashlib
import json
import sys

from repo_artifacts import ROOT, addendum as campaign_addendum, receipt, tracked
from resolution_rule import accepted_pilot, ceiling as family_ceiling, half_widths, rounded

CAMPAIGN = "v4-r1-2037941f-logical-isal-base-gap"
PILOT = receipt(CAMPAIGN)
ADDENDUM = campaign_addendum(CAMPAIGN)
RULES = tracked("logical-buffer-addendum.md")


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: render-isal-outcome.py SURVEY_ANALYSIS OUTPUT")
    receipt, receipt_record = accepted_pilot(PILOT)
    addendum = json.loads((ROOT / ADDENDUM).read_bytes())
    primary = addendum["cells"][:5]
    if len(primary) != 5 or any(cell["cache_state"] != "warm" for cell in primary):
        raise ValueError("the frozen primary ISA-L cells have changed")
    # This is the first confirmation and its five primary cells spend m = 5.
    alpha = addendum["family_wise"]["alpha"] / (1 * 2 * len(primary))
    widths = half_widths(sys.argv[1], PILOT, alpha)
    ids = [cell["cell_id"] for cell in primary]
    if any(cell_id not in widths for cell_id in ids):
        raise ValueError("canonical resolution output lacks a primary cell")
    maximum = max(widths[cell_id] for cell_id in ids)
    resolution = rounded(maximum)
    ceiling = family_ceiling(RULES, "ISA-L scalar gap")
    if resolution <= ceiling:
        raise ValueError("the ISA-L pilot no longer triggers the frozen stop rule")
    rows = "\n".join(f"| `{cell_id}` | {widths[cell_id]:.6f} |" for cell_id in ids)
    output = ROOT / sys.argv[2]
    output.write_text(
        "# ISA-L scalar-gap pilot stopping result\n\n"
        "> **Diátaxis Type:** Reference\n\n"
        f"Source: `{PILOT}/receipt.json` (SHA-256 `{hashlib.sha256(receipt).hexdigest()}`); "
        f"frozen rule: `{RULES}`.\n\n"
        "The canonical `survey-analysis resolution` command recomputes whole-pair "
        "bootstrap intervals at the first confirmation's corrected alpha.\n\n"
        f"Command: `survey-analysis resolution {PILOT} {alpha:.3f}`\n\n"
        f"First attempt: $t=1$, $m={len(primary)}$, $\\alpha_c={alpha:.3f}$; "
        f"bootstrap tail support is "
        f"{receipt_record['settings']['bootstrap_resamples'] * alpha / 2:g} "
        "expected draws per tail.\n\n"
        "| Primary warm cell | Relative half-width |\n|---|---:|\n"
        f"{rows}\n\n"
        f"Largest primary width: {maximum:.6f}; frozen upward rounding to 0.001: "
        f"$r={resolution:.3f}$. Family ceiling: {ceiling:.3f}.\n\n"
        "**Outcome: `resolution-insufficient`.** The frozen family rule ends this "
        "comparison after its exploratory pilot. No confirmatory reservation is made.\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
