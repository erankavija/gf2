#!/usr/bin/env python3
"""Render the DVB-T2 interleaver attribution tables from committed evidence.

Every figure is read from the protocol-v4 receipt, the dynamic profile session
records, the perf outputs of the retained session, or the source-evidence
ledger. The renderer derives nothing it does not read and states no host
condition of its own, so two runs over the same committed inputs produce the
same bytes. It is untimed: it opens files and writes Markdown.

Interval method, one method everywhere: a repeated figure is summarised by the
median of the per-repetition statistic with the distribution-free
order-statistic interval on that median. For n samples the interval is
[x(r), x(n+1-r)] with r = 2 when n is at least eight and r = 1 below that, and
its binomial coverage is computed from n and r, so nine profile sessions carry
[x(2), x(8)] and six receipt pairs carry [x(1), x(6)]. A figure sampled inside
one run, where the uncertainty is perf's sampling rather than repetition, gets
the Wilson interval on the sampled proportion at the samples the listing states.
A quotient of two unpaired medians gets neither and is labelled descriptive.

Usage: make-dvb-tables.py <receipt> <profile-dir> <source-evidence> <addendum>
"""

from __future__ import annotations

import csv
import json
import math
import pathlib
import re
import sys

ANNOTATED_PERCENT_LIMIT = 1.0
WILSON_Z = 1.959964
SINGLE_SESSION = (
    "These rows are descriptive single-session observations: n = 1 retained session, so they "
    "identify where the cost sits and support no comparison between cases or between routes."
)
GAP_SUFFIX = "-warm-sim-stage-gap"
STREAMING_GAP = "dvb-t2-qam16-r12-normal-streaming-sim-stage-gap"
MODCODS = ("qam16-r12-normal", "qam64-r12-normal", "qam16-r12-short", "qam64-r12-short")
COUNTER_EVENTS = (
    "cycles:u",
    "instructions:u",
    "branches:u",
    "branch-misses:u",
    "cache-references:u",
    "cache-misses:u",
)
HOT_ROW = re.compile(r"^\s*([0-9.]+)%\s+(\d+)\s+(\S+)\s+\[\.\]\s+(\S+)")
INSTRUCTION_ROW = re.compile(r"^\s*([0-9.]+)\s+:\s+([0-9a-f]+):\s+(\S+)\s*(.*)$")
ANNOTATED_SAMPLES = re.compile(r"\((\d+) samples, percent: local period\)")


def median(values: list[float]) -> float:
    return sorted(values)[len(values) // 2]


def order_statistic(values: list[float]) -> tuple[float, float, float, float]:
    """Median, its order-statistic interval bounds and the interval's coverage."""
    ordered = sorted(values)
    count = len(ordered)
    rank = 2 if count >= 8 else 1
    tail = sum(math.comb(count, k) for k in range(rank))
    coverage = 1.0 - 2.0 * tail / 2.0**count
    return ordered[count // 2], ordered[rank - 1], ordered[count - rank], coverage


def coverages(*intervals: tuple[float, float, float, float]) -> str:
    """The one coverage a set of equal-sized samples shares, for a table's caption."""
    distinct = sorted({f"{interval[3]:.3f}" for interval in intervals})
    return " and ".join(distinct)


def wilson(successes: float, samples: float) -> tuple[float, float]:
    """Wilson score interval on a sampled proportion, as percentages."""
    if samples <= 0:
        return 0.0, 100.0
    proportion = successes / samples
    span = WILSON_Z * WILSON_Z / samples
    centre = (proportion + span / 2.0) / (1.0 + span)
    width = (
        WILSON_Z
        * math.sqrt(proportion * (1.0 - proportion) / samples + span / (4.0 * samples))
        / (1.0 + span)
    )
    return 100.0 * max(0.0, centre - width), 100.0 * min(1.0, centre + width)


def fixed(value: float) -> str:
    return f"{value:,.0f}".replace(",", " ")


def span(low: float, high: float) -> str:
    return f"[{fixed(low)}, {fixed(high)}]"


def ratio(value: float) -> str:
    return f"{value:.3f}"


def frame_bits(addendum: dict) -> tuple[dict[str, int], dict[str, int]]:
    """Frame bits per frozen cell and per MODCOD, from the declared workload size."""
    per_cell: dict[str, int] = {}
    per_modcod: dict[str, int] = {}
    for cell in addendum["cells"]:
        size = cell["workload"]["size"]
        count = size["nc"] * size["nr"]
        per_cell[cell["cell_id"]] = count
        identity = cell["workload"]["identity"]
        for modcod in MODCODS:
            if modcod in identity:
                per_modcod[modcod] = count
    if set(per_modcod) != set(MODCODS):
        raise SystemExit("the addendum does not declare every MODCOD of the profile ladder")
    return per_cell, per_modcod


def case_modcods(profile: pathlib.Path) -> dict[str, str]:
    """Maps each profile case to the MODCOD its ladder declares."""
    ladder = json.loads((profile / "ladder.json").read_text())
    return {case["id"]: case["modcod"] for case in ladder}


def gap_cells(receipt: dict) -> list[dict]:
    return [
        cell
        for cell in receipt["cells"]
        if cell["cell_id"].endswith(GAP_SUFFIX) or cell["cell_id"] == STREAMING_GAP
    ]


def null_cells(receipt: dict) -> list[dict]:
    return [cell for cell in receipt["cells"] if cell["cell_id"].endswith("-isolated-null")]


def side_parts(cell: dict, side: str) -> dict[str, list[float]]:
    """Per-pair per-call total and conversion parts of one arm of a cell."""
    sides = [pair[side] for pair in cell["pairs"]]
    parts: dict[str, list[float]] = {"total": [s["ns_per_call"] for s in sides]}
    for key in ("unpack_ns", "batch_fill_ns", "pack_ns"):
        parts[key] = [float(s["conversion"][key]) for s in sides]
    parts["remainder"] = [
        total - unpack - fill - pack
        for total, unpack, fill, pack in zip(
            parts["total"], parts["unpack_ns"], parts["batch_fill_ns"], parts["pack_ns"]
        )
    ]
    return parts


PART_NAMES = (
    ("unpack_ns", "unpack"),
    ("batch_fill_ns", "input copy and output alloc"),
    ("pack_ns", "pack"),
    ("remainder", "remainder"),
)


def decomposition(receipt: dict) -> None:
    print("## Whole-consumer cost decomposition")
    print()
    cells = gap_cells(receipt)
    summaries = [
        (cell, order_statistic(side_parts(cell, "baseline")["total"]), order_statistic(side_parts(cell, "candidate")["total"]))
        for cell in cells
    ]
    print(
        "Each row is the median over the cell's paired executions of the arm's per-call wall "
        f"time, with the order-statistic interval on that median at coverage "
        f"{coverages(*[interval for _, base, cand in summaries for interval in (base, cand)])}. "
        "Cell `Decision` is the evaluator's own record."
    )
    print()
    print("| Cell | pairs | gf2 stage ns/call | interval | xdsopl ns/call | interval | Decision |")
    print("|---|---:|---:|---|---:|---|---|")
    for cell, base, cand in summaries:
        print(
            f"| `{cell['cell_id']}` | {len(cell['pairs'])} | {fixed(base[0])} | "
            f"{span(base[1], base[2])} | {fixed(cand[0])} | {span(cand[1], cand[2])} | "
            f"{cell['claimed']['decision']} |"
        )
    print()

    print("### Conversion parts of both arms")
    print()
    print(
        "Each arm times unpack, the destructive-input copy with output allocation, and pack "
        "inside every call, and reports them whether or not it crosses a representation "
        "boundary. The remainder is the per-call total less those three parts; for the xdsopl arm "
        "it holds the `PCTITL` permutation together with the final packed-batch wrap, which the "
        "arm does not time separately, so the remainder is not a `PCTITL` figure. Each part is "
        "the median over the same pairs with its own order-statistic interval."
    )
    print()
    print("| Cell | arm | part | pairs | ns/call | interval |")
    print("|---|---|---|---:|---:|---|")
    for cell in cells:
        for side, arm in (("baseline", "gf2 stage"), ("candidate", "xdsopl")):
            parts = side_parts(cell, side)
            for key, name in PART_NAMES:
                statistic = order_statistic(parts[key])
                print(
                    f"| `{cell['cell_id']}` | {arm} | {name} | {len(cell['pairs'])} | "
                    f"{fixed(statistic[0])} | {span(statistic[1], statistic[2])} |"
                )
    print()


def boundaries(receipt: dict, bits: dict[str, int]) -> None:
    print("## Isolated scatter beside the whole consumer")
    print()
    print(
        "The isolated column is the baseline arm of the same MODCOD's null cell, which runs "
        "`DvbT2BitInterleaver::interleave` alone on one packed frame. The stage column is the "
        "baseline arm of its gap cell, which wraps the same call in "
        "`gf2-sim BitInterleave::process` over a one-frame `BitPackedBatch`. Both are medians "
        "over the pairs of one receipt with their own order-statistic intervals. The two cells "
        "run separately, so the last two columns are descriptive quotients of medians and carry "
        "no interval."
    )
    print()
    print(
        "| Cell pair | frame bits | pairs | isolated ns/call | interval | stage ns/call | "
        "interval | ns per bit isolated | stage / isolated |"
    )
    print("|---|---:|---:|---:|---|---:|---|---:|---:|")
    stages = {cell["cell_id"]: cell for cell in gap_cells(receipt)}
    for cell in null_cells(receipt):
        stem = cell["cell_id"].rsplit("-", 2)[0]
        stage = stages.get(f"{stem}-sim-stage-gap")
        if stage is None:
            continue
        isolated = order_statistic(side_parts(cell, "baseline")["total"])
        wrapped = order_statistic(side_parts(stage, "baseline")["total"])
        n = bits[cell["key"]]
        print(
            f"| `{stem}` | {fixed(n)} | {len(cell['pairs'])} | {fixed(isolated[0])} | "
            f"{span(isolated[1], isolated[2])} | {fixed(wrapped[0])} | "
            f"{span(wrapped[1], wrapped[2])} | {ratio(isolated[0] / n)} | "
            f"{ratio(wrapped[0] / isolated[0])} |"
        )
    print()


def sessions(profile: pathlib.Path) -> dict[str, list[dict]]:
    """Per-case records of every completed session, in session order."""
    records: dict[str, list[dict]] = {}
    for path in sorted(profile.glob("rep-*/cases.json")):
        for record in json.loads(path.read_text()):
            records.setdefault(record["id"], []).append(record)
    return records


def census(values: list[int]) -> str:
    distinct = sorted(set(values))
    return str(distinct[0]) if len(distinct) == 1 else f"{distinct[0]}..{distinct[-1]}"


def across_sessions(profile: pathlib.Path) -> None:
    print("## Across-session cost and allocation")
    print()
    records = sessions(profile)
    ladder = [case["id"] for case in json.loads((profile / "ladder.json").read_text())]
    statistics = {case: order_statistic([row["ns_per_call"] for row in records[case]]) for case in ladder}
    print(
        "Each row is one profile case over the completed sessions of "
        f"`{profile.as_posix()}`, whose `rep-NN/cases.json` records are the raw source; the "
        "session's own `profile-summary.md` renders the same records. The per-session statistic "
        "is that session's median over its five windows, and the interval is the "
        f"order-statistic interval on the across-session median at coverage "
        f"{coverages(*statistics.values())}. The allocation columns are an exact per-call census "
        "rather than a sampled figure, so they carry the observed values and no interval; the "
        "counting allocator adds relaxed atomics, so these wall times explain composition only."
    )
    print()
    print("| Case | sessions | ns/call | interval | allocations/call | bytes/call |")
    print("|---|---:|---:|---|---:|---:|")
    for case in ladder:
        rows = records[case]
        statistic = statistics[case]
        print(
            f"| `{case}` | {len(rows)} | {fixed(statistic[0])} | "
            f"{span(statistic[1], statistic[2])} | "
            f"{census([row['allocations_per_call'] for row in rows])} | "
            f"{census([row['allocated_bytes_per_call'] for row in rows])} |"
        )
    print()


def scatter_shares(profile: pathlib.Path) -> None:
    print("## Scatter share of the BICM channel across sessions")
    print()
    records = sessions(profile)
    shares = {
        modcod: [
            100.0 * direct["ns_per_call"] / channel["ns_per_call"]
            for direct, channel in zip(records[f"{modcod}-direct"], records[f"{modcod}-bicm-channel"])
        ]
        for modcod in MODCODS
    }
    statistics = {modcod: order_statistic(values) for modcod, values in shares.items()}
    print(
        "The share is formed inside each session: that session's direct-scatter statistic "
        "divided by its own full max-log BICM channel statistic for the same MODCOD. The rows "
        "are the median of those per-session shares with the order-statistic interval at "
        f"coverage {coverages(*statistics.values())}. The two cases run as separate processes "
        "under the counting allocator, so the share describes composition and decides nothing; "
        "the materiality decision is the paired receipt's. The Amdahl ceiling is an "
        "**estimate**: it is 1/(1 - share), the single-worker speedup that would follow if the "
        "scatter vanished at no replacement cost and nothing else changed, and a measured "
        "speedup below it refutes nothing."
    )
    print()
    print("| MODCOD | sessions | share | interval | Amdahl ceiling (estimate) | ceiling interval |")
    print("|---|---:|---:|---|---:|---|")
    for modcod in MODCODS:
        point, low, high, _ = statistics[modcod]
        print(
            f"| `{modcod}` | {len(shares[modcod])} | {point:.3f}% | [{low:.3f}%, {high:.3f}%] | "
            f"{ratio(1.0 / (1.0 - point / 100.0))} | "
            f"[{ratio(1.0 / (1.0 - low / 100.0))}, {ratio(1.0 / (1.0 - high / 100.0))}] |"
        )
    print()


def counter_table(profile: pathlib.Path, bits: dict[str, int], modcods: dict[str, str]) -> None:
    print("## Hardware counters of the retained session")
    print()
    print(
        "Counts are divided by the frame bits the addendum declares for the case's MODCOD times "
        "the calls the run reports, so every column is per interleaved bit of one frame. "
        f"{SINGLE_SESSION} Only the first session collects counters, so no repetition retains "
        "the data an across-session interval would need."
    )
    print()
    print("| Case | calls | cycles/bit | instructions/bit | IPC | branches/bit | branch-misses/bit | cache-misses/bit |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|")
    counters = profile / "rep-01" / "counters"
    for record_path in sorted(counters.glob("*.json")):
        record = json.loads(record_path.read_text())
        case = record["case"]
        total = record["calls"] * bits[modcods[case]]
        events: dict[str, float] = {}
        with (counters / f"{case}.csv").open() as handle:
            for row in csv.reader(handle):
                if len(row) > 2 and row[2] in COUNTER_EVENTS:
                    events[row[2]] = float(row[0])
        if set(events) != set(COUNTER_EVENTS):
            raise SystemExit(f"{case}: counter file does not carry every requested event")
        print(
            f"| `{case}` | {fixed(record['calls'])} | {ratio(events['cycles:u'] / total)} | "
            f"{ratio(events['instructions:u'] / total)} | "
            f"{ratio(events['instructions:u'] / events['cycles:u'])} | "
            f"{ratio(events['branches:u'] / total)} | "
            f"{ratio(events['branch-misses:u'] / total)} | "
            f"{ratio(events['cache-misses:u'] / total)} |"
        )
    print()


def hot_symbols(profile: pathlib.Path) -> None:
    print("## Hot symbols of the retained session")
    print()
    print(
        "`perf report` lists symbols above a half-percent share. The row is the highest share "
        f"in each case's report with the samples that back it. {SINGLE_SESSION} The report "
        "states no total sample count, so the share carries no sampling interval either."
    )
    print()
    print("| Case | overhead | samples | symbol |")
    print("|---|---:|---:|---|")
    for report in sorted((profile / "rep-01" / "hot").glob("*.report.txt")):
        rows = [HOT_ROW.match(line) for line in report.read_text().splitlines()]
        matched = [row for row in rows if row]
        if not matched:
            print(f"| `{report.name[: -len('.report.txt')]}` | | | no symbol above the report limit |")
            continue
        top = max(matched, key=lambda row: float(row.group(1)))
        print(
            f"| `{report.name[: -len('.report.txt')]}` | {top.group(1)}% | {top.group(2)} | "
            f"`{top.group(4)}` |"
        )
    print()


def hot_instructions(profile: pathlib.Path) -> None:
    print("## Hot instructions of the selected production route")
    print()
    listing = profile / "rep-01" / "hot" / "qam16-r12-normal-direct.instructions.txt"
    text = listing.read_text()
    header = ANNOTATED_SAMPLES.search(text)
    if header is None:
        raise SystemExit(f"{listing}: the listing states no sample count")
    samples = int(header.group(1))
    print(
        "`render-hot-instructions.sh` disassembles the pinned executable against the recorded "
        "samples. Rows are every instruction of the Normal 16-QAM isolated route at or above "
        f"the {ANNOTATED_PERCENT_LIMIT:.1f} percent annotation limit, in address order. Each "
        f"share is a proportion of the {samples} samples the listing attributes to the symbol, "
        "so its interval is the Wilson interval on that proportion at 95 percent. "
        f"{SINGLE_SESSION} The interval covers perf's sampling within the retained session and "
        "nothing about session-to-session variation."
    )
    print()
    print("| address | instruction | share | Wilson 95% |")
    print("|---|---|---:|---|")
    for line in text.splitlines():
        row = INSTRUCTION_ROW.match(line)
        if not row or float(row.group(1)) < ANNOTATED_PERCENT_LIMIT:
            continue
        operands = row.group(4).split("<")[0].strip()
        share = float(row.group(1))
        low, high = wilson(share / 100.0 * samples, samples)
        print(
            f"| `{row.group(2)}` | `{row.group(3)} {operands}`".rstrip()
            + f" | {row.group(1)}% | [{low:.2f}%, {high:.2f}%] |"
        )
    print()


def memory_passes(evidence: dict) -> None:
    print("## Logical memory passes per route")
    print()
    print(
        "The passes are the ones `survey/dvb-source-evidence.json` records against the matched "
        "source fragments of each route."
    )
    print()
    print("| Route | passes | sequence |")
    print("|---|---:|---|")
    for route, passes in evidence["memory_passes"].items():
        print(f"| `{route}` | {len(passes)} | {'; '.join(passes)} |")
    print()


def main() -> None:
    receipt_path, profile_path, evidence_path, addendum_path = (
        pathlib.Path(argument) for argument in sys.argv[1:5]
    )
    receipt = json.loads(receipt_path.read_text())
    evidence = json.loads(evidence_path.read_text())
    addendum = json.loads(addendum_path.read_text())
    profile = profile_path
    cell_bits, modcod_bits = frame_bits(addendum)

    print("# DVB-T2 interleaver attribution tables")
    print()
    print("> **Diátaxis Type:** Reference")
    print()
    print(
        f"Generated by `survey/make-dvb-tables.py` from `{receipt_path.as_posix()}`, "
        f"`{profile_path.as_posix()}`, `{evidence_path.as_posix()}` and "
        f"`{addendum_path.as_posix()}`. Campaign cell verdicts stay in the receipt's acceptance "
        "summary; these tables attribute the cost the receipt compares. The interval method is "
        "the one the generator's docstring declares, and every table states the sample count it "
        "summarises. The dynamic profile session's invocation, executable digest, pinned "
        "source and build closure, RNG declaration and sampling plan are in "
        "`dev/bench_results/c04dd4ac/dvb-interleave-profile/v4-r2-dynamic-profile-provenance.md`."
    )
    print()
    decomposition(receipt)
    boundaries(receipt, cell_bits)
    across_sessions(profile)
    scatter_shares(profile)
    counter_table(profile, modcod_bits, case_modcods(profile))
    hot_symbols(profile)
    hot_instructions(profile)
    memory_passes(evidence)


if __name__ == "__main__":
    main()
