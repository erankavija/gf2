#!/usr/bin/env python3
"""Project one committed receipt into the rows findings.md quotes (jit:c7113c5a).

Usage:
    ./summarize-receipt.py <receipt-directory> [--table cells|conversion|flagged|products|onset]

`cells` emits one markdown row per cell: the median-of-medians speedup of the
baseline over the candidate, its bootstrap interval at the family confidence,
the pair count behind it, the decision and outcome the acceptance tool
recorded, and the runtime path each arm selected. `conversion` emits the setup,
packing, batch-fill, dispatch and unpacking costs both arms report outside
their timed windows. `flagged` recomputes the outlier count both pooled across
arms, as the protocol's rule does, and separately within each arm, beside the
widest single window each arm produced as a multiple of that arm's own median.
`products` divides each arm's measured call by the number of one-word
coefficient products the schoolbook definition of that cell's operation
performs, so cells of different sizes are comparable on one scale. `onset`
solves the pooled flagged-window rule for the arm separation at which it starts
to flag, and checks that closed form against the count the rule actually
produced.

Every number comes from the receipt and the acceptance summary in that
directory, so re-running the script after a new receipt refreshes the rows
rather than restating them.
"""

import json
import pathlib
import statistics
import sys


def load(directory):
    root = pathlib.Path(directory)
    receipt = json.loads((root / "receipt.json").read_text())
    summary = json.loads((root / "acceptance-summary.json").read_text())
    plan = json.loads((root / "plan.json").read_text())
    return receipt, summary, plan


def arm_values(cell, side):
    return [
        window["elapsed_ns"] / window["calls"]
        for pair in cell["pairs"]
        for window in pair[side]["windows"]
    ]


def verdicts(summary):
    rows = summary.get("cells") or summary.get("verdicts") or []
    return {row["cell_id"]: row for row in rows}


def selected(cell, side):
    paths = {pair[side].get("selected_path") for pair in cell["pairs"]}
    paths.discard(None)
    return ";".join(sorted(paths)) if paths else "(none reported)"


def conversion_median(cell, side):
    reports = [pair[side].get("conversion") for pair in cell["pairs"]]
    reports = [report for report in reports if report]
    if not reports:
        return None
    return {
        key: int(statistics.median([report[key] for report in reports]))
        for key in reports[0]
    }


def format_ns(value):
    for scale, unit in ((1e9, "s"), (1e6, "ms"), (1e3, "us")):
        if value >= scale:
            return f"{value / scale:.3g} {unit}"
    return f"{value:.3g} ns"


def cells_table(receipt, summary):
    rows = verdicts(summary)
    print(
        "| Cell | gf2 median | comparator median | speedup of medians | interval "
        "| pairs | decision | outcome | gf2 path |"
    )
    print("|---|---:|---:|---:|---|---:|---|---|---|")
    for cell in receipt["cells"]:
        identifier = cell["cell_id"]
        row = rows.get(identifier, {})
        if not cell["pairs"]:
            print(
                f"| `{identifier}` | unavailable | unavailable | | | 0 | "
                f"| {row.get('outcome', '')} | |"
            )
            continue
        base = statistics.median(arm_values(cell, "baseline"))
        cand = statistics.median(arm_values(cell, "candidate"))
        interval = row.get("interval") or {}
        low = interval.get("lower")
        high = interval.get("upper")
        speedup = interval.get("estimate", base / cand)
        span = (
            f"[{low:.4g}, {high:.4g}] at {interval['confidence']:.4g}"
            if low is not None
            else "n/a"
        )
        print(
            f"| `{identifier}` | {format_ns(base)} | {format_ns(cand)} | {speedup:.4g} | "
            f"{span} | {row.get('pairs', len(cell['pairs']))} | "
            f"{row.get('decision', '')} | {row.get('outcome', '')} | "
            f"`{selected(cell, 'baseline')}` |"
        )


def conversion_table(receipt):
    print("| Cell | arm | setup | pack | batch fill | dispatch | unpack |")
    print("|---|---|---:|---:|---:|---:|---:|")
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            costs = conversion_median(cell, side)
            if costs is None:
                continue
            print(
                f"| `{cell['cell_id']}` | `{cell[f'{side}_arm']}` | {costs['setup_ns']} ns | "
                f"{costs['pack_ns']} ns | {costs['batch_fill_ns']} ns | "
                f"{costs['dispatch_ns']} ns | {costs['unpack_ns']} ns |"
            )


def flagged_table(receipt):
    factor = receipt["settings"]["flagged_window_factor"]
    print(
        "| Cell | ratio of medians | pooled flagged | per-arm flagged "
        "| widest window within its arm | windows |"
    )
    print("|---|---:|---:|---:|---:|---:|")
    for cell in receipt["cells"]:
        if not cell["pairs"]:
            continue
        base = arm_values(cell, "baseline")
        cand = arm_values(cell, "candidate")
        pooled = base + cand
        centre = statistics.median(pooled)
        pooled_flagged = sum(1 for value in pooled if value > factor * centre)
        per_arm = sum(
            1
            for values in (base, cand)
            for value in values
            if value > factor * statistics.median(values)
        )
        widest = max(
            max(values) / statistics.median(values) for values in (base, cand)
        )
        ratio = statistics.median(base) / statistics.median(cand)
        print(
            f"| `{cell['cell_id']}` | {ratio:.4g} | {pooled_flagged} | {per_arm} "
            f"| {widest:.3g} | {len(pooled)} |"
        )


def coefficient_products(case, workers):
    """One-word coefficient products the schoolbook definition performs per call.

    A `words`-word long product is `words * words` of them, and a batch or dot
    product of `count` one-word elements is `count`. Every worker performs
    `inner` calls' worth inside one logical call, so the multicore cells scale
    by the observed worker count. gf2x avoids most of these products by
    subquadratic recursion: for its arm the figure is a normalised rate on the
    schoolbook scale, not a count of the products gf2x performs.
    """
    inner = max(case.get("inner", 1), 1)
    if case["kind"] == "poly-mul":
        per_call = case["words"] * case["words"]
    else:
        per_call = case["count"]
    return per_call * inner * workers


def products_table(receipt, plan):
    cases = {cell["cell_id"]: cell["case"] for cell in plan["cells"]}
    print(
        "| Cell | operand words | workers | schoolbook products per call "
        "| gf2 ns per product | comparator ns per product |"
    )
    print("|---|---:|---:|---:|---:|---:|")
    for cell in receipt["cells"]:
        case = cases.get(cell["cell_id"])
        if not cell["pairs"] or case is None:
            continue
        workers = cell["pairs"][0]["baseline"]["workers_observed"]
        products = coefficient_products(case, workers)
        base = statistics.median(arm_values(cell, "baseline"))
        cand = statistics.median(arm_values(cell, "candidate"))
        words = case.get("words", 1)
        print(
            f"| `{cell['cell_id']}` | {words} | {workers} | {products} "
            f"| {base / products:.4g} | {cand / products:.4g} |"
        )


def onset_table(receipt):
    """Where the pooled flagged-window rule starts to flag, per cell.

    With `n` windows from each arm and the two arms' windows disjoint, the
    pooled median is the mean of the two middle order statistics, one from each
    cluster, so the rule's threshold is exactly

        T = 2 * median(pooled) = max(fast) + min(slow)

    and a slow window is flagged when it exceeds T. No fast window ever can,
    since T is at least max(fast). Writing `R` for the ratio of arm medians,
    `D` for the slow arm's full window range relative to its own median and `a`
    for the fast arm's excess of maximum over median, no window is flagged while

        R <= (1 + a) / D,

    which this table reports as the critical ratio. The rule therefore starts to
    misfire sooner on cells whose slower arm is noisier, which is the opposite of
    what a reader expects a stability rule to do.
    """
    factor = receipt["settings"]["flagged_window_factor"]
    print(
        "| Cell | arm ratio | slow-arm range | fast-arm excess | critical ratio "
        "| pooled flagged | closed form |"
    )
    print("|---|---:|---:|---:|---:|---:|---:|")
    for cell in receipt["cells"]:
        if not cell["pairs"]:
            continue
        base = arm_values(cell, "baseline")
        cand = arm_values(cell, "candidate")
        slow, fast = (
            (base, cand)
            if statistics.median(base) > statistics.median(cand)
            else (cand, base)
        )
        slow_median = statistics.median(slow)
        fast_median = statistics.median(fast)
        ratio = slow_median / fast_median
        spread = (max(slow) - min(slow)) / slow_median
        excess = (max(fast) - fast_median) / fast_median
        critical = (1 + excess) / spread
        pooled = sum(
            1
            for value in slow + fast
            if value > factor * statistics.median(slow + fast)
        )
        predicted = sum(1 for value in slow if value > max(fast) + min(slow))
        print(
            f"| `{cell['cell_id']}` | {ratio:.2f} | {spread:.4f} | {excess:.4f} "
            f"| {critical:.1f} | {pooled} | {predicted} |"
        )


def main():
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    receipt, summary, plan = load(sys.argv[1])
    table = sys.argv[3] if len(sys.argv) > 3 and sys.argv[2] == "--table" else "cells"
    if table == "cells":
        cells_table(receipt, summary)
    elif table == "conversion":
        conversion_table(receipt)
    elif table == "flagged":
        flagged_table(receipt)
    elif table == "products":
        products_table(receipt, plan)
    elif table == "onset":
        onset_table(receipt)
    else:
        print(f"unknown table {table}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
