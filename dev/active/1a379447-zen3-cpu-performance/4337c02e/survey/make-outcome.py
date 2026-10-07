#!/usr/bin/env python3
"""Apply the frozen resolution rule to the lane-threshold confirmation (jit:4337c02e).

The rule is the *Resolution rule* section of the lane-threshold addendum. It
reads the confirmation's acceptance summary and the confirmation addendum that
receipt pins, and writes `outcome.json` and `outcome-record.md` beside the
addendum document. The record also states what P-20's tail rule admits for a
further confirmation on the family ledger as committed. `--self-test` checks
the rule on constructed summaries and writes nothing.
"""

import argparse
import hashlib
import json
import sys

sys.dont_write_bytecode = True

from budget import budget  # noqa: E402
from locate import CONFIRMATION, FAMILY, HERE, ROOT, SELECTOR, repo_artifacts  # noqa: E402

OUTPUT = HERE.parent / "outcome.json"
RENDERED = HERE.parent / "outcome-record.md"


def resolve(verdict, cells):
    """The rule's value and deciding step.

    `cells` holds one mapping per confirmatory cell with its `stride`,
    `objective` and `outcome`. Returns `(value, step)`; `value` is `None` for
    *no change*.
    """
    if verdict != "accepted":
        return None, "validity: the acceptance summary's verdict is not accepted"
    lanes = sorted(cells, key=lambda cell: cell["stride"])
    if not lanes or any(cell["objective"] != "improvement" for cell in lanes):
        raise ValueError("the confirmation does not hold improvement lane cells only")
    leading = 0
    for cell in lanes:
        if cell["outcome"] != "pass":
            break
        leading += 1
    if leading == 0:
        return None, "value: the first lane stride does not pass"
    return lanes[leading - 1]["stride"] + 1, f"value: leading run of {leading} passing lane strides"


def self_test():
    def lanes(*outcomes):
        return [
            {"stride": stride, "objective": "improvement", "outcome": outcome}
            for stride, outcome in zip((8, 9, 32, 63, 64, 65), outcomes)
        ]

    passing = ["pass"] * 6
    cases = [
        (("accepted", lanes(*passing)), 66),
        (("rejected", lanes(*passing)), None),
        (("accepted", lanes("pass", "pass", "pass", "pass", "not-material", "pass")), 64),
        (("accepted", lanes("pass", "pass", "pass", "inconclusive", "pass", "pass")), 33),
        (("accepted", lanes("pass", "pass", "fail", "pass", "pass", "pass")), 10),
        (("accepted", lanes("pass", "not-confirmatory", "pass", "pass", "pass", "pass")), 9),
        (("accepted", lanes("not-material", "pass", "pass", "pass", "pass", "pass")), None),
        (("accepted", lanes("pass", "pass", "pass", "pass", "pass", "unstable")), 65),
        (("accepted", list(reversed(lanes("pass", "pass", "pass", "pass", "fail", "fail")))), 64),
    ]
    for arguments, expected in cases:
        value, step = resolve(*arguments)
        assert value == expected, (arguments, value, step)
    print(f"make-outcome self-test: {len(cases)} cases")


def sha256(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    if parser.parse_args().self_test:
        self_test()
        return

    receipt = repo_artifacts.receipt(CONFIRMATION)
    summary_path = receipt / "acceptance-summary.json"
    addendum_path = repo_artifacts.addendum(CONFIRMATION)
    summary = json.loads((ROOT / summary_path).read_bytes())
    addendum = json.loads((ROOT / addendum_path).read_bytes())
    if summary["label"] != "confirmation" or addendum["family"]["id"] != FAMILY:
        raise SystemExit(f"{summary_path} is not the confirmation of {FAMILY}")
    declared = {cell["cell_id"]: cell for cell in addendum["cells"]}
    cells = []
    for row in summary["cells"]:
        interval = row.get("interval") or {}
        cells.append(
            {
                "cell_id": row["cell_id"],
                "stride": declared[row["cell_id"]]["workload"]["size"]["words"],
                "objective": row["objective"],
                "pairs": row["pairs"],
                "estimate": interval.get("estimate"),
                "lower": interval.get("lower"),
                "upper": interval.get("upper"),
                "confidence": interval.get("confidence"),
                "decision": row["decision"],
                "outcome": row["outcome"],
            }
        )
    if {cell["cell_id"] for cell in cells} != set(declared):
        raise SystemExit(f"{summary_path} does not report every declared cell")
    value, step = resolve(summary["verdict"], cells)
    # One further cell is the smallest confirmation the ledger could be asked for.
    further = budget(addendum, 1)
    document = {
        "schema": "matvec-lane-threshold-outcome-v1",
        "issue": "4337c02e",
        "family": FAMILY,
        "selector": SELECTOR,
        "outcome": "no-change" if value is None else "threshold",
        "threshold_words": value,
        "decided_by": step,
        "confirmation": {
            "campaign": CONFIRMATION,
            "summary": str(summary_path),
            "summary_sha256": sha256(summary_path),
            "receipt_sha256": summary["receipt_sha256"],
            "addendum": str(addendum_path),
            "addendum_sha256": sha256(addendum_path),
            "verdict": summary["verdict"],
            "qualifies": summary["qualifies"],
            "findings": summary["findings"],
            "equivalence_margin": addendum["effect"]["equivalence_margin"],
            "worthwhile_speedup": addendum["effect"]["worthwhile_speedup"],
        },
        "cells_confirmatory": summary["qualifies"],
        "cells": sorted(cells, key=lambda cell: cell["stride"]),
        "further_confirmation": {
            "ledger": addendum["family_wise"]["ledger_path"],
            "ledger_entries": further["entries"],
            "spending_reservations": further["spending"],
            "attempt": further["attempt"],
            "attempt_alpha": further["attempt_alpha"],
            "reserved_comparisons": further["comparisons"] - 1,
            "bootstrap_resamples": further["resamples"],
            "required_draws_per_tail": further["tail_draws"],
            "one_cell_corrected_alpha": further["corrected"],
            "one_cell_draws_per_tail": further["draws"],
            "admissible_cells": max(further["capacity"], 0),
        },
    }
    OUTPUT.write_text(json.dumps(document, indent=2) + "\n")

    admitted = document["further_confirmation"]
    lines = [
        "# Matvec lane-threshold outcome record",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        f"Generated by `survey/make-outcome.py` from the acceptance summary of `{CONFIRMATION}`",
        f"(`{summary_path}`) and the family ledger `{admitted['ledger']}`;",
        "[`outcome.json`](outcome.json) holds every value unrounded.",
        "",
        f"**Outcome for `{SELECTOR}`: "
        + ("no change" if value is None else f"threshold value {value} words")
        + f".** Deciding step of the frozen rule: {step}.",
        "",
        f"Acceptance verdict `{summary['verdict']}`, qualifies `{json.dumps(summary['qualifies'])}`. Findings:",
        "",
    ]
    lines += [
        f"- {finding['rule']} {finding['severity']}"
        + (f" (`{finding['cell']}`)" if finding.get("cell") else "")
        + f": {finding['message']}"
        for finding in summary["findings"]
    ] or ["- none"]
    lines += [
        "",
        "## Recorded cell outcomes",
        "",
        "The outcomes are confirmatory only when the summary qualifies.",
        "",
        "| Cell | Stride (words) | Pairs | Decision | Outcome | Confirmatory |",
        "|---|---:|---:|---|---|---|",
    ]
    for cell in document["cells"]:
        lines.append(
            f"| `{cell['cell_id']}` | {cell['stride']} | {cell['pairs']} | {cell['decision']} "
            f"| {cell['outcome']} | {'yes' if summary['qualifies'] else 'no'} |"
        )
    lines += [
        "",
        "## A further confirmation under P-20",
        "",
        "| Quantity | Value |",
        "|---|---|",
        f"| Ledger entries, spending reservations | {admitted['ledger_entries']}, "
        f"{admitted['spending_reservations']} |",
        f"| Next attempt | {admitted['attempt']} |",
        f"| Attempt alpha | {admitted['attempt_alpha']:.6g} |",
        f"| Comparisons already reserved | {admitted['reserved_comparisons']} |",
        f"| Bootstrap resamples | {admitted['bootstrap_resamples']} |",
        f"| Expected draws per tail P-20 requires | {admitted['required_draws_per_tail']} |",
        f"| Corrected alpha with one further cell | {admitted['one_cell_corrected_alpha']:.6g} |",
        f"| Expected draws per tail with one further cell | {admitted['one_cell_draws_per_tail']:.4g} |",
        f"| Further confirmatory cells admitted | {admitted['admissible_cells']} |",
    ]
    RENDERED.write_text("\n".join(lines) + "\n")
    print(f"{OUTPUT.relative_to(ROOT)}: {document['outcome']}")


if __name__ == "__main__":
    main()
