#!/usr/bin/env python3
"""Render the logical-buffer profile session's attribution tables.

Every figure is the median over the session's repetitions with the
order-statistic interval the repetition count supports, derived from the
repetition directories alone, so a re-run over the same directories reproduces
this document byte for byte.
"""

import argparse
import collections
import json
import pathlib
import re

REPORT_LINE = re.compile(r"^\s+([0-9.]+)%\s+\S+\s+\S+\s+\[[^\]]\]\s+(.+?)\s*$")


def order_statistic_interval(count):
    """Ranks bounding the median at the highest coverage below 0.95 exclusion.

    For nine observations the interval is [x(2), x(8)]; for fewer it widens to
    the extremes, which is stated rather than silently narrowed.
    """
    if count >= 9:
        return 1, count - 2
    return 0, count - 1


def median(values):
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2


def summarize(values):
    ordered = sorted(values)
    low, high = order_statistic_interval(len(ordered))
    return median(ordered), ordered[low], ordered[high]


def read_counters(path):
    """Counter values of one `perf stat -x,` file, keyed by event name."""
    counters = {}
    for line in path.read_text().splitlines():
        if line.startswith("#") or not line.strip():
            continue
        fields = line.split(",")
        if len(fields) < 3:
            continue
        value, event = fields[0], fields[2]
        if value in ("<not counted>", "<not supported>"):
            continue
        # perf appends the modifiers it applied, such as `:u` when the host
        # restricts counting to user space.
        counters[event.split(":")[0]] = float(value)
    return counters


def read_case(rep, case, group):
    driver = json.loads((rep / f"{case}.{group}.json").read_text())
    return driver, read_counters(rep / f"{case}.{group}.csv")


def ratio(numerator, denominator):
    return numerator / denominator if denominator else 0.0


def collect(cases, reps):
    """Per-case series of the derived per-call figures across repetitions."""
    series = collections.defaultdict(lambda: collections.defaultdict(list))
    paths = {}
    for case in cases:
        for rep in reps:
            issue_driver, issue = read_case(rep, case, "issue")
            memory_driver, memory = read_case(rep, case, "memory")
            paths.setdefault(case, issue_driver["selected_path"])
            if issue_driver["selected_path"] != paths[case]:
                raise SystemExit(f"{case}: the observed route differs between repetitions")
            calls = issue_driver["calls"]
            series[case]["ns_per_call"].append(ratio(issue_driver["elapsed_ns"], calls))
            series[case]["calls"].append(float(calls))
            series[case]["cycles_per_call"].append(ratio(issue.get("cycles", 0.0), calls))
            series[case]["instructions_per_call"].append(
                ratio(issue.get("instructions", 0.0), calls)
            )
            series[case]["ipc"].append(
                ratio(issue.get("instructions", 0.0), issue.get("cycles", 0.0))
            )
            series[case]["branches_per_call"].append(ratio(issue.get("branches", 0.0), calls))
            series[case]["branch_miss_rate"].append(
                ratio(issue.get("branch-misses", 0.0), issue.get("branches", 0.0))
            )
            memory_calls = memory_driver["calls"]
            series[case]["l1_loads_per_call"].append(
                ratio(memory.get("L1-dcache-loads", 0.0), memory_calls)
            )
            series[case]["l1_miss_rate"].append(
                ratio(memory.get("L1-dcache-load-misses", 0.0), memory.get("L1-dcache-loads", 0.0))
            )
            series[case]["llc_loads_per_call"].append(
                ratio(memory.get("LLC-loads", 0.0), memory_calls)
            )
            series[case]["llc_miss_rate"].append(
                ratio(memory.get("LLC-load-misses", 0.0), memory.get("LLC-loads", 0.0))
            )
    return series, paths


def cell(values, digits):
    point, low, high = summarize(values)
    return f"{point:.{digits}f} [{low:.{digits}f}, {high:.{digits}f}]"


def symbol_shares(recorded, reps):
    """Complete-report share summaries; omitted rows remain censored."""
    shares = {}
    for case in recorded:
        per_symbol = collections.defaultdict(list)
        for rep in reps:
            report = rep / f"{case}.report.txt"
            seen = {}
            for line in report.read_text().splitlines():
                match = REPORT_LINE.match(line)
                if match:
                    seen[match.group(2)] = seen.get(match.group(2), 0.0) + float(match.group(1))
            for symbol, percent in seen.items():
                per_symbol[symbol].append(percent)
        shares[case] = sorted(
            ((symbol, summarize(values) if len(values) == len(reps) else None,
              len(values)) for symbol, values in per_symbol.items()),
            key=lambda row: (row[1] is None, -row[1][0] if row[1] else 0, row[0]),
        )
    return shares


def report_links(case, reps):
    """Raw reports for checking each symbol's presence across repetitions."""
    return ", ".join(
        "[" + rep.name + "](" + (pathlib.Path(rep.name) / f"{case}.report.txt").as_posix() + ")"
        for rep in reps
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=pathlib.Path, help="the profile output directory")
    arguments = parser.parse_args()
    out = arguments.output
    cases = [line for line in (out / "cases.txt").read_text().splitlines() if line]
    recorded = [line for line in (out / "recorded-cases.txt").read_text().splitlines() if line]
    reps = sorted(path for path in out.glob("rep-*") if path.is_dir())
    if not reps:
        raise SystemExit(f"{out}: no repetition directory")
    series, paths = collect(cases, reps)

    print("# Logical-buffer profile attribution (jit:18a87159)")
    print()
    print(
        f"Source: {len(reps)} repetitions under `{out.name}/rep-*`, "
        f"{len(cases)} frozen profile cases, "
        f"driver and host identities in `{out.name}/host.txt`."
    )
    print()
    print(
        "Per-call and memory figures are medians over the repetitions with "
        "order-statistic intervals in brackets. A per-call figure divides a "
        "whole-process counter by the call count the driver observed in that "
        "same pass, so it carries the pass's own start-up and fixture "
        "construction as well as its measured operations. Symbol-share "
        "availability and bounds follow the rules below."
    )
    print()
    print("## Per-call cost and instruction mix")
    print()
    print("| Case | ns/call | cycles/call | instructions/call | IPC | branches/call | branch-miss rate |")
    print("|---|---|---|---|---|---|---|")
    for case in cases:
        values = series[case]
        print(
            f"| `{case}` | {cell(values['ns_per_call'], 3)} | "
            f"{cell(values['cycles_per_call'], 2)} | "
            f"{cell(values['instructions_per_call'], 2)} | "
            f"{cell(values['ipc'], 3)} | "
            f"{cell(values['branches_per_call'], 2)} | "
            f"{cell(values['branch_miss_rate'], 4)} |"
        )
    print()
    print("## Memory traffic")
    print()
    print("| Case | L1 loads/call | L1 load-miss rate | LLC loads/call | LLC load-miss rate | calls per pass |")
    print("|---|---|---|---|---|---|")
    for case in cases:
        values = series[case]
        print(
            f"| `{case}` | {cell(values['l1_loads_per_call'], 2)} | "
            f"{cell(values['l1_miss_rate'], 4)} | "
            f"{cell(values['llc_loads_per_call'], 3)} | "
            f"{cell(values['llc_miss_rate'], 4)} | "
            f"{cell(values['calls'], 0)} |"
        )
    print()
    print("## Observed route")
    print()
    print("| Case | selected path |")
    print("|---|---|")
    for case in cases:
        print(f"| `{case}` | `{paths[case]}` |")
    print()
    print("## Sampled symbol shares")
    print()
    print(
        "Symbols the flat sample report attributes at or above its own "
        "percent limit. A share is summarized only when every repetition "
        "reports that symbol, using the same order-statistic interval as the "
        "other tables. Missing rows are censored by report display and are "
        "not zero measurements; their all-repetition aggregate is unavailable. "
        "The linked raw reports below show which repetitions contain each symbol."
    )
    print()
    print("| Case | symbol | share, median [interval] | reports present |")
    print("|---|---|---|---:|")
    share_rows = symbol_shares(recorded, reps)
    for case, rows in share_rows.items():
        for symbol, summary, count in rows:
            share = (
                f"{summary[0]:.2f}% [{summary[1]:.2f}%, {summary[2]:.2f}%]"
                if summary else "unavailable (report-censored)"
            )
            print(f"| `{case}` | `{symbol}` | {share} | {count}/{len(reps)} |")
    censored = [case for case, rows in share_rows.items()
                if any(summary is None for _, summary, _ in rows)]
    for case in censored:
        print()
        print(f"Raw reports for censored `{case}` rows: {report_links(case, reps)}.")


if __name__ == "__main__":
    main()
