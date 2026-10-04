#!/usr/bin/env python3
"""Freeze a confirmation addendum from its accepted pilot.

Usage:
  freeze-confirmation.py --pilot-addendum <json> --pilot <receipt-dir>
                         --frozen-utc <YYYY-MM-DDTHH:MM:SSZ>
                         --output <addendum> --record <derivation.txt>
                         [--cell <cell-id> ...] [--selection-rationale <text>]
                         [--resolution <float> --resolution-derivation <file>]
                         [--resolution-decimals <n>]
                         [--holdout-cells <declaration.json>] [--purpose <purpose>]

The pilot's acceptance summary holds every cell's percentile interval at the
pilot's ledger-derived corrected alpha, the value protocol P-03 recomputes
from the raw pairs. The widest relative half-width,
max(|s - l|, |u - s|) / s over the pilot's cells, rounded up to two decimals,
becomes the frozen measurement resolution; the pilot receipt is pinned by path
and SHA-256 as its evidence. `--resolution-decimals` selects another rounding
step for a family whose frozen rule states one. Every retained cell keeps its pilot declaration
with the confirmatory role. A margin the pilot's resolution invalidates may be
replaced, with its new rationale, through the margin options, and a family whose
pilot declares no worthwhile speedup fixes one the same way, and a margin the
confirmation keeps may restate its rationale alone; the record names every
replaced value and every restated rationale. `--family-description` restates
the family prose the confirmation stage carries. The script refuses to write an
addendum whose margins do not strictly exceed one plus that resolution, and it
writes the derivation record beside the addendum so the frozen number is
reproducible from committed bytes.

A family that calibrates a selector confirms it on holdout cells, which are
fresh samples that took no part in selection and therefore appear in no pilot
cell. `--holdout-cells` names a committed declaration whose `addendum_cells`
are appended with the holdout role and listed under `holdout.cells`, and
`--purpose` restates the family purpose the confirmation stage carries, since a
pilot that has calibrated nothing is not itself a calibration. The record names
the declaration, its digest and every holdout identifier, so the confirmation's
holdout is as reproducible from committed bytes as its resolution. The
declaration must predate the pilot receipt for its cells to be a holdout at
all; that ordering is the executor's to establish and the record states the
declaration's digest so a reviewer can check it against the commit that
introduced it.

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
    parser.add_argument("--worthwhile-speedup", type=float)
    parser.add_argument("--worthwhile-rationale")
    parser.add_argument("--equivalence-margin", type=float)
    parser.add_argument("--equivalence-rationale")
    parser.add_argument("--material-gap-threshold", type=float)
    parser.add_argument("--material-gap-rationale")
    parser.add_argument("--cell", action="append", default=[], dest="cells")
    parser.add_argument("--selection-rationale")
    parser.add_argument("--family-description")
    parser.add_argument("--resolution", type=float)
    parser.add_argument("--resolution-derivation")
    parser.add_argument("--resolution-decimals", type=int, default=2)
    parser.add_argument("--holdout-cells")
    parser.add_argument("--purpose")
    args = parser.parse_args()
    if bool(args.cells) != bool(args.selection_rationale):
        raise SystemExit("a cell selection needs both --cell and --selection-rationale")
    if (args.resolution is None) != (args.resolution_derivation is None):
        raise SystemExit("a supplied resolution needs both a value and its derivation")
    replacements = {
        "worthwhile_speedup": (args.worthwhile_speedup, "rationale", args.worthwhile_rationale),
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

    family = summary["family"]
    # Acceptance-summary schema `zen3-benchmark-acceptance-v1` reports the
    # sequential-attempt allocation under the single field name `family_alpha`.
    # `zen3-benchmark-acceptance-v2` (`@/issue/c5e01de3`) separates that
    # allocation (`attempt_alpha`) from the frozen total (`family_alpha`) and
    # the per-comparison corrected level (`corrected_alpha`). The legacy
    # branch below is keyed on the declared schema identity, not on which
    # fields happen to be present, so a summary that adds fields under the
    # v1 identity in the future still reads as v1. A v1 summary reproduces
    # its committed derivation record byte for byte in the single-line format
    # the freezer always wrote for it.
    LEGACY_ACCEPTANCE_SCHEMA = "zen3-benchmark-acceptance-v1"
    if summary["schema"] == LEGACY_ACCEPTANCE_SCHEMA:
        family_line = (
            f"family         {family['family_id']}: {family['comparisons']} ledger comparisons, "
            f"attempt alpha {family['family_alpha']}, per-comparison confidence "
            f"{family['per_comparison_confidence']}"
        )
    else:
        family_line = (
            f"family         {family['family_id']}: {family['comparisons']} ledger comparisons, "
            f"family-wise alpha {family['family_alpha']}, attempt alpha {family['attempt_alpha']}, "
            f"corrected alpha {family['corrected_alpha']}, per-comparison confidence "
            f"{family['per_comparison_confidence']}"
        )
    lines = [
        f"pilot receipt  {receipt_path}",
        f"receipt sha256 {receipt_sha}",
        f"campaign       {summary['campaign_id']}",
        f"verdict        {summary['verdict']} (label {summary['label']})",
        family_line,
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
    decimals = args.resolution_decimals
    step = 10 ** decimals
    resolution = math.ceil(widest * step) / step
    lines += ["", f"widest relative half-width   {widest:.6f}"]
    if args.resolution is not None:
        if args.resolution < resolution:
            raise SystemExit(
                f"supplied resolution {args.resolution} is below the pilot-alpha floor {resolution}")
        with open(args.resolution_derivation) as handle:
            derivation = handle.read().rstrip("\n")
        lines += [f"pilot-alpha floor            {resolution:.{decimals}f}",
                  f"supplied resolution          {args.resolution:.{decimals}f}",
                  f"derivation                   {args.resolution_derivation}", "", derivation, ""]
        resolution = args.resolution
    lines.append(f"frozen measurement resolution {resolution:.{decimals}f}")

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
    if args.purpose:
        lines.append(f"restated purpose {frozen['family']['purpose']} -> {args.purpose}")
        frozen["family"]["purpose"] = args.purpose
    if args.holdout_cells:
        with open(args.holdout_cells, "rb") as handle:
            declaration_bytes = handle.read()
        declaration = json.loads(declaration_bytes)
        if declaration["family"] != frozen["family"]["id"]:
            raise SystemExit("the holdout declaration belongs to another family")
        pilot_ids = {cell["cell_id"] for cell in addendum["cells"]}
        holdout_cells = []
        for cell in declaration["addendum_cells"]:
            if cell["cell_id"] in pilot_ids:
                raise SystemExit(
                    f"holdout cell {cell['cell_id']} is also a pilot cell, so its"
                    " samples took part in selection")
            cell = copy.deepcopy(cell)
            cell["role"] = "holdout"
            holdout_cells.append(cell)
        frozen["cells"] += holdout_cells
        frozen["holdout"] = {
            "required": True,
            "cells": [cell["cell_id"] for cell in holdout_cells],
        }
        lines += [
            "",
            f"holdout declaration {args.holdout_cells}",
            f"declaration sha256  {hashlib.sha256(declaration_bytes).hexdigest()}",
        ]
        lines += [f"holdout cell        {cell['cell_id']}" for cell in holdout_cells]
        lines.append(f"holdout rationale   {declaration['rationale']}")
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
