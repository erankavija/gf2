#!/usr/bin/env python3
"""Project the integrated route evidence using the shared outcome readers.

Invoke from the worktree root with python3 -B; --check checks the saved report.
"""

import argparse
from pathlib import Path

from locate import HERE, ROOT, load

tables = load("survey/make-outcome-tables.py", "route_outcome_tables")
shared = tables.outcome
OUTPUT = HERE / "outcome.md"


def link(path, label):
    return f"[{label}]({shared.link(OUTPUT, path)})"


def campaigns():
    records = {}
    for route in ("core", "decoder"):
        for stage in ("pilot", "confirmation"):
            directory = HERE / route / f"31727c57-{route}-{stage}"
            if route == "decoder":
                record = tables.campaign(directory)
            else:
                tables.verifier.verify(str(directory / "execution.log"),
                                       str(directory / "receipt.json"),
                                       str(directory / "plan.json"))
                summary, _, cells = shared.outcomes(directory)
                record = {"directory": directory, "summary": summary, "cells": cells}
            record["receipt"] = shared.read(directory / "receipt.json")
            record["addendum"] = shared.read(directory / record["receipt"]["addendum"]["snapshot"])
            record["plan"] = shared.read(directory / "plan.json")
            records[route, stage] = record
    return records


def counts(campaign):
    roles = [cell["summary"]["role"] for cell in campaign["cells"]]
    return roles.count("confirmatory"), roles.count("holdout")


def result_rows(campaign):
    rows = []
    for cell in campaign["cells"]:
        recorded, planned = cell["summary"], cell["planned"]
        interval, margins = recorded["interval"], recorded["margins"]
        builds = [campaign["plan"]["arms"][planned[f"{side}_arm"]]["build"]
                  for side in ("baseline", "candidate")]
        rows.append([
            f"`{recorded['cell_id']}`", recorded["role"],
            " → ".join(builds), recorded["core_arm"], str(recorded["pairs"]),
            f"{interval['estimate']:.4f}", f"[{interval['lower']:.4f}, {interval['upper']:.4f}]",
            f"{margins['improvement']:g} / {margins['equivalence']:g}",
            f"`{recorded['decision']}`", f"`{recorded['outcome']}`",
            "; ".join(recorded["unresolved_settings"]) or recorded["reason"] or "—",
        ])
    return rows


def measurement_rows(campaign):
    rows = []
    measured = {cell["cell_id"]: cell for cell in campaign["receipt"]["cells"]}
    for cell, side in tables.arm_rows(campaign["cells"]):
        identifier = cell["summary"]["cell_id"]
        time, _ = tables.per_call(side)
        _, _, _, coverage = tables.median_interval(side["times"])
        fill = "unmeasured; see receipt values"
        if side["batch_fill"] is not None:
            fill = tables.spread(side["batch_fill"])[0]
        elif all(pair[side["side"]]["conversion"]["batch_fill_ns"] == 0
                 for pair in measured[identifier]["pairs"]):
            fill = "unmeasured; recorded 0"
        frames = tables.frames_per_call(cell, side)
        rows.append([
            f"`{identifier}`", side["side"], tables.workers(side), str(cell["batch"]),
            str(frames), str(len(side["times"])), f"{coverage:.9f}", time,
            tables.throughput(side, frames), fill,
        ])
    return rows


def stage_record(campaign, stage):
    summary, directory = campaign["summary"], campaign["directory"]
    family = summary["family"]
    return [
        f"### {stage.capitalize()}", "",
        f"Sources: {link(directory / 'receipt.json', 'receipt')}, "
        f"{link(directory / 'acceptance-summary.json', 'acceptance JSON')}, "
        f"{link(directory / 'acceptance-summary.md', 'acceptance Markdown')}, "
        f"{link(directory / 'plan.json', 'executed plan')}, "
        f"{link(directory / 'execution.log', 'execution log')}.", "",
        f"Verdict `{summary['verdict']}`; qualifies for production selection "
        f"`{str(summary['qualifies']).lower()}`; sessions {summary['sessions']}; "
        f"resumed `{str(summary['resumed']).lower()}`. "
        f"Receipt SHA-256 `{summary['receipt_sha256']}`.", "",
        f"Family alpha {family['family_alpha']}; attempt alpha {family['attempt_alpha']}; "
        f"corrected alpha {family['corrected_alpha']}; per-comparison confidence "
        f"{family['per_comparison_confidence']}; bootstrap resamples {family['bootstrap_resamples']}. "
        "The table preserves the summary's decision and outcome tokens; numeric estimates "
        "and interval endpoints use its Markdown precision.", "",
        *shared.table(["Cell", "Role", "Baseline → candidate build", "Core arm", "Pairs",
                       "Speedup", "Interval", "Improvement / equivalence margins", "Decision",
                       "Outcome", "Unresolved setting or reason"], result_rows(campaign)), "",
        *(summary["findings"] or ["Findings: none."]), "",
    ]


def margin_record(route, pilot, confirmation):
    directory = HERE / route / "confirmation"
    derivation = (directory / "derivation.txt").read_text()
    ruling = [line for line in derivation.splitlines()
              if line.startswith(("frozen measurement resolution", "replaced "))]
    effect = confirmation["addendum"]["effect"]
    return [
        "### Canonical freeze and margin ruling", "",
        f"The {link(directory / 'addendum.json', 'frozen addendum')}, "
        f"{link(directory / 'plan.json', 'frozen plan')} and "
        f"{link(directory / 'derivation.txt', 'derivation')} record the canonical freeze. "
        "The freezer refuses the pre-pilot margins below because they do not strictly exceed "
        "one plus the pilot resolution. The invoker ruling of 2026-10-28, before any "
        "confirmation sample, authorizes replacement through the freezer's margin options "
        "with the smallest two-decimal value strictly above that bound. "
        "This is the ruling recorded in the frozen addendum's rationale.", "",
        "Derivation excerpt (verbatim):", "", "```text", *ruling, "```", "",
        f"Frozen worthwhile speedup {effect['worthwhile_speedup']:g}; equivalence margin "
        f"{effect['equivalence_margin']:g}; measurement resolution "
        f"{effect['measurement_resolution']:g}. "
        f"The {link(pilot['directory'] / pilot['receipt']['addendum']['snapshot'], 'pilot addendum snapshot')} "
        "retains the refused values. These refusals remain part of the outcome.", "",
    ]


def render(records):
    lines = [
        "# Integrated core and decoder route outcome", "",
        "> **Diátaxis Type:** Reference", "",
        "Outcome record for `@/issue/31727c57`, families FI-1 and FI-2 of the "
        f"{link(HERE.parent / '1362381c/design.md', 'integrated scorecard design')}. "
        "Both families have completed pilot, canonical freeze and independent confirmation "
        "under frozen protocol v4 on the integrated release builds. "
        "The confirmation acceptance verdicts are recorded below. "
        "The tuned decoder arm's separate batch-fill latency is unmeasured in the committed "
        "evidence; REQ-04 has that evidence gap.", "",
        f"Generated by {link(HERE / 'make_outcome.py', 'make_outcome.py')} using the shared "
        f"{link(Path(tables.__file__), 'outcome table reader')} "
        "and its collection outcome reader and median intervals. From the worktree root:", "",
        "```sh", f"python3 -B {(HERE / 'make_outcome.py').relative_to(ROOT)}",
        f"python3 -B {(HERE / 'make_outcome.py').relative_to(ROOT)} --check", "```", "",
        "The generator verifies each execution log with the shared campaign verifier. "
        "Receipts pin the protocol, measurement contract, addendum, producing inputs, arm "
        "digests, toolchain and observed host; their `inputs/` directories carry the pinned "
        "snapshots. Executed plans identify each arm. Speedup is baseline median time per "
        "call divided by candidate median time per call, with the acceptance summary's "
        "percentile-bootstrap interval. Values above one favour the candidate. "
        "All declared objectives are non-regression; `improved` and `not-worse` decisions "
        "can both yield `pass`. Pilot outcomes remain exploratory and do not qualify for "
        "production selection. Confirmation claims apply only to selected and holdout cells.", "",
    ]
    for route in ("core", "decoder"):
        pilot, confirmation = (records[route, stage] for stage in ("pilot", "confirmation"))
        confirmatory, holdout = counts(confirmation)
        summary = confirmation["summary"]
        passed = sum(cell["outcome"] == "pass" for cell in summary["cells"])
        route_dir = HERE / route
        lines += [
            f"## {route.capitalize()} family", "",
            f"`{summary['family']['family_id']}`: confirmation verdict `{summary['verdict']}`; "
            f"{passed}/{len(summary['cells'])} cells record `pass`. "
            f"{confirmatory} confirmatory + {holdout} holdout = {confirmatory + holdout} cells, "
            f"within the {link(HERE.parent / '1362381c/design.md', 'P-20 first-attempt bound')} "
            "of six. The family has a fresh "
            f"{link(route_dir / 'ledger.jsonl', 'ledger')}; its pilot spends zero comparisons. "
            f"The {link(route_dir / 'holdout-cells.json', 'holdout declaration')} is committed "
            "before the pilot and supplies the holdouts named in the freeze derivation.", "",
        ]
        if route == "core":
            lines += [
                "Core cells measure one core because the kernels execute on one thread "
                "(design D-5). Native cells compare native with tuned-portable; portable "
                "cells compare tuned-portable with conservative-portable. The "
                f"{link(route_dir / 'arm-builds.json', 'build record')} pins the release arms. "
                "The pilot transpose decision is `inconclusive`, with outcome `pilot`; "
                "the table retains it and the pilot estimates below one.", "",
            ]
        else:
            lines += [
                "Each cell compares the tuned-profile arm under the committed coding envelope "
                "with the conservative single-frame arm. The pilot covers both codes at one "
                "worker with one-frame and full waves, six and twelve physical cores and "
                "twenty-four logical CPUs. The frozen confirmation spans those worker counts "
                "across the selected codes, plus the NR partial-wave holdouts. The per-cell "
                "tables distinguish pilot coverage from confirmation; they retain pilot "
                "estimates below one. Full-wave cells omitted by the freeze remain exploratory.", "",
            ]
        lines += stage_record(pilot, "pilot")
        lines += margin_record(route, pilot, confirmation)
        lines += stage_record(confirmation, "confirmation")
        if route == "decoder":
            lines += [
                "### Throughput and separate batch-fill latency", "",
                "Each row is one arm of a decoder cell. Time per call and batch fill are in "
                "nanoseconds: median [order-statistic interval] over the stated executions, "
                "with the coverage shown. These descriptive intervals use the shared median "
                "estimator and are separate from the acceptance intervals above. Throughput "
                "is frames per call divided by time per call, with interval endpoints inverted. "
                "A call dispatches one batch per worker; throughput includes the whole "
                "consumer's conversion, routing, decoding and extraction.", "",
                "Batch fill uses each execution's `conversion.batch_fill_ns`. Conservative "
                "arm diagnostics declare nine stage probes and the median statistic: the "
                "decoder call at zero iterations includes message initialization and the "
                "terminal syndrome pass. It measures decoder preparation rather than "
                "waiting for frames to arrive. Tuned-arm diagnostics declare no stage probes; "
                "every recorded batch-fill field is zero. Under the shared reader's rule "
                "those entries are unmeasured, and the recorded zeros provide no latency "
                "estimate or confidence interval. REQ-04 therefore lacks a measured "
                "tuned-arm batch-fill result in every decoder cell despite accepted "
                "throughput outcomes.", "",
            ]
            for stage, campaign in (("Pilot", pilot), ("Confirmation", confirmation)):
                lines += [f"{stage} arm measurements (basis: its receipt and execution log above):", "",
                          *shared.table(["Cell", "Side", "Workers", "Batch per worker",
                                         "Frames per call", "Executions", "Coverage", "Time per call, ns",
                                         "Throughput, frames/s", "Batch fill, ns"],
                                        measurement_rows(campaign)), ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    text = render(campaigns())
    if args.check:
        if OUTPUT.read_text() != text:
            parser.exit(1, "outcome.md differs from its evidence projection\n")
    else:
        OUTPUT.write_text(text)
    print(f"{OUTPUT.relative_to(ROOT)}: {'current' if args.check else 'written'}")


if __name__ == "__main__":
    main()
