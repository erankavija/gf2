#!/usr/bin/env python3
"""Freeze a c7113c5a confirmation addendum from its accepted v3 pilot.

Usage:
  freeze-confirmation.py --pilot-addendum <json> --pilot <receipt-dir>
                         --frozen-utc <YYYY-MM-DDTHH:MM:SSZ>
                         --output <addendum> --record <derivation.txt>
                         [--cell <cell-id> ...] [--selection-rationale <text>]
                         [--resolution <float> --resolution-derivation <file>]

The pilot's acceptance summary holds every cell's percentile interval at the
pilot's ledger-derived corrected alpha, the value protocol P-03 recomputes
from the raw pairs. The widest relative half-width,
max(|s - l|, |u - s|) / s over the pilot's cells, rounded up to two decimals,
becomes the frozen measurement resolution; the pilot receipt is pinned by path
and SHA-256 as its evidence. Every retained cell keeps its pilot declaration
with the confirmatory role. A margin the pilot's resolution invalidates may be
replaced, with its new rationale, through the margin options, and a margin the
confirmation keeps may restate its rationale alone; the record names every
replaced value and every restated rationale. `--family-description` restates
the family prose the confirmation stage carries. The script refuses to write an
addendum whose margins do not strictly exceed one plus that resolution, and it
writes the derivation record beside the addendum so the frozen number is
reproducible from committed bytes.

By default every pilot cell becomes confirmatory. `--cell` selects a subset,
for a family whose pilot carries more cells than P-20's tail-support bound
admits as Bonferroni comparisons; each selection needs a `--selection-rationale`
the record carries beside the retained and dropped identifiers. The resolution
is always derived from the whole pilot, retained cells and dropped alike, so
narrowing the confirmation never narrows the evidence that sizes it.

A family whose rule sizes the resolution from something this script cannot
observe — the pilot's intervals recomputed at the confirmation's own corrected
alpha, or P-20's endpoint shift between seed streams — supplies that value with
`--resolution` and the derivation that produced it with
`--resolution-derivation`. The supplied value must be at least the pilot-alpha
width computed here, which stays the floor, and the record carries the
derivation verbatim so the frozen number remains reproducible from committed
bytes.
"""

import argparse
import copy
import hashlib
import json
import math


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pilot-addendum", required=True)
    parser.add_argument("--pilot", required=True)
    parser.add_argument("--frozen-utc", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--record", required=True)
    parser.add_argument("--equivalence-margin", type=float)
    parser.add_argument("--equivalence-rationale")
    parser.add_argument("--material-gap-threshold", type=float)
    parser.add_argument("--material-gap-rationale")
    parser.add_argument("--cell", action="append", default=[], dest="cells")
    parser.add_argument("--selection-rationale")
    parser.add_argument("--family-description")
    parser.add_argument("--resolution", type=float)
    parser.add_argument("--resolution-derivation")
    args = parser.parse_args()
    if bool(args.cells) != bool(args.selection_rationale):
        raise SystemExit("a cell selection needs both --cell and --selection-rationale")
    if (args.resolution is None) != (args.resolution_derivation is None):
        raise SystemExit("a supplied resolution needs both a value and its derivation")
    replacements = {
        "equivalence_margin": (args.equivalence_margin, "equivalence_rationale", args.equivalence_rationale),
        "material_gap_threshold": (args.material_gap_threshold, "material_gap_rationale", args.material_gap_rationale),
    }
    for name, (value, rationale_field, rationale) in replacements.items():
        if value is not None and rationale is None:
            raise SystemExit(f"replacing {name} needs a rationale")

    receipt_path = f"{args.pilot.rstrip('/')}/receipt.json"
    with open(receipt_path, "rb") as handle:
        receipt_bytes = handle.read()
    receipt_sha = hashlib.sha256(receipt_bytes).hexdigest()
    with open(f"{args.pilot.rstrip('/')}/acceptance-summary.json") as handle:
        summary = json.load(handle)
    with open(args.pilot_addendum) as handle:
        addendum = json.load(handle)
    receipt = json.loads(receipt_bytes)

    if summary["receipt_sha256"] != receipt_sha:
        raise SystemExit("acceptance summary evaluates a different receipt")
    if summary["label"] != "pilot" or summary["verdict"] != "accepted":
        raise SystemExit("resolution evidence must be an accepted pilot")
    if receipt["family_id"] != addendum["family"]["id"]:
        raise SystemExit("pilot belongs to another family")

    lines = [
        f"pilot receipt  {receipt_path}",
        f"receipt sha256 {receipt_sha}",
        f"campaign       {summary['campaign_id']}",
        f"verdict        {summary['verdict']} (label {summary['label']})",
        f"family         {summary['family']['family_id']}: {summary['family']['comparisons']} ledger comparisons, "
        f"attempt alpha {summary['family']['family_alpha']}, per-comparison confidence "
        f"{summary['family']['per_comparison_confidence']}",
        "",
        f"{'cell':<36}{'pairs':>6}{'estimate':>12}{'lower':>12}{'upper':>12}{'rel_half':>10}",
    ]
    widest = 0.0
    for cell in summary["cells"]:
        interval = cell["interval"]
        estimate = interval["estimate"]
        half = max(abs(estimate - interval["lower"]), abs(interval["upper"] - estimate)) / estimate
        widest = max(widest, half)
        lines.append(f"{cell['cell_id']:<36}{cell['pairs']:>6}{estimate:>12.5g}{interval['lower']:>12.5g}"
                     f"{interval['upper']:>12.5g}{half:>10.4f}")
    resolution = math.ceil(widest * 100.0) / 100.0
    lines += ["", f"widest relative half-width   {widest:.6f}"]
    if args.resolution is not None:
        if args.resolution < resolution:
            raise SystemExit(
                f"supplied resolution {args.resolution} is below the pilot-alpha floor {resolution}")
        with open(args.resolution_derivation) as handle:
            derivation = handle.read().rstrip("\n")
        lines += [f"pilot-alpha floor            {resolution:.2f}",
                  f"supplied resolution          {args.resolution:.2f}",
                  f"derivation                   {args.resolution_derivation}", "", derivation, ""]
        resolution = args.resolution
    lines.append(f"frozen measurement resolution {resolution:.2f}")

    frozen = copy.deepcopy(addendum)
    effect = frozen["effect"]
    for name, (value, rationale_field, rationale) in replacements.items():
        if rationale is None:
            continue
        if value is None:
            lines.append(f"restated {name} rationale; value {effect[name]} unchanged")
        else:
            lines.append(f"replaced {name} {effect[name]} -> {value}")
            effect[name] = value
        effect[rationale_field] = rationale
    if args.family_description:
        frozen["family"]["description"] = args.family_description
        lines.append("restated the family description for the confirmatory stage")
    for name in ("worthwhile_speedup", "equivalence_margin", "material_gap_threshold"):
        if effect[name] is not None and not effect[name] > 1.0 + resolution:
            raise SystemExit(f"{name} {effect[name]} does not exceed 1 + {resolution}")
    frozen["frozen"]["frozen_utc"] = args.frozen_utc
    frozen["effect"]["measurement_resolution"] = resolution
    frozen["effect"]["resolution_evidence"] = {"receipt": receipt_path, "sha256": receipt_sha}
    if args.cells:
        declared = [cell["cell_id"] for cell in frozen["cells"]]
        unknown = [cell_id for cell_id in args.cells if cell_id not in declared]
        if unknown:
            raise SystemExit(f"selected cells are not in the pilot addendum: {', '.join(unknown)}")
        selected = set(args.cells)
        dropped = [cell_id for cell_id in declared if cell_id not in selected]
        frozen["cells"] = [cell for cell in frozen["cells"] if cell["cell_id"] in selected]
        lines += [
            "",
            f"selection      {len(frozen['cells'])} of {len(declared)} pilot cells become confirmatory",
            f"rationale      {args.selection_rationale}",
        ]
        lines += [f"retained       {cell['cell_id']}" for cell in frozen["cells"]]
        lines += [f"dropped        {cell_id}" for cell_id in dropped]
    for cell in frozen["cells"]:
        cell["role"] = "confirmatory"
    with open(args.output, "w") as handle:
        json.dump(frozen, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    lines.append(f"margins: equivalence {effect['equivalence_margin']}, material gap {effect['material_gap_threshold']}")
    lines.append(f"confirmation addendum {args.output}")
    with open(args.record, "w") as handle:
        handle.write("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
