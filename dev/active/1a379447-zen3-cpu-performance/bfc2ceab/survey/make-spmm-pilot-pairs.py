#!/usr/bin/env python3
"""Size the second pilot trial of the sparse product check-cost family (jit:bfc2ceab).

Usage: make-spmm-pilot-pairs.py <benchmark-acceptance executable>

Writes `spmm-second-pilot-pairs.json` beside itself from the family's committed
first pilot receipt. Every interval is the evaluator's own bootstrap, read
through `benchmark-acceptance --bootstrap` under the seed the acceptance
summary records; `verification.md` states the two sizings the record holds.
"""

import json
import math
import subprocess
import sys

import review
import spmm_family
from locate import HERE, ROOT

RECORD = "spmm-second-pilot-pairs.json"
# The canonical freezer rounds the widest half-width up to this many decimals.
RESOLUTION_DECIMALS = 2
# Factor on the projected half-width: a pilot interval is itself an estimate.
SAFETY_FACTOR = 1.25


def intervals(tool, pairs, resamples, seed, alphas):
    """The evaluator's estimate of `pairs` and its interval at each named alpha."""
    request = {
        "pairs": [{"baseline_ns_per_call": baseline, "candidate_ns_per_call": candidate}
                  for baseline, candidate in pairs],
        "resamples": resamples, "seed": seed, "alphas": list(alphas.values()),
    }
    sized = json.loads(subprocess.run(
        [tool, "--bootstrap"], input=json.dumps(request), capture_output=True, check=True,
        text=True,
    ).stdout)
    found = {
        name: {"alpha": alpha, "lower": row["interval"]["lower"],
               "upper": row["interval"]["upper"],
               "relative_half_width": row["relative_half_width"]}
        for (name, alpha), row in zip(alphas.items(), sized)
    }
    return sized[0]["interval"]["estimate"], found


def required_pairs(pairs, half_width, target):
    return math.ceil(pairs * (SAFETY_FACTOR * half_width / target) ** 2)


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    tool = sys.argv[1]
    addendum = spmm_family.pilot_addendum()
    _, stage, receipt = spmm_family.stages()[0]
    summary = json.loads((stage / "acceptance-summary.json").read_text())
    if summary["verdict"] != "accepted":
        raise SystemExit("the acceptance summary does not accept the committed pilot receipt")
    settings, effect = receipt["settings"], addendum["effect"]

    # A first confirmatory attempt spends alpha / (1 * 2) over the retained
    # cells (`trial_ledger::attempt_alpha`, `trial_ledger::comparisons`).
    alphas = {
        "pilot": summary["family"]["corrected_alpha"],
        "confirmation": settings["family_alpha"] / 2.0 / len(addendum["cells"]),
    }
    step = 10 ** RESOLUTION_DECIMALS
    margins = [effect[name] for name in ("worthwhile_speedup", "equivalence_margin")]
    # The freezer needs every margin strictly above one plus the rounded-up
    # resolution, so the widest half-width may reach the step below the margin.
    target = (min(round((margin - 1.0) * step) for margin in margins) - 1) / step
    reported = {cell["cell_id"]: cell["interval"] for cell in summary["cells"]}

    cells = []
    for cell in receipt["cells"]:
        pairs = [(p["baseline"]["ns_per_call"], p["candidate"]["ns_per_call"])
                 for p in cell["pairs"]]
        expected = reported[cell["cell_id"]]
        seed = expected["seed"]
        estimate, whole = intervals(tool, pairs, settings["bootstrap_resamples"], seed, alphas)
        if (estimate, whole["pilot"]["lower"], whole["pilot"]["upper"]) != (
                expected["estimate"], expected["lower"], expected["upper"]):
            raise SystemExit(f"{cell['cell_id']}: the tool is not the summary's evaluator")
        fastest = [min(column) for column in zip(*pairs)]
        ceiling = [effect["equivalence_margin"] * value for value in fastest]
        disturbed = [index for index, pair in enumerate(pairs)
                     if any(value > limit for value, limit in zip(pair, ceiling))]
        kept = [pair for index, pair in enumerate(pairs) if index not in disturbed]
        entry = {
            "cell_id": cell["cell_id"], "pairs": len(pairs), "estimate": estimate,
            "intervals": whole, "disturbed_pairs": disturbed,
        }
        if disturbed and len(kept) >= 2:
            kept_estimate, kept_intervals = intervals(
                tool, kept, settings["bootstrap_resamples"], seed, alphas)
            entry["undisturbed"] = {
                "pairs": len(kept), "estimate": kept_estimate, "intervals": kept_intervals}
        cells.append(entry)

    def widest(select):
        return max(((select(cell), cell["cell_id"]) for cell in cells if select(cell)),
                   key=lambda found: found[0]["intervals"]["confirmation"]["relative_half_width"])

    whole, whole_cell = widest(lambda cell: cell)
    quiet, quiet_cell = widest(lambda cell: cell.get("undisturbed", cell))
    cap = settings["pilot_max_pairs"]
    law = {
        name: required_pairs(whole["pairs"], whole["intervals"][name]["relative_half_width"],
                             target)
        for name in alphas
    }
    projected = {
        name: SAFETY_FACTOR * quiet["intervals"][name]["relative_half_width"]
        * math.sqrt(quiet["pairs"] / cap)
        for name in alphas
    }
    record = {
        "schema": "spmm-second-pilot-pairs-v1",
        "generator": str((HERE / "make-spmm-pilot-pairs.py").relative_to(ROOT)),
        "first_pilot": {
            "receipt": str((stage / "receipt.json").relative_to(ROOT)),
            "sha256": spmm_family.sha256(stage / "receipt.json"),
            "session_load_average": [host["load_average"] for host in receipt["session_hosts"]],
        },
        "pilot_addendum": str(
            (spmm_family.FAMILY / review.GRAIN_FAMILY_ADDENDUM).relative_to(ROOT)),
        "pilot_trials_admitted_per_cell": addendum["search_budget"]["max_pilot_trials_per_cell"],
        "alphas": alphas,
        "resolution_step": 1 / step,
        "safety_factor": SAFETY_FACTOR,
        "largest_admissible_half_width": target,
        "pilot_max_pairs": cap,
        "cells": cells,
        "square_root_law": {
            "widest_cell": whole_cell,
            "required_pairs": law,
            "within_pilot_max_pairs": {name: pairs <= cap for name, pairs in law.items()},
        },
        "undisturbed": {
            "label": "estimate: the subset is selected after the samples were read",
            "rule": "a pair is disturbed when an execution exceeds equivalence_margin times "
                    "its arm's fastest execution of the cell",
            "widest_cell": quiet_cell,
            "projected_half_width_at_pilot_max_pairs": projected,
            "below_largest_admissible": {
                name: value <= target for name, value in projected.items()},
        },
        "second_pilot": {
            "pairs": cap,
            "basis": "pilot_max_pairs: the square-root law asks for more pairs than a pilot "
                     "may take, so the trial takes the largest admitted sample",
        },
    }
    (HERE / RECORD).write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    main()
