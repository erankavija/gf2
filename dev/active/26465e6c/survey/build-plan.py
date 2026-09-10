#!/usr/bin/env python3
"""Projects a `zen3-benchmark-plan-v1` from a frozen addendum (jit:26465e6c).

  build-plan.py <popcount|and-popcnt> <addendum> <campaign-id> <label>
                <arm-binary> <lock-path> <max-cells-per-session> <output>

Every cell's arms and case come from `families.py`; the addendum's cells must
equal that design in both directions, including workload size, seed, cache
state, metric kind and build identities, so a plan cannot measure a cell the
addendum does not freeze. The campaign seed is derived from the campaign
identity, so it is fixed before any sample exists. Exploratory cells run
twelve pairs, two counterbalanced blocks of six; confirmatory cells run the
protocol's frozen count.
"""

import hashlib
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import families  # noqa: E402

PILOT_PAIRS = 12
PRODUCING_MANIFEST = f"dev/active/{families.ISSUE}/survey/producing-inputs.json"


def expected_cell(family, key, candidate):
    if family == "popcount":
        words, offset, _, cache, seed, _ = families.POPCOUNT_WORKLOADS[key]
        metric, conversion = "kernel-isolated", False
    else:
        words, cache, seed, _ = families.AND_WORKLOADS[key]
        offset = 0
        metric, conversion = families.AND_CANDIDATES[candidate]
    return {
        "size": {"words": words, "word_offset": offset},
        "seed": seed,
        "cache_state": cache,
        "metric_kind": metric,
        "conversion_costs_included": conversion,
    }


def main():
    if len(sys.argv) != 9 or sys.argv[1] not in families.FAMILIES:
        sys.exit(__doc__)
    family, addendum_path, campaign, label, binary, lock, max_cells, output = sys.argv[1:]
    spec = families.FAMILIES[family]
    addendum = json.loads(pathlib.Path(addendum_path).read_text(encoding="utf-8"))
    if addendum["family"]["id"] != spec["id"]:
        sys.exit(f"{addendum_path} freezes {addendum['family']['id']}, not {spec['id']}")
    design = {cell_id: (key, candidate) for cell_id, key, candidate in spec["cells"]()}
    declared = {cell["cell_id"]: cell for cell in addendum["cells"]}
    if set(design) != set(declared):
        sys.exit(f"addendum and design disagree on cells: {sorted(set(design) ^ set(declared))}")
    baseline = spec["baseline"]
    cells, used = [], {baseline}
    for cell in addendum["cells"]:
        key, candidate = design[cell["cell_id"]]
        expected = expected_cell(family, key, candidate)
        observed = {
            "size": cell["workload"]["size"],
            "seed": cell["workload"]["seed"],
            "cache_state": cell["cache_state"],
            "metric_kind": cell["metric_kind"],
            "conversion_costs_included": cell["conversion_costs_included"],
        }
        if observed != expected:
            sys.exit(f"cell {cell['cell_id']}: addendum {observed} differs from design {expected}")
        builds = {"baseline": families.ARMS[baseline][0], "candidate": families.ARMS[candidate][0]}
        if cell["builds"] != builds:
            sys.exit(f"cell {cell['cell_id']}: builds {cell['builds']} differ from {builds}")
        used.add(candidate)
        cells.append({
            "cell_id": cell["cell_id"],
            "baseline_arm": baseline,
            "candidate_arm": candidate,
            "case": dict(sorted(families.case_for(family, key, candidate).items())),
            "pilot_pairs": PILOT_PAIRS if cell["role"] == "exploratory" else None,
        })
    arms = {
        name: {
            "build": families.ARMS[name][0],
            "description": families.ARMS[name][1],
            "executable": binary,
            "arguments": [],
            "environment": {families.ARM_VAR: name},
            "rustflags": None,
            "tuning_profile": None,
        }
        for name in sorted(used)
    }
    plan = {
        "schema": "zen3-benchmark-plan-v1",
        "campaign_id": campaign,
        "issue": families.ISSUE,
        "label": label,
        "campaign_seed": int(hashlib.sha256(campaign.encode()).hexdigest()[:15], 16),
        "addendum": addendum_path,
        "producing_manifest": PRODUCING_MANIFEST,
        "lock_path": lock,
        "wrapper": "dev/scripts/ccx1-bench-flock.sh",
        "timing_override": None,
        "arms": arms,
        "cells": cells,
        "max_cells_per_session": int(max_cells),
    }
    pathlib.Path(output).write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    print(f"{output}: {len(cells)} cells over {len(arms)} arms, seed {plan['campaign_seed']}")


if __name__ == "__main__":
    main()
