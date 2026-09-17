#!/usr/bin/env python3
"""Render the DVB-T2 interleaver attribution tables from committed evidence.

Every figure is read from the protocol-v4 receipt, the dynamic profile session
records, the perf outputs of the retained session, or the source-evidence
ledger. The renderer derives nothing it does not read and states no host
condition of its own, so two runs over the same committed inputs produce the
same bytes. It is untimed: it opens files and writes Markdown.

Usage: make-dvb-tables.py <receipt> <profile-dir> <source-evidence> <addendum>
"""

from __future__ import annotations

import csv
import json
import pathlib
import re
import sys

ANNOTATED_PERCENT_LIMIT = 1.0
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


def median(values: list[float]) -> float:
    return sorted(values)[len(values) // 2]


def fixed(value: float) -> str:
    return f"{value:,.0f}".replace(",", " ")


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


def side_medians(cell: dict, side: str) -> dict[str, float]:
    """Medians of one arm's per-call total and conversion parts over the pairs."""
    sides = [pair[side] for pair in cell["pairs"]]
    parts = {"total": median([s["ns_per_call"] for s in sides])}
    for key in ("unpack_ns", "batch_fill_ns", "pack_ns"):
        parts[key] = median([float(s["conversion"][key]) for s in sides])
    parts["remainder"] = parts["total"] - parts["unpack_ns"] - parts["batch_fill_ns"] - parts["pack_ns"]
    return parts


def decomposition(receipt: dict) -> None:
    print("## Whole-consumer cost decomposition")
    print()
    print(
        "Each row is the median over the cell's six paired executions of the arm's per-call "
        "wall time and of the conversion parts the arm times inside every call. The remainder "
        "is the per-call total less the three timed parts; for the xdsopl arm it holds the "
        "`PCTITL` permutation together with the final packed-batch wrap, which the arm does "
        "not time separately. Cell `Decision` is the evaluator's own record."
    )
    print()
    print(
        "| Cell | pairs | gf2 stage ns/call | xdsopl ns/call | xdsopl unpack | "
        "xdsopl input copy and output alloc | xdsopl pack | xdsopl remainder | Decision |"
    )
    print("|---|---:|---:|---:|---:|---:|---:|---:|---|")
    for cell in gap_cells(receipt):
        base = side_medians(cell, "baseline")
        cand = side_medians(cell, "candidate")
        print(
            f"| `{cell['cell_id']}` | {len(cell['pairs'])} | {fixed(base['total'])} | "
            f"{fixed(cand['total'])} | {fixed(cand['unpack_ns'])} | "
            f"{fixed(cand['batch_fill_ns'])} | {fixed(cand['pack_ns'])} | "
            f"{fixed(cand['remainder'])} | {cell['claimed']['decision']} |"
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
        "over six pairs from the same receipt, so the wrapper share is a ratio of medians."
    )
    print()
    print("| Cell pair | frame bits | isolated ns/call | stage ns/call | ns per bit isolated | stage / isolated |")
    print("|---|---:|---:|---:|---:|---:|")
    stages = {cell["cell_id"]: cell for cell in gap_cells(receipt)}
    for cell in null_cells(receipt):
        stem = cell["cell_id"].rsplit("-", 2)[0]
        stage = stages.get(f"{stem}-sim-stage-gap")
        if stage is None:
            continue
        isolated = side_medians(cell, "baseline")["total"]
        wrapped = side_medians(stage, "baseline")["total"]
        n = bits[cell["key"]]
        print(
            f"| `{stem}` | {fixed(n)} | {fixed(isolated)} | {fixed(wrapped)} | "
            f"{ratio(isolated / n)} | {ratio(wrapped / isolated)} |"
        )
    print()


def counter_table(profile: pathlib.Path, bits: dict[str, int], modcods: dict[str, str]) -> None:
    print("## Hardware counters of the retained session")
    print()
    print(
        "One session records these; each row is a single observation with no interval. Counts "
        "are divided by the frame bits the addendum declares for the case's MODCOD times the "
        "calls the run reports, so every column is per interleaved bit of one frame."
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
        "in each case's report with the samples that back it."
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
    print(
        "`render-hot-instructions.sh` disassembles the pinned executable against the recorded "
        "samples. Rows are every instruction of the Normal 16-QAM isolated route at or above "
        f"the {ANNOTATED_PERCENT_LIMIT:.1f} percent annotation limit, in address order."
    )
    print()
    print("| address | instruction | share |")
    print("|---|---|---:|")
    listing = profile / "rep-01" / "hot" / "qam16-r12-normal-direct.instructions.txt"
    for line in listing.read_text().splitlines():
        row = INSTRUCTION_ROW.match(line)
        if not row or float(row.group(1)) < ANNOTATED_PERCENT_LIMIT:
            continue
        operands = row.group(4).split("<")[0].strip()
        print(f"| `{row.group(2)}` | `{row.group(3)} {operands}`".rstrip() + f" | {row.group(1)}% |")
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
        f"`{addendum_path.as_posix()}`. Campaign cell verdicts, intervals and pair counts stay "
        "in the receipt's acceptance summary; these tables attribute the cost the receipt "
        "compares."
    )
    print()
    decomposition(receipt)
    boundaries(receipt, cell_bits)
    counter_table(profile, modcod_bits, case_modcods(profile))
    hot_symbols(profile)
    hot_instructions(profile)
    memory_passes(evidence)


if __name__ == "__main__":
    main()
