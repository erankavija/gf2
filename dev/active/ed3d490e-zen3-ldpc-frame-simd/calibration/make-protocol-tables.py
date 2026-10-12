#!/usr/bin/env python3
"""Write the generated tables of the decoder calibration protocol (jit:0a77ecfc).

Usage: make-protocol-tables.py [--check]

Rewrites every `<!-- generated:<name> -->` block of
`premeasurement-protocol.md` that the document holds: the grid and the fixture
codes from the committed grid listing of the coding producer, the declared
counts from that listing, the self-check and the campaign-support timing
constants, and the window estimate from those counts and the rehearsal record.
"""

import json
import math
import re
import sys

from locate import CALIBRATION, HERE, ROOT, emit, package

DOCUMENT = HERE / "premeasurement-protocol.md"
PRODUCER = HERE / "producer"
REHEARSAL = HERE / "rehearsal" / "record.json"
USIZE_MAX = (1 << 64) - 1
DETAIL = re.compile(r"(decode\.\w+): (candidates|stratum)=(.*)")
# Minutes the queue estimate is rounded up to.
ESTIMATE_STEP = 5


def evidence(mode):
    return json.loads(json.loads((PRODUCER / f"coding-{mode}.json").read_text())["evidence"])


def constant(source, name, pattern=r"(\d[\d_]*)"):
    found = re.search(rf"\b{name}: \w+ = {pattern}", source)
    if not found:
        raise SystemExit(f"the campaign-support sources do not carry {name}")
    return int(found.group(1).replace("_", ""))


def shown(value):
    return "`usize::MAX`" if int(value) == USIZE_MAX else f"`{int(value)}`"


def table(header, rows):
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    lines += ["| " + " | ".join(str(cell) for cell in row) + " |" for row in rows]
    return "\n".join(lines)


def grid(listing):
    fields = {}
    for detail in listing["details"]:
        found = DETAIL.fullmatch(detail)
        if not found:
            continue
        entry = fields.setdefault(found.group(1), {"strata": []})
        if found.group(2) == "candidates":
            candidates, conservative = found.group(3).split(" conservative=")
            entry["candidates"] = json.loads(candidates)
            entry["conservative"] = conservative
        else:
            stratum, entry["entry"] = found.group(3).split(" entry=")
            entry["strata"].append(stratum)
    tags = {item["field"]: item["seeds"]["field_tag"] for item in listing["inventory"]}
    rows = [
        (tags[name], f"`{name}`", ", ".join(shown(value) for value in field["candidates"]),
         shown(field["conservative"]), f"`{field['entry']}`",
         ", ".join(f"`{stratum}`" for stratum in field["strata"]),
         len(field["candidates"]) * len(field["strata"]))
        for name, field in fields.items()
    ]
    if sum(row[-1] for row in rows) != listing["cells"]:
        raise SystemExit("the grid listing's details do not multiply out to its cells")
    return table(("Tag", "Field", "Candidates", "Conservative", "Entry", "Strata", "Cells"), rows)


def codes(listing):
    rows = [
        (f"`{item['field']}`", f"`{item['code']}`", item["frame_len"], item["edges"],
         item["noise_sigma_milli"], ", ".join(map(str, item["batch_frames"])),
         item["seeds"]["shape_key"], item["max_iterations"])
        for item in listing["inventory"]
    ]
    return table(("Field", "Code", "Frame length", "Edges", "Noise deviation (per mille)",
                  "Batch frames", "Shape key", "Iteration cap"), rows)


def counts(listing, timing):
    cells = listing["cells"]
    timed = cells * timing["executions"]
    declared = json.loads((HERE / "campaign-declaration.json").read_text())
    if [owner["cells"] for owner in declared["measured_owners"]] != [cells]:
        raise SystemExit("the declaration's cells differ from the grid listing's")
    if (listing["accepted_results"], listing["windows"]) != (
            cells + timed, timed * timing["windows"]):
        raise SystemExit("the grid listing's counts differ from the timing constants'")
    inventory = next(detail for detail in evidence("self-check")["details"]
                     if detail.startswith("codec-inventory="))
    measured, leaves = inventory.removeprefix("codec-inventory=").split("/")
    return table(("Quantity", "Count"), [
        ("cells", cells),
        ("untimed probes", cells),
        ("timed children", timed),
        ("accepted results", listing["accepted_results"]),
        ("raw windows", listing["windows"]),
        ("timing progress records", timed * (timing["windows"] + 1)),
        ("nominal target-window seconds", timed * timing["windows"] * timing["target_ms"] / 1000),
        ("coding codec leaves", leaves),
        ("measured leaves", measured),
    ])


def estimate(listing, timing):
    record = json.loads(REHEARSAL.read_text())
    cells = listing["cells"]
    timed = cells * timing["executions"]
    windows = timed * timing["windows"] * timing["target_ms"] / 1000
    untimed = record["active_seconds"]
    minutes = math.ceil((windows + untimed) / 60 / ESTIMATE_STEP) * ESTIMATE_STEP
    return table(("Term", "Value"), [
        ("nominal target-window seconds", windows),
        ("active seconds of the rehearsal session (one run, unlocked host)", untimed),
        ("session budget seconds", timing["budget"]),
        (f"estimated minutes, rounded up to {ESTIMATE_STEP}", minutes),
    ])


def main():
    support = ROOT / package("tuning-campaign-support") / "src"
    timing_source = (support / "timing.rs").read_text()
    timing = {
        "executions": constant(timing_source, "EXECUTIONS"),
        "windows": constant(timing_source, "WINDOWS"),
        "target_ms": constant(timing_source, "TARGET", r"Duration::from_millis\((\d+)\)"),
        "budget": constant((support / "campaign.rs").read_text(), "SESSION_BUDGET_SECONDS"),
    }
    listing = evidence("list-grid")
    blocks = {"grid": grid, "codes": codes}
    text = DOCUMENT.read_text()
    for name in re.findall(r"<!-- generated:(\w+) -->", text):
        if name in blocks:
            body = blocks[name](listing)
        elif name == "counts":
            body = counts(listing, timing)
        elif name == "estimate":
            body = estimate(listing, timing)
        else:
            raise SystemExit(f"unknown generated block {name}")
        block = re.compile(rf"(<!-- generated:{name} -->\n).*?(<!-- /generated:{name} -->)", re.S)
        if not block.search(text):
            raise SystemExit(f"generated block {name} is not closed")
        text = block.sub(lambda found: f"{found.group(1)}{body}\n{found.group(2)}", text)
    emit(DOCUMENT, text, sys.argv[1:] == ["--check"], f"{CALIBRATION}/make-protocol-tables.py")


if __name__ == "__main__":
    main()
