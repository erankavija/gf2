#!/usr/bin/env python3
"""Recompute the confirmatory budget and the window estimate of the calibration plan (jit:63bad95d).

Reads the frozen settings and the P-20 tail rule from the campaign-support
sources, every committed family ledger of the core-kernel track, the planned
families of `planned-families.json`, and the child durations the committed
core-kernel receipts record. Writes `calibration-budget.json` and
`calibration-budget.md` into the issue directory.

For a ledger with `t - 1` reservations that spend a comparison and `m0`
reserved comparisons, attempt `t` with `n` non-exploratory cells has the
corrected alpha `alpha / (t (t + 1)) / (m0 + n)`; P-20 admits the cells while
`resamples * corrected_alpha / 2` reaches the minimum tail draws.

Usage: make-calibration-budget.py, with no arguments
"""

import json
import re
import statistics

from locate import HERE, ROOT, package, repository_files, tracked

ISSUE = HERE.parent
SUPPORT = "tuning-campaign-support"
TAIL_RULE = re.compile(
    r"f64::from\(settings\.bootstrap_resamples\) \* corrected_alpha / 2\.0 < (\d+)\.0;"
)
ATTEMPT_RULE = "    Ok(family.family_wise.alpha / (attempts * (attempts + 1.0)))"
SEAM_PROTOCOL = ("premeasurement-protocol.md",
                 b"# Seam threshold calibration: premeasurement protocol")
SESSION_BUDGET = re.compile(r"The session budget stays ([\d,]+) s")


def frozen(text, name):
    found = re.search(rf"^    {name}: ([\d_.]+),$", text, re.M)
    if not found:
        raise SystemExit(f"the protocol source does not carry {name}")
    return float(found.group(1).replace("_", ""))


def admitted(alpha, resamples, tail, attempt, reserved):
    """Largest cell count attempt `attempt` admits over `reserved` earlier comparisons."""
    attempt_alpha = alpha / (attempt * (attempt + 1))
    count = 0
    while resamples * (attempt_alpha / (reserved + count + 1)) / 2.0 >= tail:
        count += 1
    return attempt_alpha, count


def main():
    support = f"{package(SUPPORT)}/src"
    protocol = (ROOT / support / "protocol.rs").read_text()
    receipt = (ROOT / support / "receipt.rs").read_text()
    ledger_source = (ROOT / support / "trial_ledger.rs").read_text()
    tail = TAIL_RULE.search(receipt)
    if not tail or ATTEMPT_RULE not in ledger_source:
        raise SystemExit("the campaign-support sources no longer carry the rules this record reads")
    settings = {
        name: frozen(protocol, name)
        for name in ("family_alpha", "bootstrap_resamples", "confirmatory_pairs",
                     "pilot_min_pairs", "pilot_max_pairs", "windows_per_execution",
                     "window_target_ms")
    }
    alpha, resamples, tail_draws = (
        settings["family_alpha"], settings["bootstrap_resamples"], float(tail.group(1))
    )

    workloads = [
        json.loads(line) for line in (ISSUE / "workload-routes.jsonl").read_text().splitlines()
    ]
    core = [row for row in workloads if row["track"] == "core-kernel"]
    core_families = {row["family"] for row in core}

    ledgers = []
    for path in tracked("**/*ledger*.jsonl"):
        entries = [json.loads(line) for line in (ROOT / path).read_text().splitlines() if line]
        families = sorted({entry["family"] for entry in entries})
        if len(families) > 1:
            raise SystemExit(f"{path} holds several families")
        if not families or families[0] not in core_families:
            continue
        spending = sum(entry["comparisons"] > 0 for entry in entries)
        reserved = sum(entry["comparisons"] for entry in entries)
        attempt_alpha, cells = admitted(alpha, resamples, tail_draws, spending + 1, reserved)
        ledgers.append(
            {
                "family": families[0],
                "ledger": path,
                "entries": len(entries),
                "reservations_spending_a_comparison": spending,
                "reserved_comparisons": reserved,
                "next_attempt": spending + 1,
                "next_attempt_alpha": attempt_alpha,
                "cells_the_next_attempt_admits": cells,
            }
        )
    ledgers.sort(key=lambda row: row["family"])

    durations = {}
    for row in core:
        record = json.loads((ROOT / row["receipt"]).read_text())
        for cell in record["cells"]:
            for pair in cell["pairs"]:
                for side in ("baseline", "candidate"):
                    elapsed = pair[side].get("elapsed_ns")
                    if elapsed:
                        durations.setdefault(row["label"], []).append(elapsed / 1e9)
    child = {
        label: {"children": len(values), "median_seconds": statistics.median(values)}
        for label, values in sorted(durations.items())
    }

    plan = json.loads((HERE / "planned-families.json").read_text())
    fresh_alpha, fresh_cells = admitted(alpha, resamples, tail_draws, 1, 0)
    pilot_pairs = plan["pilot_pairs"]
    if not settings["pilot_min_pairs"] <= pilot_pairs <= settings["pilot_max_pairs"]:
        raise SystemExit("the planned pilot pair count lies outside the frozen pilot range")
    planned = []
    for family in plan["families"]:
        cells = family["cells"]
        count = len(cells)
        if count > fresh_cells:
            raise SystemExit(f"{family['id']} plans more cells than a first attempt admits")
        piloted = sum(cell["role"] != "holdout" for cell in cells)
        corrected = fresh_alpha / count
        confirmation_children = count * int(settings["confirmatory_pairs"]) * 2
        pilot_children = piloted * pilot_pairs * 2
        planned.append(
            {
                "family": family["id"],
                "purpose": family["purpose"],
                "non_exploratory_cells": count,
                "holdout_cells": count - piloted,
                "small_input_cells": sum(cell["small_input"] for cell in cells),
                "attempt": 1,
                "attempt_alpha": fresh_alpha,
                "corrected_alpha": corrected,
                "expected_draws_per_tail": resamples * corrected / 2.0,
                "pilot_cells": piloted,
                "pilot_children": pilot_children,
                "confirmation_children": confirmation_children,
                "estimated_pilot_minutes":
                    pilot_children * child["pilot"]["median_seconds"] / 60.0,
                "estimated_confirmation_minutes":
                    confirmation_children * child["confirmation"]["median_seconds"] / 60.0,
            }
        )

    producer = (ROOT / package("gf2-core") / "benches/tuning_calibration.rs").read_text()
    grid = re.search(r"Self::MatvecSimdMinWords => vec!\[([\d, ]+)\]", producer)
    reserved = json.loads((HERE / "matvec-holdout-cells.json").read_text())
    sweep_grid = [int(value) for value in grid.group(1).split(",")] if grid else []
    if not sweep_grid or set(sweep_grid) & set(reserved["reserved_stride_words"]):
        raise SystemExit("the producer's matvec grid is absent or holds a reserved stride")

    seam = repository_files.document(ROOT, *SEAM_PROTOCOL)
    session = SESSION_BUDGET.search((ROOT / seam).read_text())
    if not session:
        raise SystemExit(f"{seam} no longer states its session budget")
    record = {
        "schema": "calibration-budget-v1",
        "issue": "63bad95d",
        "frozen_settings": settings,
        "minimum_tail_draws": tail_draws,
        "first_attempt_of_a_new_family": {
            "attempt_alpha": fresh_alpha, "cells_admitted": fresh_cells,
        },
        "core_kernel_ledgers": ledgers,
        "observed_child_seconds": child,
        "planned_families": planned,
        "planned_totals": {
            "non_exploratory_cells": sum(row["non_exploratory_cells"] for row in planned),
            "estimated_pilot_minutes": sum(row["estimated_pilot_minutes"] for row in planned),
            "estimated_confirmation_minutes":
                sum(row["estimated_confirmation_minutes"] for row in planned),
        },
        "offline_tuning_campaign": {
            "matvec_sweep_stride_words": sweep_grid,
            "reserved_stride_words": reserved["reserved_stride_words"],
            "protocol": seam,
            "session_budget_minutes": int(session.group(1).replace(",", "")) / 60.0,
        },
    }
    (ISSUE / "calibration-budget.json").write_text(json.dumps(record, indent=1) + "\n")

    def table(header, rows):
        lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
        return "\n".join(lines + ["| " + " | ".join(str(c) for c in row) + " |" for row in rows])

    cells = [
        [f"`{family['id']}`", f"`{cell['cell_id']}`", cell["role"], cell["objective"],
         cell["metric_kind"], cell["core_arm"], cell["cache_state"],
         f"{cell['baseline_build']} → {cell['candidate_build']}", cell["workload"],
         "yes" if cell["small_input"] else ""]
        for family in plan["families"] for cell in family["cells"]
    ]
    out = [
        "# Calibration budget and planned cells",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-calibration-budget.py`; "
        "[`calibration-budget.json`](calibration-budget.json) holds every value unrounded. "
        "The planned families are those of "
        "[`survey/planned-families.json`](survey/planned-families.json); no addendum is "
        "frozen from them.",
        "",
        "## Frozen settings and the P-20 tail rule",
        "",
        table(["Setting", "Value"],
              [[f"`{name}`", f"{value:g}"] for name, value in settings.items()]
              + [["minimum expected draws per bootstrap tail", f"{tail_draws:g}"],
                 ["attempt alpha of a first attempt", f"{fresh_alpha:g}"],
                 ["non-exploratory cells a first attempt admits", fresh_cells]]),
        "",
        "## Planned families",
        "",
        "Minutes are estimates: the child count times the median child duration the "
        "committed core-kernel receipts record for that stage, without process start, lock "
        "and finalization time.",
        "",
        table(
            ["Family", "Purpose", "Cells", "Holdout", "Small input", "Corrected alpha",
             "Draws per tail", "Pilot cells", "Pilot minutes (est.)",
             "Confirmation minutes (est.)"],
            [[f"`{row['family']}`", row["purpose"], row["non_exploratory_cells"],
              row["holdout_cells"], row["small_input_cells"], f"{row['corrected_alpha']:.6f}",
              f"{row['expected_draws_per_tail']:.1f}", row["pilot_cells"],
              f"{row['estimated_pilot_minutes']:.1f}",
              f"{row['estimated_confirmation_minutes']:.1f}"] for row in planned]
            + [["total", "", record["planned_totals"]["non_exploratory_cells"], "", "", "", "", "",
                f"{record['planned_totals']['estimated_pilot_minutes']:.1f}",
                f"{record['planned_totals']['estimated_confirmation_minutes']:.1f}"]],
        ),
        "",
        table(["Stage", "Children observed", "Median child seconds"],
              [[label, row["children"], f"{row['median_seconds']:.3f}"]
               for label, row in child.items()]),
        "",
        f"One session of the offline tuning campaign has a budget of "
        f"{record['offline_tuning_campaign']['session_budget_minutes']:g} minutes "
        f"(`{seam}`, budget section).",
        "",
        "## Build levels",
        "",
        table(["Arm identity", "RUSTFLAGS", "Selectors", "Competitor", "Definition"],
              [[f"`{name}`", f"`{level['rustflags']}`" if level["rustflags"] else "none",
                level["selectors"], level["external"], level.get("definition", "")]
               for name, level in plan["builds"].items()]),
        "",
        "## Excluded from the plan",
        "",
        table(["Subject", "Reason"], [[row["id"], row["reason"]] for row in plan["excluded"]]),
        "",
        "## Planned cells",
        "",
        table(["Family", "Cell", "Role", "Objective", "Metric", "Core arm", "Cache",
               "Builds", "Workload", "Small input"], cells),
        "",
        "## Ledgers of the measured core-kernel families",
        "",
        "`Admits` is the number of non-exploratory cells the family's next attempt can "
        "hold under P-20, given the reservations its ledger already carries.",
        "",
        table(["Family", "Ledger", "Entries", "Spending reservations", "Reserved comparisons",
               "Next attempt", "Admits"],
              [[f"`{row['family']}`", f"`{row['ledger']}`", row["entries"],
                row["reservations_spending_a_comparison"], row["reserved_comparisons"],
                row["next_attempt"], row["cells_the_next_attempt_admits"]] for row in ledgers]),
        "",
    ]
    (ISSUE / "calibration-budget.md").write_text("\n".join(out))
    print(f"{len(planned)} planned families, {len(ledgers)} core-kernel ledgers, "
          f"first attempt admits {fresh_cells} cells")


if __name__ == "__main__":
    main()
