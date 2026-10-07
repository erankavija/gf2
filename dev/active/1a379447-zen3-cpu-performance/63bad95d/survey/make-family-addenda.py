#!/usr/bin/env python3
"""Project the pilot addenda and holdout declarations of the profile families (jit:63bad95d).

Reads `planned-families.json` and writes, per family, the frozen pilot addendum
`campaigns/<family>-pilot.json`, its genesis ledger when none exists, and the
holdout declaration the plan names for it. A pilot holds the family's piloted
cells as exploratory cells; the holdout cells appear in the declaration alone,
which the canonical freezer attaches to the confirmation.

Nothing here is a measurement. `--check` compares the committed files with the
projection and writes nothing.

Usage: make-family-addenda.py [--check]
"""

import json
import sys

from locate import HERE, ROOT, repository_files

ISSUE = HERE.parent
FROZEN_UTC = "2026-10-07T05:01:11Z"
# Receipts and ledgers sit beside the launcher.
RESULTS = str((ROOT / repository_files.live_file(ROOT, "run-profile-families.sh")).parent.relative_to(ROOT))

DESCRIPTIONS = {
    "63bad95d-profile-dispatch-overhead": (
        "Exploratory pilot of issue 63bad95d, protocol version 4: this addendum is the pilot "
        "and every cell is exploratory. Question: is a gf2-core build that bakes the measured "
        "core owner's selectors no worse than the conservative build on small buffers of the "
        "bit-backend entry points? The baseline arm is the conservative-portable build: default "
        "target, simd and tuning-profile features, conservative selector constants, no profile "
        "installed. The candidate arm is the tuned-portable build: the same sources and features "
        "compiled with --cfg gf2_tuning_baked, with the measured core owner the plan pins "
        "installed before the first selector read. The two arms are separate executables of one "
        "harness crate and differ in RUSTFLAGS and the installed owner alone. The baked "
        "bit_backend.simd_min_words differs between the two builds, so a cell whose buffer "
        "length lies between the two thresholds runs the scalar backend in the baseline and the "
        "SIMD backend in the candidate, and each arm reports the backend or route the library "
        "selects for its operands and the compiled threshold it observes. Every cell is "
        "kernel-isolated, warm, single-core and calibrated; operands are prepared before timing "
        "and no conversion is included. The confirmation adds the holdout cells of "
        "survey/dispatch-holdout-cells.json, which no pilot measures. Frozen decision rule: the "
        "baked threshold of the measured owner is retained when no confirmatory or holdout cell "
        "records fail, and the baked threshold takes the conservative value otherwise; "
        "the rule reads the acceptance summary's recorded outcomes and nothing else."
    ),
    "63bad95d-native-ladder": (
        "Exploratory pilot of issue 63bad95d, protocol version 4: this addendum is the pilot "
        "and every cell is exploratory. Question: is the native build of the profile no worse "
        "than its portable build on one primary workload per entry-point group? The baseline "
        "arm is the tuned-portable build: default target, simd and tuning-profile features, "
        "--cfg gf2_tuning_baked, the measured core owner the plan pins installed before the "
        "first selector read. The candidate arm is the native build: the same sources, features, "
        "cfg and installed owner with -C target-cpu=native added. The two arms are separate "
        "executables of one harness crate and differ in that one flag. Kernel-isolated cells "
        "time the public kernel entry on prepared operands; whole-consumer cells time the public "
        "BitMatrix call including its output allocation and release. Every cell is warm, "
        "single-core and calibrated. The family selects nothing: it reports whether a consumer "
        "who builds for the host loses anything against the portable profile build."
    ),
}

WORTHWHILE = (
    1.05,
    "Five percent of a kernel call is the smallest change a consumer dominated by these calls "
    "would act on. The confirmation refreezes the margin strictly above one plus the "
    "pilot-derived resolution.",
)
EQUIVALENCE = (
    1.05,
    "Within five percent the two builds are interchangeable for a consumer. The confirmation "
    "refreezes the margin strictly above one plus the pilot-derived resolution.",
)
MATERIAL_GAP = "The family holds no comparator-gap cell and declares no external gap."
MAINTENANCE = (
    "Zero added production lines and zero unsafe kernels: the family measures build "
    "configurations of committed sources, and its outcome is the value of existing baked "
    "constants."
)
HOLDOUT_RATIONALE = (
    "The declared cells are measured by no pilot of this family, and the offline producer's "
    "bit-backend sweep times none of these operation and length pairs, so no sample of a "
    "held-out workload takes part in selecting the threshold or in sizing the confirmation's "
    "resolution. Each cell is a non-regression cell of the profile build against the "
    "conservative build."
)


def declaration(cell, role):
    case = cell["case"]
    size = {name: value for name, value in case.items() if name not in ("operation", "seed")}
    extent = "x".join(str(size[name]) for name in sorted(size))
    whole = cell["metric_kind"] == "whole-consumer"
    return {
        "cell_id": cell["cell_id"],
        "objective": cell["objective"],
        "role": role,
        "workload": {
            "identity": f"profile-{case['operation']}-{extent}",
            "size": size,
            "seed": case["seed"],
        },
        "metric_kind": cell["metric_kind"],
        "scaling": "single-core-latency",
        "core_arm": cell["core_arm"],
        "workers": {"declared": 1, "nested_pools_allowed": False},
        "cache_state": cell["cache_state"],
        "builds": {"baseline": cell["baseline_build"], "candidate": cell["candidate_build"]},
        "conversion_costs_included": whole,
        "decoder": None,
    }


def projections(plan):
    """Root-relative path to canonical text, for every file this generator owns."""
    files = {}
    issue = ISSUE.relative_to(ROOT)
    for family in plan["families"]:
        piloted = [cell for cell in family["cells"] if cell["role"] != "holdout"]
        held_out = [cell for cell in family["cells"] if cell["role"] == "holdout"]
        addendum = {
            "schema": "zen3-benchmark-addendum-v4",
            "protocol": {"id": "zen3-benchmark-protocol", "version": 4},
            "family": {
                "id": family["id"],
                "issue": plan["issue"],
                "purpose": family["pilot_purpose"],
                "description": DESCRIPTIONS[family["id"]],
            },
            "frozen": {"frozen_utc": FROZEN_UTC},
            "effect": {
                "worthwhile_speedup": WORTHWHILE[0],
                "rationale": WORTHWHILE[1],
                "measurement_resolution": None,
                "resolution_evidence": None,
                "equivalence_margin": EQUIVALENCE[0],
                "equivalence_rationale": EQUIVALENCE[1],
                "material_gap_threshold": None,
                "material_gap_rationale": MATERIAL_GAP,
            },
            "complexity_budget": {
                "max_new_unsafe_kernels": 0,
                "max_added_source_lines": 0,
                "maintenance_rationale": MAINTENANCE,
            },
            "family_wise": {
                "alpha": 0.05,
                "prior_confirmatory_trials": 0,
                "prior_trials": [],
                "ledger_path": f"{RESULTS}/{family['ledger']}",
            },
            "search_budget": {
                "max_pilot_trials_per_cell": 2,
                "max_confirmatory_attempts_per_candidate": 1,
            },
            "holdout": {"required": False, "cells": []},
            "cells": [declaration(cell, "exploratory") for cell in piloted],
        }
        files[f"{issue}/campaigns/{family['id']}-pilot.json"] = (
            json.dumps(addendum, indent=1) + "\n"
        )
        if held_out:
            record = {
                "schema": "63bad95d-holdout-declaration-v1",
                "family": family["id"],
                "rationale": HOLDOUT_RATIONALE,
                "addendum_cells": [declaration(cell, "holdout") for cell in held_out],
            }
            files[f"{issue}/survey/{family['holdout_declaration']}"] = (
                json.dumps(record, indent=1) + "\n"
            )
    return files


def main():
    check = sys.argv[1:] == ["--check"]
    if sys.argv[1:] and not check:
        raise SystemExit(__doc__)
    plan = json.loads((HERE / "planned-families.json").read_text())
    stale = []
    for path, text in sorted(projections(plan).items()):
        target = ROOT / path
        if check:
            if not target.is_file() or target.read_text() != text:
                stale.append(path)
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
        print(path)
    for family in plan["families"]:
        ledger = ROOT / RESULTS / family["ledger"]
        if check:
            if not ledger.is_file():
                stale.append(str(ledger.relative_to(ROOT)))
        elif not ledger.exists():
            ledger.parent.mkdir(parents=True, exist_ok=True)
            ledger.write_bytes(b"")
            print(ledger.relative_to(ROOT))
    if stale:
        raise SystemExit("stale or absent: " + ", ".join(stale))


if __name__ == "__main__":
    main()
