#!/usr/bin/env python3
"""Project the c7113c5a protocol-v3 receipts into Markdown tables.

Usage: summarize-v3.py <output.md> <receipt-dir>...

Every number below is read from a receipt, its acceptance summary or the v1
confirmation receipt named on the command line; nothing is transcribed. Each
receipt directory must hold receipt.json and acceptance-summary.json written by
benchmark-acceptance. Medians are over the per-execution values of all pairs;
the speedup and interval are the acceptance tool's recomputation.
"""

import hashlib
import json
import re
import statistics
import sys


def load(directory):
    with open(f"{directory}/receipt.json", "rb") as handle:
        raw = handle.read()
    with open(f"{directory}/acceptance-summary.json") as handle:
        summary = json.load(handle)
    return json.loads(raw), summary, hashlib.sha256(raw).hexdigest()


def fmt(value, digits=4):
    if value is None:
        return "-"
    return f"{value:.{digits}g}"


def ns(value):
    for unit, scale in (("ms", 1e6), ("us", 1e3)):
        if value >= scale:
            return f"{value / scale:.4g} {unit}"
    return f"{value:.4g} ns"


def arm_median(cell, side):
    return statistics.median(pair[side]["ns_per_call"] for pair in cell["pairs"])


def execution_flags(cell, factor=2.0):
    """Windows at or above `factor` times their own execution's median, the
    protocol-v3 rule of `abtest::flagged_windows`, recounted from raw windows."""
    flagged = total = 0
    for pair in cell["pairs"]:
        for side in ("baseline", "candidate"):
            values = [w["elapsed_ns"] / w["calls"] for w in pair[side]["windows"]]
            center = statistics.median(values)
            flagged += sum(1 for value in values if value >= factor * center)
            total += len(values)
    return flagged, total


def paths(cell, side):
    found = sorted({pair[side].get("selected_path") or "-" for pair in cell["pairs"]})
    return found


def overview(rows):
    lines = ["## Campaigns", "",
             "| Receipt | Label | Verdict | Qualifies | Sessions | Ledger comparisons | Attempt alpha | Per-comparison confidence | Findings | Receipt SHA-256 |",
             "|---|---|---|---|---:|---:|---:|---:|---|---|"]
    for directory, (receipt, summary, digest) in rows:
        rules = {}
        for finding in summary["findings"]:
            key = f"{finding['rule']} {finding['severity']}"
            rules[key] = rules.get(key, 0) + 1
        found = ", ".join(f"{count} x {key}" for key, count in sorted(rules.items())) or "none"
        family = summary["family"]
        lines.append(f"| `{directory}` | {summary['label']} | {summary['verdict']} | {str(summary['qualifies']).lower()} | "
                     f"{summary['sessions']} | {family['comparisons']} | {fmt(family['family_alpha'])} | "
                     f"{family['per_comparison_confidence']:.6f} | {found} | `{digest}` |")
    return lines


def cells_table(directory, receipt, summary):
    verdicts = {cell["cell_id"]: cell for cell in summary["cells"]}
    lines = [f"### `{directory}`", "",
             f"Family `{summary['family']['family_id']}`, campaign `{summary['campaign_id']}`. "
             "gf2 is the baseline and gf2x the candidate: a speedup above 1 means gf2x is faster.", "",
             "`Flagged` is the acceptance tool's count under the receipt's protocol version; `Per-execution` "
             "recounts the raw windows under the protocol-v3 rule.", "",
             "| Cell | Pairs | gf2 median | gf2x median | Speedup | Interval | gf2 faster by | Decision | Outcome | Flagged | Per-execution | gf2 path |",
             "|---|---:|---:|---:|---:|---|---:|---|---|---:|---:|---|"]
    for cell in receipt["cells"]:
        verdict = verdicts[cell["cell_id"]]
        interval = verdict.get("interval")
        if not cell["pairs"] or interval is None:
            lines.append(f"| `{cell['cell_id']}` | 0 | - | - | - | - | - | - | {verdict['outcome']} | - | - | {cell.get('unavailable_reason') or '-'} |")
            continue
        estimate = interval["estimate"]
        faster = f"{1 / estimate:.4g} [{1 / interval['upper']:.4g}, {1 / interval['lower']:.4g}]" if estimate < 1 else "-"
        flagged, total = execution_flags(cell)
        lines.append(
            f"| `{cell['cell_id']}` | {len(cell['pairs'])} | {ns(arm_median(cell, 'baseline'))} | "
            f"{ns(arm_median(cell, 'candidate'))} | {estimate:.4g} | [{interval['lower']:.4g}, {interval['upper']:.4g}] "
            f"at {interval['confidence']:.5f} | {faster} | {verdict.get('decision') or '-'} | {verdict['outcome']} | "
            f"{verdict['flagged_windows']}/{verdict['total_windows']} | {flagged}/{total} | "
            f"`{'; '.join(paths(cell, 'baseline'))}` |")
    return lines


def conversion_table(directory, receipt):
    lines = [f"### Costs outside the timed windows: `{directory}`", "",
             "Medians over all executions of each arm. `unpack` is the separated field reduction for the "
             "4-word, 9-word and dot-product cells (rounded up, amortised over 4096 repetitions) and zero elsewhere.", "",
             "| Cell | Arm | setup | pack | batch fill | dispatch | unpack (reduction) |",
             "|---|---|---:|---:|---:|---:|---:|"]
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            executions = [pair[side] for pair in cell["pairs"] if pair[side].get("conversion")]
            if not executions:
                continue
            arm = executions[0]["arm"]
            median = {key: statistics.median(e["conversion"][key] for e in executions)
                      for key in ("setup_ns", "pack_ns", "batch_fill_ns", "dispatch_ns", "unpack_ns")}
            lines.append(f"| `{cell['cell_id']}` | `{arm}` | {median['setup_ns']:g} ns | {median['pack_ns']:g} ns | "
                         f"{median['batch_fill_ns']:g} ns | {median['dispatch_ns']:g} ns | {median['unpack_ns']:g} ns |")
    return lines


def library_table(rows):
    lines = ["## gf2x libraries the candidate arms mapped", "",
             "| Receipt | Arm | Library | SHA-256 | CFLAGS |", "|---|---|---|---|---|"]
    pattern = re.compile(r"library=(\S+) sha256=(\S+) cflags=(.*)$")
    for directory, (receipt, _, _) in rows:
        seen = set()
        for cell in receipt["cells"]:
            for pair in cell["pairs"]:
                execution = pair["candidate"]
                match = pattern.search(execution.get("selected_path") or "")
                if match:
                    key = (execution["arm"],) + match.groups()
                    if key not in seen:
                        seen.add(key)
                        lines.append(f"| `{directory}` | `{key[0]}` | `{key[1]}` | `{key[2]}` | `{key[3]}` |")
    return lines


def sessions_table(rows):
    lines = ["## Host observations per session", "",
             "| Receipt | Session | Observed (UTC) | Load average | Available memory (KiB) |", "|---|---:|---|---|---:|"]
    for directory, (receipt, _, _) in rows:
        for index, host in enumerate(receipt.get("session_hosts") or [receipt["host"]], start=1):
            lines.append(f"| `{directory}` | {index} | {host['observed_utc']} | {host.get('load_average')} | "
                         f"{host.get('available_memory_kib')} |")
    return lines


def per_product_table(directory, receipt, summary):
    lines = [f"### Cost per schoolbook word product: `{directory}`", "",
             "Each arm's median call divided by the $N^2$ word products the schoolbook definition performs per "
             "operation, times `inner` and the worker count. For gf2x this is a normalised rate, because its "
             "recursion avoids most of those products.", "",
             "| Cell | Words | Workers | Products per call | gf2 ns per product | gf2x ns per product |",
             "|---|---:|---:|---:|---:|---:|"]
    plan_cases = {}
    with open(f"{directory}/plan.json") as handle:
        for cell in json.load(handle)["cells"]:
            plan_cases[cell["cell_id"]] = cell["case"]
    for cell in receipt["cells"]:
        case = plan_cases[cell["cell_id"]]
        if case["kind"] != "poly-mul" or not cell["pairs"]:
            continue
        workers = cell["pairs"][0]["baseline"]["workers_observed"]
        products = case["words"] ** 2 * case["inner"] * workers
        lines.append(f"| `{cell['cell_id']}` | {case['words']} | {workers} | {products} | "
                     f"{arm_median(cell, 'baseline') / products:.4g} | {arm_median(cell, 'candidate') / products:.4g} |")
    return lines


def main():
    output, directories = sys.argv[1], sys.argv[2:]
    rows = [(directory.rstrip("/"), load(directory.rstrip("/"))) for directory in directories]
    lines = ["# Polynomial-multiplication survey tables (jit:c7113c5a)", "",
             "Generated by `dev/active/c7113c5a/survey/summarize-v3.py` from the receipt directories below.", ""]
    lines += overview(rows) + [""]
    lines += ["## Cells", ""]
    for directory, (receipt, summary, _) in rows:
        lines += cells_table(directory, receipt, summary) + [""]
    for directory, (receipt, summary, _) in rows:
        if summary["label"] == "confirmation":
            lines += conversion_table(directory, receipt) + [""]
            lines += per_product_table(directory, receipt, summary) + [""]
    lines += library_table(rows) + [""]
    lines += sessions_table(rows) + [""]
    with open(output, "w") as handle:
        handle.write("\n".join(lines))
    print(f"wrote {output}", file=sys.stderr)


if __name__ == "__main__":
    main()
