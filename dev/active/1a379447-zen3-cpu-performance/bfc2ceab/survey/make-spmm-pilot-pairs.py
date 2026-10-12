#!/usr/bin/env python3
"""Size the second pilot trial of the sparse product check-cost family (jit:bfc2ceab).

Usage: make-spmm-pilot-pairs.py, with no arguments

Writes `spmm-second-pilot-pairs.json` beside itself from the family's committed
first pilot receipt, its acceptance summary and the frozen pilot addendum. The
launcher and the arm smoke read the second trial's pair count from that record.

The bootstrap is the protocol's (`abtest::paired_bootstrap_speedup`, P-20):
whole pairs resampled under xoshiro256** seeded through SplitMix64, the speedup
of medians per draw, nearest-rank percentiles. The port is accepted only when
it reproduces every interval of the acceptance summary exactly, and it then
gives the same replicates' percentiles at the corrected alpha of a first
confirmation that retains every cell.

The record holds two sizings. `square_root_law` scales the first pilot's widest
half-width to the half-width the frozen margins need and names the pair count
that asks for. `undisturbed` repeats the half-widths on the pairs in which
neither execution exceeds its arm's fastest execution of the cell by the
equivalence margin, and projects them to the largest pilot the protocol admits;
that projection is an estimate, since the subset is selected after the fact.
"""

import hashlib
import json
import math
import sys

from locate import HERE, ROOT

PILOT_ADDENDUM = "addendum-v4-spmm-check-pilot.json"
FIRST_PILOT = "r1-spmm-check-pilot"
RECORD = "spmm-second-pilot-pairs.json"
# The canonical freezer rounds the widest half-width up to this many decimals.
RESOLUTION_DECIMALS = 2
# Factor on the projected half-width: a pilot interval is itself an estimate.
SAFETY_FACTOR = 1.25
MASK = (1 << 64) - 1


class SplitMix64:
    def __init__(self, seed):
        self.state = seed

    def next(self):
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)


def rotl(value, count):
    return ((value << count) | (value >> (64 - count))) & MASK


class Xoshiro256StarStar:
    def __init__(self, seed):
        mixer = SplitMix64(seed)
        self.s = [mixer.next() for _ in range(4)]

    def below(self, n):
        s = self.s
        result = (rotl((s[1] * 5) & MASK, 7) * 9) & MASK
        t = (s[1] << 17) & MASK
        s[2] ^= s[0]
        s[3] ^= s[1]
        s[1] ^= s[2]
        s[0] ^= s[3]
        s[2] ^= t
        s[3] = rotl(s[3], 45)
        return min(int((result >> 11) / (1 << 53) * n), n - 1)


def median(values):
    values = sorted(values)
    middle = len(values) // 2
    return values[middle] if len(values) % 2 else (values[middle - 1] + values[middle]) / 2.0


def speedup(pairs):
    return median([b for b, _ in pairs]) / median([c for _, c in pairs])


def cell_seed(campaign_seed, key):
    word = int.from_bytes(hashlib.sha256(key.encode()).digest()[:8], "little")
    return SplitMix64(campaign_seed ^ word).next()


def intervals(pairs, resamples, seed, alphas):
    """The percentile interval of one replicate set at each alpha."""
    generator, n = Xoshiro256StarStar(seed), len(pairs)
    replicates = sorted(
        speedup([pairs[generator.below(n)] for _ in range(n)]) for _ in range(resamples))
    estimate = speedup(pairs)

    def at(quantile):
        return replicates[min(max(math.ceil(quantile * resamples), 1), resamples) - 1]

    found = {}
    for name, alpha in alphas.items():
        lower, upper = at(alpha / 2.0), at(1.0 - alpha / 2.0)
        found[name] = {
            "alpha": alpha, "lower": lower, "upper": upper,
            "relative_half_width": max(abs(estimate - lower), abs(upper - estimate)) / estimate,
        }
    return estimate, found


def required_pairs(pairs, half_width, target):
    return math.ceil(pairs * (SAFETY_FACTOR * half_width / target) ** 2)


def main():
    if sys.argv[1:]:
        raise SystemExit(__doc__)
    addendum = json.loads((HERE.parent / PILOT_ADDENDUM).read_text())
    stage = (ROOT / addendum["family_wise"]["ledger_path"]).parent / FIRST_PILOT
    receipt_bytes = (stage / "receipt.json").read_bytes()
    receipt = json.loads(receipt_bytes)
    summary = json.loads((stage / "acceptance-summary.json").read_text())
    receipt_sha = hashlib.sha256(receipt_bytes).hexdigest()
    if summary["receipt_sha256"] != receipt_sha or summary["verdict"] != "accepted":
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
        seed = cell_seed(receipt["campaign_seed"], cell["key"])
        estimate, whole = intervals(pairs, settings["bootstrap_resamples"], seed, alphas)
        expected = reported[cell["cell_id"]]
        if (seed, estimate, whole["pilot"]["lower"], whole["pilot"]["upper"]) != (
                expected["seed"], expected["estimate"], expected["lower"], expected["upper"]):
            raise SystemExit(f"{cell['cell_id']}: the port differs from the acceptance summary")
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
                kept, settings["bootstrap_resamples"], seed, alphas)
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
            "sha256": receipt_sha,
            "session_load_average": [host["load_average"] for host in receipt["session_hosts"]],
        },
        "pilot_addendum": str((HERE.parent / PILOT_ADDENDUM).relative_to(ROOT)),
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
