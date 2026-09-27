#!/usr/bin/env python3
"""Render the frozen ISA-L confirmation stopping rule from its pilot."""

import hashlib
import json
import math
import pathlib
import re
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[4]
STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
PILOT = pathlib.Path("dev/bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot")
ADDENDUM = STORY / "campaigns/logical-isal-base-gap.json"
RULES = STORY / "logical-buffer-addendum.md"


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: render-isal-outcome.py SURVEY_ANALYSIS OUTPUT")
    receipt = (ROOT / PILOT / "receipt.json").read_bytes()
    receipt_record = json.loads(receipt)
    summary = json.loads((ROOT / PILOT / "acceptance-summary.json").read_bytes())
    addendum = json.loads((ROOT / ADDENDUM).read_bytes())
    if (
        summary["receipt_sha256"] != hashlib.sha256(receipt).hexdigest()
        or summary["verdict"] != "accepted"
        or summary["label"] != "pilot"
    ):
        raise ValueError("ISA-L pilot is not an accepted, digest-matched pilot")
    primary = addendum["cells"][:5]
    if len(primary) != 5 or any(cell["cache_state"] != "warm" for cell in primary):
        raise ValueError("the frozen primary ISA-L cells have changed")
    # This is the first confirmation and its five primary cells spend m = 5.
    alpha = addendum["family_wise"]["alpha"] / (1 * 2 * len(primary))
    command = [sys.argv[1], "resolution", str(PILOT), format(alpha, ".12g")]
    raw = subprocess.check_output(command, cwd=ROOT, text=True)
    widths = {}
    for line in raw.splitlines():
        match = re.fullmatch(r"stricter alpha\s+\S+\s+(\S+)\s+([0-9.]+)", line)
        if match:
            widths[match.group(1)] = float(match.group(2))
    ids = [cell["cell_id"] for cell in primary]
    if any(cell_id not in widths for cell_id in ids):
        raise ValueError("canonical resolution output lacks a primary cell")
    maximum = max(widths[cell_id] for cell_id in ids)
    resolution = math.ceil(maximum * 1000) / 1000
    rules = (ROOT / RULES).read_text()
    family_row = next(line for line in rules.splitlines() if line.startswith("| ISA-L scalar gap |"))
    columns = [column.strip() for column in family_row.split("|")]
    ceiling = float(columns[4])
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
