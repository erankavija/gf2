#!/usr/bin/env python3
"""Apply the frozen resolution rule to the lane-threshold confirmation (jit:4337c02e).

The rule is the *Resolution rule* section of the lane-threshold addendum. It
reads the confirmation's acceptance summary and the confirmation addendum that
receipt pins, and writes `outcome.json` and `outcome.md` beside the addendum
document. `--self-test` checks the rule on constructed summaries and writes
nothing.
"""

import argparse
import hashlib
import json

from locate import CONFIRMATION, FAMILY, HERE, ROOT, SELECTOR, repo_artifacts

OUTPUT = HERE.parent / "outcome.json"
RENDERED = HERE.parent / "outcome.md"


def resolve(verdict, cells, equivalence):
    """The rule's value and deciding step.

    `cells` holds one mapping per confirmatory cell with its `stride`,
    `objective`, `outcome`, `decision` and interval `upper` bound. Returns
    `(value, step)`; `value` is `None` for *no change*.
    """
    if verdict != "accepted":
        return None, "validity: the acceptance summary's verdict is not accepted"
    controls = [cell for cell in cells if cell["objective"] == "non-regression"]
    lanes = sorted(
        (cell for cell in cells if cell["objective"] == "improvement"),
        key=lambda cell: cell["stride"],
    )
    if len(controls) != 1 or len(controls) + len(lanes) != len(cells) or not lanes:
        raise ValueError("the confirmation does not hold one control and the lane cells")
    control = controls[0]
    bounded = (
        control["outcome"] == "pass"
        and control["decision"] == "not-worse"
        and control["upper"] is not None
        and control["upper"] <= equivalence
    )
    if not bounded:
        return None, "control: control failed"
    leading = 0
    for cell in lanes:
        if cell["outcome"] != "pass":
            break
        leading += 1
    if leading == 0:
        return None, "value: the first lane stride does not pass"
    return lanes[leading - 1]["stride"] + 1, f"value: leading run of {leading} passing lane strides"


def self_test():
    def cell(stride, outcome, objective="improvement", decision="improved", upper=1.2):
        return {
            "stride": stride, "objective": objective, "outcome": outcome,
            "decision": decision, "upper": upper,
        }

    def control(outcome="pass", decision="not-worse", upper=1.01):
        return cell(7, outcome, "non-regression", decision, upper)

    def lanes(*outcomes):
        return [cell(stride, outcome) for stride, outcome in zip((8, 9, 63, 64, 65), outcomes)]

    passing = ["pass"] * 5
    cases = [
        (("accepted", lanes(*passing) + [control()], 1.05), 66),
        (("rejected", lanes(*passing) + [control()], 1.05), None),
        (("accepted", lanes("pass", "pass", "pass", "not-material", "pass") + [control()], 1.05), 64),
        (("accepted", lanes("pass", "pass", "inconclusive", "pass", "pass") + [control()], 1.05), 10),
        (("accepted", lanes("pass", "fail", "pass", "pass", "pass") + [control()], 1.05), 9),
        (("accepted", lanes("not-material", "pass", "pass", "pass", "pass") + [control()], 1.05), None),
        (("accepted", lanes("pass", "pass", "pass", "pass", "unstable") + [control()], 1.05), 65),
        (("accepted", lanes(*passing) + [control(upper=1.06)], 1.05), None),
        (("accepted", lanes(*passing) + [control(decision="improved")], 1.05), None),
        (("accepted", lanes(*passing) + [control(outcome="fail", decision="regressed")], 1.05), None),
        (("accepted", lanes(*passing) + [control(outcome="not-confirmatory")], 1.05), None),
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
    equivalence = addendum["effect"]["equivalence_margin"]
    value, step = resolve(summary["verdict"], cells, equivalence)
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
            "equivalence_margin": equivalence,
            "worthwhile_speedup": addendum["effect"]["worthwhile_speedup"],
        },
        "cells": sorted(cells, key=lambda cell: cell["stride"]),
    }
    OUTPUT.write_text(json.dumps(document, indent=2) + "\n")

    def shown(number):
        return "" if number is None else f"{number:.4f}"

    lines = [
        "# Matvec lane-threshold outcome",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        f"Generated by `survey/make-outcome.py` from the acceptance summary of `{CONFIRMATION}`",
        f"(`{summary_path}`); [`outcome.json`](outcome.json) holds every value unrounded.",
        "",
        f"**Outcome for `{SELECTOR}`: "
        + ("no change" if value is None else f"threshold value {value} words")
        + f".** Deciding step of the frozen rule: {step}.",
        "",
        "A speedup above one favours the scalar lane. The interval is the percentile bootstrap",
        "at the confidence shown.",
        "",
        "| Cell | Stride (words) | Objective | Pairs | Speedup | Interval | Confidence | Decision | Outcome |",
        "|---|---:|---|---:|---:|---|---:|---|---|",
    ]
    for cell in document["cells"]:
        lines.append(
            f"| `{cell['cell_id']}` | {cell['stride']} | {cell['objective']} | {cell['pairs']} "
            f"| {shown(cell['estimate'])} | [{shown(cell['lower'])}, {shown(cell['upper'])}] "
            f"| {shown(cell['confidence'])} | {cell['decision']} | {cell['outcome']} |"
        )
    RENDERED.write_text("\n".join(lines) + "\n")
    print(f"{OUTPUT.relative_to(ROOT)}: {document['outcome']}")


if __name__ == "__main__":
    main()
