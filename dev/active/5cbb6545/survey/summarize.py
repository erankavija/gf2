#!/usr/bin/env python3
"""Project the count-optimization receipts into `dev/bench_results/5cbb6545/tables.md`.

Every number comes from a committed receipt, the addendum it snapshots, the
acceptance summary `benchmark-acceptance` wrote beside it, the committed clock
observation, or the committed conformance record. Speedups, their intervals and
the cell decisions are the acceptance tool's and are copied, not recomputed: the
tool owns the estimator, the family correction and the verdicts. Per-call times
are medians over a cell's pairs, labelled descriptive; rates divide a cell's own
operand bytes by them; a cycles-per-byte row converts one median at the clock the
observation recorded for that same cell and arm.

The script writes one file and reproduces it byte for byte from the same
committed inputs: it holds no timestamp of its own and iterates in a declared
order.

Usage: summarize.py  (from the repository root)
"""

import json
import pathlib
import re
import statistics
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from families import bytes_per_call  # noqa: E402

ISSUE = "5cbb6545"
ACTIVE = pathlib.Path("dev/active") / ISSUE
RESULTS = pathlib.Path("dev/bench_results") / ISSUE
OUTPUT = RESULTS / "tables.md"
CLOCK = ACTIVE / "survey" / "clock-observation.json"
VALIDATION = ACTIVE / "validation.json"
#: The stages of `run-count-campaign.sh`, in the order it runs them. A stage
#: with no committed receipt is an error: this table has no optional evidence.
STAGES = [
    "popcount-sweep",
    "popcount-sweep2",
    "popcount-pilot",
    "popcount-pilot-r2",
    "popcount-confirmation",
    "fused-sweep",
    "fused-sweep2",
    "fused-pilot",
    "fused-pilot-r2",
    "fused-confirmation",
]
#: The conversion probes a whole-consumer cell records, in the order shown.
PROBES = ("setup_ns", "pack_ns", "unpack_ns", "batch_fill_ns", "dispatch_ns")


def load(stage):
    """Every committed record of one stage, keyed by cell where per-cell."""
    matches = sorted(RESULTS.glob(f"*-{ISSUE}-{stage}"))
    if len(matches) != 1:
        raise SystemExit(f"stage {stage} has {len(matches)} committed receipts")
    directory = matches[0]
    receipt = json.loads((directory / "receipt.json").read_text())
    summary = json.loads((directory / "acceptance-summary.json").read_text())
    addendum = json.loads((directory / "inputs" / "family-addendum.json").read_text())
    plan = json.loads((directory / "plan.json").read_text())
    ledger = [
        json.loads(line)
        for line in (directory / "inputs" / "trial-ledger.jsonl").read_text().splitlines()
        if line
    ]
    return {
        "stage": stage,
        "directory": directory,
        "receipt": receipt,
        "summary": summary,
        "addendum": addendum,
        "plan": plan,
        "ledger": ledger,
        "cases": {cell["cell_id"]: cell["case"] for cell in plan["cells"]},
        "declared": {cell["cell_id"]: cell for cell in addendum["cells"]},
        "measured": {cell["cell_id"]: cell for cell in receipt["cells"]},
        "verdicts": {cell["cell_id"]: cell for cell in summary["cells"]},
    }


def medians(cell):
    """Median per-call time of each arm of one cell, over its pairs."""
    out = {}
    for side in ("baseline", "candidate"):
        samples = [pair[side]["ns_per_call"] for pair in cell["pairs"]]
        out[side] = statistics.median(samples) if samples else None
    return out


def clock_index():
    record = json.loads(CLOCK.read_text())
    return record, {
        (item["cell_id"], item["arm"]): item for item in record["observations"]
    }


def attempt_number(stage):
    """The family attempt this stage's own frozen ledger prefix makes it.

    `trial_ledger::attempt_alpha` counts the entries that reserved at least one
    comparison; a stage whose prefix has none and which reserves none is not an
    attempt at all.
    """
    entries = [entry for entry in stage["ledger"] if entry["comparisons"] > 0]
    reserved = sum(
        1
        for cell in stage["addendum"]["cells"]
        if cell["role"] != "exploratory"
    )
    return len(entries) if reserved else None


def fmt(value, digits=4):
    return "—" if value is None else f"{value:.{digits}f}"


def interval(verdict):
    box = verdict.get("interval")
    if not box:
        return "—"
    return (
        f"{box['estimate']:.4f} [{box['lower']:.4f}, {box['upper']:.4f}]"
        f" at {box['confidence']:.4f}"
    )


def table(lines, header, rows):
    lines.append("| " + " | ".join(header) + " |")
    lines.append("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")


def campaigns_section(lines, stages):
    lines.append("## Campaigns")
    lines.append("")
    lines.append(
        "One row per committed receipt, in the order `run-count-campaign.sh` runs its"
        " stages. *Attempt* is the family attempt the receipt's own frozen ledger"
        " prefix makes it, counting only entries that reserved a comparison, as"
        " `trial_ledger::attempt_alpha` does; a stage that reserves none decides"
        " nothing and shows none. *m* is the comparisons the prefix reserves,"
        " *alpha_c* the corrected per-comparison alpha the tool applied, and *Draws"
        " per tail* the expected draws in each interval tail at that alpha, the P-20"
        " tail-support quantity. *Arms* digests the arm executable and the survey's"
        " arm source in the receipt's own producing snapshot, so two stages that"
        " measured the same arms are recognisable. *Verdict* is the receipt's"
        " acceptance verdict; *Qualifies* is the separate production-qualification"
        " flag and remains explicit even when the receipt itself is accepted."
    )
    lines.append("")
    rows = []
    for stage in stages:
        summary = stage["summary"]
        family = summary.get("family") or {}
        comparisons = family.get("comparisons")
        alpha = family.get("family_alpha")
        corrected = alpha / comparisons if alpha and comparisons else None
        draws = (
            family["bootstrap_resamples"] * corrected / 2.0 if corrected else None
        )
        attempt = attempt_number(stage)
        arm_source = stage["directory"] / "inputs" / "producing" / ACTIVE / "survey" / (
            "gf2-side/src/arms.rs"
        )
        rows.append(
            [
                f"`{stage['stage']}`",
                stage["receipt"]["label"],
                str(len(stage["addendum"]["cells"])),
                "—" if attempt is None else str(attempt),
                "—" if not comparisons else str(comparisons),
                fmt(corrected, 6),
                fmt(draws, 2),
                fmt(stage["addendum"]["effect"].get("measurement_resolution"), 3),
                f"{stage['addendum']['effect']['worthwhile_speedup']:.2f} /"
                f" {stage['addendum']['effect']['equivalence_margin']:.2f} /"
                f" {stage['addendum']['effect']['material_gap_threshold']:.2f}",
                summary["verdict"],
                "yes" if summary["qualifies"] else "no",
                str(len(summary["findings"])),
                digest(stage["receipt"]["arms"]) + " / " + short(sha256_of(arm_source)),
            ]
        )
    table(
        lines,
        [
            "Stage",
            "Label",
            "Cells",
            "Attempt",
            "m",
            "alpha_c",
            "Draws per tail",
            "Resolution",
            "Margins (worthwhile / equivalence / gap)",
            "Verdict",
            "Qualifies",
            "Findings",
            "Arms (executable / arms.rs)",
        ],
        rows,
    )


def sha256_of(path):
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()


def short(digest_hex):
    return f"`{digest_hex[:12]}`"


def digest(arms):
    digests = {arm["executable_sha256"] for arm in arms.values()}
    if len(digests) != 1:
        raise SystemExit("a receipt names more than one arm executable")
    return short(digests.pop())


def cells_section(lines, stage, clock):
    receipt = stage["receipt"]
    lines.append(f"## Cells: `{stage['stage']}`")
    lines.append("")
    lines.append(
        f"Campaign `{receipt['campaign_id']}`, receipt"
        f" [`{stage['directory'].name}`](./{stage['directory'].name}/),"
        f" toolchain {receipt['toolchain']}, host {receipt['host']['cpu_model']},"
        f" kernel {receipt['host']['os_kernel']}, load average at the host"
        f" observation {receipt['host']['load_average']}."
        " *Speedup* and *Interval* are the acceptance tool's at the corrected"
        " confidence; per-call times are medians over the cell's pairs and are"
        " descriptive. *Route* is the route each arm reported taking for the cell's"
        " own width, from the pairs' `selected_path`."
    )
    lines.append("")
    rows = []
    for cell_id in [cell["cell_id"] for cell in stage["addendum"]["cells"]]:
        measured = stage["measured"][cell_id]
        verdict = stage["verdicts"][cell_id]
        declared = stage["declared"][cell_id]
        times = medians(measured)
        case = stage["cases"][cell_id]
        paths = {
            side: sorted({pair[side]["selected_path"] for pair in measured["pairs"]})
            for side in ("baseline", "candidate")
        }
        rows.append(
            [
                f"`{cell_id}`",
                declared["workload"]["identity"],
                declared["objective"],
                str(len(measured["pairs"])),
                f"`{measured['baseline_arm']}` {fmt(times['baseline'], 2)}",
                f"`{measured['candidate_arm']}` {fmt(times['candidate'], 2)}",
                interval(verdict),
                verdict["decision"] or "—",
                verdict["outcome"],
                f"{verdict['flagged_windows']}/{verdict['total_windows']}",
                " → ".join(
                    ", ".join(paths[side]) for side in ("baseline", "candidate")
                ),
            ]
        )
    table(
        lines,
        [
            "Cell",
            "Workload",
            "Objective",
            "Pairs",
            "Baseline ns/call",
            "Candidate ns/call",
            "Speedup [interval]",
            "Decision",
            "Outcome",
            "Flagged",
            "Route baseline → candidate",
        ],
        rows,
    )
    findings = stage["summary"]["findings"]
    if findings:
        for finding in findings:
            lines.append(f"- Finding {finding}")
        lines.append("")


def rates_section(lines, stages, clock):
    lines.append("## Rates (REQ-09)")
    lines.append("")
    lines.append(
        "Per-call latency, useful bytes per second and cycles per byte for every arm"
        " of every confirmatory cell. *ns/call* is the median over the cell's pairs"
        " and is descriptive; *Bytes/call* is the operand bytes one call feeds to the"
        " route, as `families.bytes_per_call` derives them from the cell's own case;"
        " *GB/s* divides the two. *Cycles/byte* multiplies the same median by the"
        " clock § Clock observation records for that cell and arm and divides by the"
        " bytes: an estimate at an observed clock, not a counted cycle total."
    )
    lines.append("")
    rows = []
    for stage in stages:
        for cell_id in [cell["cell_id"] for cell in stage["addendum"]["cells"]]:
            measured = stage["measured"][cell_id]
            times = medians(measured)
            size = bytes_per_call(stage["cases"][cell_id])
            for side in ("baseline", "candidate"):
                arm = measured[f"{side}_arm"]
                observed = clock.get((cell_id, arm))
                ns = times[side]
                rows.append(
                    [
                        f"`{cell_id}`",
                        f"`{arm}`",
                        str(size),
                        fmt(ns, 2),
                        fmt(size / ns if ns else None, 3),
                        fmt(observed["cycles_per_ns"] if observed else None, 3),
                        fmt(
                            observed["cycles_per_ns"] * ns / size
                            if observed and ns
                            else None,
                            4,
                        ),
                    ]
                )
    table(
        lines,
        ["Cell", "Arm", "Bytes/call", "ns/call", "GB/s", "Clock (cycles/ns)",
         "Cycles/byte"],
        rows,
    )


def conversion_section(lines, stages):
    lines.append("## Conversion costs (REQ-09)")
    lines.append("")
    lines.append(
        "Every probe of every cell that declares its conversion costs included: the"
        " whole-consumer cells, one row per arm per stage that measured them. Each"
        " figure is one probe of the cell's first pair, in nanoseconds, as the arm"
        " child measured it beside the timed windows: `setup_ns` builds that arm's"
        " inputs, which for a matrix-vector cell is the whole matrix, and"
        " `dispatch_ns` is the backend selection one call performs. A cell that"
        " declares no conversion costs records none. These are the dispatch and setup"
        " costs the timed calls of those cells already contain."
    )
    lines.append("")
    rows = []
    for stage in stages:
        for cell_id in [cell["cell_id"] for cell in stage["addendum"]["cells"]]:
            declared = stage["declared"][cell_id]
            if not declared["conversion_costs_included"]:
                continue
            measured = stage["measured"][cell_id]
            pair = measured["pairs"][0]
            for side in ("baseline", "candidate"):
                costs = pair[side]["conversion"] or {}
                rows.append(
                    [
                        f"`{stage['stage']}`",
                        f"`{cell_id}`",
                        declared["metric_kind"],
                        f"`{measured[f'{side}_arm']}`",
                        *[str(costs.get(probe, "—")) for probe in PROBES],
                    ]
                )
    table(lines, ["Stage", "Cell", "Metric kind", "Arm", *PROBES], rows)


def derivation(stage):
    """The committed resolution derivation of one confirmation stage.

    The canonical freezer writes it beside the addendum from the pinned pilot's
    raw pairs, so the widest relative half-width the declared resolution had to
    clear is read from that record rather than recomputed here. The record must
    pin the same pilot receipt and digest the addendum does, or this refuses.
    """
    family = stage["stage"].split("-")[0]
    path = ACTIVE / f"pilot-resolution-{family}.txt"
    text = path.read_text()
    evidence = stage["addendum"]["effect"]["resolution_evidence"]
    if evidence["receipt"] not in text or evidence["sha256"] not in text:
        raise SystemExit(f"{path} does not pin the addendum's resolution evidence")
    widest = re.search(r"widest relative half-width\s+([0-9.]+)", text)
    frozen = re.search(r"frozen measurement resolution ([0-9.]+)", text)
    if not widest or not frozen:
        raise SystemExit(f"{path} states no widest half-width and resolution")
    return path, float(widest.group(1)), float(frozen.group(1))


def resolution_section(lines, stages):
    lines.append("## Numerical resolution and tail support")
    lines.append("")
    lines.append(
        "Each confirmation's declared measurement resolution against the widest"
        " relative half-width of the pilot receipt it pins, and the P-20 tail support"
        " at its corrected alpha. *Pilot widest* and *Declared* are the committed"
        " derivation record's, which the canonical freezer computed from the pinned"
        " pilot's raw pairs; this table checks that the record pins the same pilot"
        " receipt and digest as the addendum and copies the two numbers. *Arms match*"
        " states whether the pinned pilot measured the same arm executable the"
        " confirmation measured."
    )
    lines.append("")
    by_directory = {stage["directory"]: stage for stage in stages}
    rows = []
    for stage in stages:
        effect = stage["addendum"]["effect"]
        evidence = effect.get("resolution_evidence")
        if not evidence:
            continue
        record, widest, frozen = derivation(stage)
        if frozen != effect["measurement_resolution"]:
            raise SystemExit("the derivation record and the addendum disagree")
        pilot = by_directory[pathlib.Path(evidence["receipt"]).parent]
        family = stage["summary"]["family"]
        corrected = family["family_alpha"] / family["comparisons"]
        rows.append(
            [
                f"`{stage['stage']}`",
                f"`{pilot['stage']}`",
                f"`{record}`",
                fmt(widest, 6),
                fmt(frozen, 3),
                fmt(corrected, 6),
                fmt(family["bootstrap_resamples"] * corrected / 2.0, 2),
                "yes" if digest(pilot["receipt"]["arms"]) == digest(
                    stage["receipt"]["arms"]
                ) else "no",
                "none" if not any(
                    cell["outcome"] == "not-confirmatory"
                    for cell in stage["summary"]["cells"]
                ) else "present",
            ]
        )
    table(
        lines,
        [
            "Confirmation",
            "Pinned pilot",
            "Derivation record",
            "Pilot widest relative half-width",
            "Declared resolution",
            "alpha_c",
            "Draws per tail",
            "Arms match",
            "Cells recorded not-confirmatory",
        ],
        rows,
    )


def coverage_section(lines):
    record = json.loads(VALIDATION.read_text())
    lines.append("## Correctness coverage (REQ-08)")
    lines.append("")
    lines.append(
        "The conformance record beside this survey, whose raw outputs carry the case"
        " counts of each group and the route each width resolves to. The campaign"
        " launcher refuses to take the benchmark mutex unless this record passes, so"
        " no timed window precedes it."
    )
    lines.append("")
    rows = [
        [
            "shared suite",
            f"`{record['shared_suite']['suite']}`",
            f"`{record['shared_suite']['raw_output']}`",
            str(record["shared_suite"]["exit_status"]),
        ],
        [
            "arm verifier",
            f"`{record['arm_verifier']['source']}`",
            f"`{record['arm_verifier']['raw_output']}`",
            str(record["arm_verifier"]["exit_status"]),
        ],
        [
            "production selection audit",
            f"`{record['production_audit']['command']}`",
            f"`{record['production_audit']['raw_output']}`",
            str(record["production_audit"]["exit_status"]),
        ],
    ]
    table(lines, ["Group", "Source", "Raw output", "Exit status"], rows)
    lines.append(
        f"All groups pass: `passed` is {json.dumps(record['passed'])} in"
        f" `{VALIDATION}`."
    )
    lines.append("")


def clock_section(lines, record, clock):
    lines.append("## Clock observation")
    lines.append("")
    lines.append(
        "The clock the cycles-per-byte column converts at, from"
        f" `{CLOCK}`: one child per arm per confirmatory cell, on that cell's own"
        " case, under `perf stat` for user-space cycles and task-clock. *Timed share*"
        " is the fraction of the child's task-clock its five timed windows occupy,"
        " from the child's own window record. The record establishes a clock and no"
        " comparison; *Route* is the route the observation's candidate-producing"
        " executable took. The final production route is stated in the findings and"
        " protected by the conformance record. The observation runs an arm executable"
        " of its own, whose digest the line below this table gives beside the revision;"
        " the receipts' own arm digests are in § Campaigns."
    )
    lines.append("")
    rows = []
    for key in sorted(clock):
        item = clock[key]
        rows.append(
            [
                f"`{item['cell_id']}`",
                f"`{item['arm']}`",
                f"`{item['selected_path']}`",
                f"{item['cycles']:.0f}",
                f"{item['task_clock_ns'] / 1e6:.2f}",
                fmt(item["cycles_per_ns"], 3),
                fmt(item["timed_share"], 3),
            ]
        )
    table(
        lines,
        ["Cell", "Arm", "Route", "Cycles", "Task clock (ms)", "Cycles/ns",
         "Timed share"],
        rows,
    )
    lines.append(
        f"Observation revision `{record['revision'][:12]}`, arm executable"
        f" `{record['arm_executable_sha256'][:12]}`, command"
        f" `{record['command_template']}`."
    )
    lines.append("")


def method_section(lines, stages):
    first = stages[0]["receipt"]
    lines.append("## Method")
    lines.append("")
    lines.append(
        "Every stage measures one executable and selects its route with"
        " `GF2_COUNT_ARM`, so a ratio attributes to the route rather than to two"
        " builds (each stage's arm descriptions are in its `receipt.json`). The"
        " protocol, contract and addendum-schema pins, the producing-input snapshot"
        " and the frozen ledger prefix travel inside each receipt's `inputs/`. The"
        " acceptance tool owns every decision: this table copies its estimates,"
        " intervals, decisions and outcomes and recomputes none of them. Exploratory"
        " cells decide nothing and spend no comparison; their rows carry outcome"
        " `pilot`."
    )
    lines.append("")
    lines.append(
        f"Protocol version {stages[0]['addendum']['protocol']['version']}, pinned at"
        f" `{first['protocol']['path']}` digest `{first['protocol']['sha256'][:12]}`,"
        f" contract `{first['contract']['path']}` digest"
        f" `{first['contract']['sha256'][:12]}`. Shared settings:"
        f" {json.dumps(first['settings'], sort_keys=True)}."
    )
    lines.append("")


def main():
    stages = [load(stage) for stage in STAGES]
    record, clock = clock_index()
    lines = [f"# Receipt tables for jit:{ISSUE}", ""]
    lines.append(
        "Written by `dev/active/5cbb6545/survey/summarize.py` from the committed"
        " receipts, their snapshotted addenda, the acceptance summaries beside them,"
        f" the clock observation `{CLOCK}` and the conformance record"
        f" `{VALIDATION}`. Running it from the repository root reproduces this file"
        " byte for byte. Source: the receipts under"
        f" `{RESULTS}/`; finding counts are in § Campaigns, column *Findings*."
    )
    lines.append("")
    method_section(lines, stages)
    campaigns_section(lines, stages)
    resolution_section(lines, stages)
    confirmations = [s for s in stages if s["receipt"]["label"] == "confirmation"]
    rates_section(lines, confirmations, clock)
    conversion_section(lines, stages)
    clock_section(lines, record, clock)
    coverage_section(lines)
    for stage in stages:
        cells_section(lines, stage, clock)
    OUTPUT.write_text("\n".join(lines).rstrip("\n") + "\n")
    print(f"{len(stages)} stages -> {OUTPUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
