#!/usr/bin/env python3
"""Freeze a family's exploratory re-measurement addendum after the warm-pass fix.

Usage: dev/active/6fb89a3c/survey/freeze-remeasure.py transpose|logical|bch <frozen-utc>

The family's v3 pilot and confirmation ran gf2 arms that reported `warm`
without the protocol's untimed pass before calibration. The re-measurement
repeats every cell of the family's committed v3 pilot addendum with the fixed
arms. Protocol v3 allows one confirmatory attempt per candidate identity and
version, and the withdrawn confirmation spent that attempt for every external
candidate, so every re-measured cell is exploratory. The addendum keeps the
pilot's cells, seeds and margins, records the pilot trials the family has
used, and says why; it is written to `<stem>-v3-remeasure.json`.
"""
from __future__ import annotations

import json
import pathlib
import sys

ISSUE = pathlib.Path("dev/active/6fb89a3c")
FAMILIES = {
    # stem, pilot trials per cell including this campaign, trial accounting
    "transpose": ("addendum-transpose", 3,
                  "The five cells the failed v3-r1 pilot measured reach their third pilot trial here and the other three "
                  "cells their second"),
    "logical": ("addendum-logical-buffer", 2, "Every cell reaches its second pilot trial here"),
    "bch": ("addendum-bch-genmatrix", 2, "Every cell reaches its second pilot trial here"),
}
SPLITMIX = ("Workload seeds expand through SplitMix64 [Steele2014], implemented as tuning_campaign_support::abtest::SplitMix64 "
            "(dev/tools/tuning-campaign-support/src/abtest.rs) in the gf2 arms and as splitmix64_next "
            "(dev/active/6fb89a3c/survey/harness_common.h) in the C arms; both are pinned by the producing snapshot.")


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    family, frozen_utc = sys.argv[1], sys.argv[2]
    stem, trials, accounting = FAMILIES[family]
    pilot = json.loads((ISSUE / f"{stem}-v3-pilot.json").read_text())
    confirmation = json.loads((ISSUE / f"{stem}-v3-confirmation.json").read_text())
    document = json.loads(json.dumps(pilot))
    description = pilot["family"]["description"]
    if not description.startswith("Exploratory pilot of"):
        raise SystemExit("pilot description changed; review the re-measurement text")
    description = "Exploratory re-measurement of" + description[len("Exploratory pilot of"):]
    for pilot_clause, replacement in ((" This pilot sizes the family's measurement resolution; no cell here decides adoption.", ""),
                                      ("; no cell here decides adoption.", ".")):
        description = description.replace(pilot_clause, replacement)
    description += (
        " The family's v3 pilot and confirmation ran gf2 arms that reported `warm` without the protocol's untimed pass "
        "over the working set before calibration, so their gf2 `warm` declarations are false and their comparisons are "
        "withdrawn. This campaign re-measures every cell of the v3 pilot addendum with fixed gf2 arms that make that pass "
        "(timed_windows in dev/active/6fb89a3c/survey/gf2-side/src/lib.rs). Protocol v3 allows one confirmatory attempt "
        "per candidate identity and version, and the withdrawn confirmation spent it for every external candidate, so "
        "every cell here is exploratory and no interval carries a confirmatory decision. " + accounting
        + "; this addendum's search budget records that count. " + SPLITMIX
        + " This survey adopts no production implementation; no cell here decides adoption.")
    document["family"]["description"] = description
    document["frozen"] = {"frozen_utc": frozen_utc}
    effect = document["effect"]
    effect["measurement_resolution"] = None
    effect["resolution_evidence"] = None
    effect["equivalence_margin"] = confirmation["effect"]["equivalence_margin"]
    effect["material_gap_threshold"] = confirmation["effect"]["material_gap_threshold"]
    effect["equivalence_rationale"] = (
        "The one-sided non-inferiority margin the withdrawn confirmation froze; exploratory cells record decisions under "
        "it and never pass.")
    effect["material_gap_rationale"] = (
        confirmation["effect"]["material_gap_rationale"].split("; it exceeds one plus the frozen resolution")[0]
        + "; it is the threshold the withdrawn confirmation froze.")
    document["search_budget"]["max_pilot_trials_per_cell"] = trials
    for cell in document["cells"]:
        if cell["role"] != "exploratory" or cell["cache_state"] != "warm":
            raise SystemExit(f"pilot cell {cell['cell_id']} is not an exploratory warm cell")

    def cell_text(c):
        return ("    {\n"
                f"      \"cell_id\": {json.dumps(c['cell_id'])}, \"objective\": {json.dumps(c['objective'])}, \"role\": {json.dumps(c['role'])},\n"
                f"      \"workload\": {json.dumps(c['workload'], separators=(', ', ': '))},\n"
                f"      \"metric_kind\": {json.dumps(c['metric_kind'])}, \"scaling\": {json.dumps(c['scaling'])}, \"core_arm\": {json.dumps(c['core_arm'])},\n"
                f"      \"workers\": {json.dumps(c['workers'], separators=(', ', ': '))}, \"cache_state\": {json.dumps(c['cache_state'])},\n"
                f"      \"builds\": {json.dumps(c['builds'], separators=(', ', ': '))}, \"conversion_costs_included\": {json.dumps(c['conversion_costs_included'])}, \"decoder\": null\n"
                "    }")
    head = {k: v for k, v in document.items() if k != "cells"}
    text = "{\n" + ",\n".join(
        f"  {json.dumps(k)}: " + (json.dumps(v, indent=2).replace("\n", "\n  ") if k in ("family", "effect") else json.dumps(v, separators=(', ', ': ')))
        for k, v in head.items())
    text += ",\n  \"cells\": [\n" + ",\n".join(cell_text(c) for c in document["cells"]) + "\n  ]\n}\n"
    if json.loads(text) != document:
        raise SystemExit("rendered addendum differs from the derived document")
    output = ISSUE / f"{stem}-v3-remeasure.json"
    output.write_text(text)
    print(f"{family}: {len(document['cells'])} exploratory cells, max_pilot_trials_per_cell {trials} -> {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
