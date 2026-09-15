#!/usr/bin/env python3
"""Project the committed profile session into attribution tables.

Usage: dev/active/53c5a8c0/survey/summarize-profile.py <profile-dir>

Reads only the session's own records - `runs.jsonl`, `counters.jsonl`, the
per-row `report/*.txt` and `host.txt` - and writes `profile.md` beside them.
Every figure is derived from those records; nothing is transcribed by hand and
nothing here is a timing comparison.

Per-operation figures divide a counter by the operation count the counted
process performed, which is `(1 + calls) x inner` for that row: the process runs
one warm-up logical call and `calls` measured ones, each logical call being the
cell's `inner` operations, and a counting session covers the whole process.

Every counter ratio is reported as the median over the session's repetitions
with the order-statistic interval `[x(2), x(8)]` of nine ordered repetitions,
which covers 96.1 percent. A row with a different repetition count reports its
own count and the widest interval its order statistics support.

A symbol's share of a row's listed cycle samples is one sampling session's
binomial share, reported with a Wilson interval at 95 percent from the exact
sample counts the report carries. Perf's rounded coverage percentage and
approximate event count are explicitly descriptive.
"""

import json
import hashlib
import math
import os
import re
import sys

SAMPLE_LINE = re.compile(
    r"^\s+([0-9.]+)%\s+(\d+)\s+(\S+)\s+\[(.)\]\s+(.*?)\s*$"
)
# perf keeps its instructions-per-cycle columns after the symbol even when the
# field list does not name them, separated by a run of spaces.
TRAILING_COLUMNS = re.compile(r"\s{2,}.*$")
TOTAL_SAMPLES = re.compile(r"^# Samples: (\S+)\s+of event '(.+)'$")
EVENT_COUNT = re.compile(r"^# Event count \(approx\.\): (\d+)$")
REPORT_HEADER = re.compile(
    r"^# cycle samples of (\S+) \((\S+) arm, cell (\S+)\)$"
)
REPORT_CALLS = re.compile(r"^# recorded by run-profile\.sh at \d+ Hz over (\d+) logical calls$")
LOST_SAMPLES = re.compile(r"^# Total Lost Samples: (\d+)$")
OFFSET = re.compile(r"^0x0*([0-9a-f]+)$")

# Repetition count whose ordered second and eighth values bracket the median at
# 96.1 percent; the session declares it and this summary checks it.
DECLARED_REPETITIONS = 9
WILSON_Z = 1.959963985
EVENTS = {
    "throughput": (
        "cycles", "instructions", "ex_ret_ops", "branches", "branch-misses",
    ),
    "memory": (
        "cycles", "ls_dispatch.ld_dispatch", "ls_dispatch.store_dispatch",
        "de_dis_dispatch_token_stalls1.int_phy_reg_file_rsrc_stall",
        "de_dis_dispatch_token_stalls1.store_queue_rsrc_stall",
    ),
}


def read_jsonl(path):
    with open(path, encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def digest(path):
    """SHA-256 of one evidence artifact."""
    hasher = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            hasher.update(block)
    return hasher.hexdigest()


def expected_rows(path):
    """Ordered profile-row declarations from the committed TSV."""
    rows = []
    with open(path, encoding="utf-8") as handle:
        for number, line in enumerate(handle, 1):
            if not line.strip() or line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) != 6:
                raise ValueError("{}:{} has {} columns, expected 6".format(
                    path, number, len(fields)
                ))
            label, cell, build, arm, crossover_path, case = fields
            rows.append({
                "label": label,
                "cell": cell,
                "build": build,
                "arm": arm,
                "crossover_path": crossover_path,
                "case": json.loads(case),
            })
    labels = [row["label"] for row in rows]
    if len(labels) != len(set(labels)):
        raise ValueError("profile-cases.tsv has duplicate labels")
    return rows


def normalized_offset(value):
    """Canonical spelling shared by perf reports and the resolution record."""
    match = OFFSET.fullmatch(value)
    return "0x{:x}".format(int(match.group(1), 16)) if match else value


def table(lines, header, rows):
    lines.append("| " + " | ".join(header) + " |")
    lines.append("|" + "|".join(["---"] * len(header)) + "|")
    for row in rows:
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")


def order_statistics(values):
    """Median and the order-statistic interval of `values`.

    For nine values the interval is the second and eighth ordered values. For
    any other count it is the widest pair that leaves one value outside on each
    side, and the caller reports the count beside it.
    """
    ordered = sorted(values)
    count = len(ordered)
    median = ordered[count // 2] if count % 2 else (
        (ordered[count // 2 - 1] + ordered[count // 2]) / 2.0
    )
    if count >= 3:
        low, high = ordered[1], ordered[-2]
    else:
        low, high = ordered[0], ordered[-1]
    return median, low, high


def fmt(value):
    if value is None:
        return "-"
    if value >= 1000:
        return "{:.0f}".format(value)
    if value >= 10:
        return "{:.2f}".format(value)
    return "{:.4f}".format(value)


def fmt_statistic(values):
    median, low, high = order_statistics(values)
    return "{} [{}, {}]".format(fmt(median), fmt(low), fmt(high))


def wilson(successes, total):
    """Wilson score interval of a binomial share at 95 percent."""
    if total == 0:
        return 0.0, 0.0, 0.0
    share = successes / total
    z2 = WILSON_Z * WILSON_Z
    denominator = 1.0 + z2 / total
    centre = (share + z2 / (2.0 * total)) / denominator
    spread = WILSON_Z * math.sqrt(
        share * (1.0 - share) / total + z2 / (4.0 * total * total)
    ) / denominator
    return share, max(0.0, centre - spread), min(1.0, centre + spread)


def counter(record, name):
    return record["counters"].get(name + ":u")


def operations(record, inner):
    """Operations the counted process performed: one warm-up call plus the
    measured calls, each of `inner` operations."""
    return (1 + record["calls"]) * inner


def derived(records, inner, definitions):
    """One column value per definition, over every repetition of one row."""
    out = []
    for _label, function in definitions:
        values = []
        for record in records:
            value = function(record, operations(record, inner))
            if value is not None:
                values.append(value)
        out.append(fmt_statistic(values) if values else "-")
    return out


THROUGHPUT = [
    ("Cycles per operation", lambda r, n: _ratio(counter(r, "cycles"), n)),
    ("Instructions per operation", lambda r, n: _ratio(counter(r, "instructions"), n)),
    ("Instructions per cycle", lambda r, n: _ratio(counter(r, "instructions"), counter(r, "cycles"))),
    ("Macro-ops per cycle", lambda r, n: _ratio(counter(r, "ex_ret_ops"), counter(r, "cycles"))),
    ("Branches per operation", lambda r, n: _ratio(counter(r, "branches"), n)),
    ("Branch misses per 1000 branches", lambda r, n: _scaled(counter(r, "branch-misses"), counter(r, "branches"), 1000)),
]

MEMORY = [
    ("Cycles per operation", lambda r, n: _ratio(counter(r, "cycles"), n)),
    ("Loads per operation", lambda r, n: _ratio(counter(r, "ls_dispatch.ld_dispatch"), n)),
    ("Stores per operation", lambda r, n: _ratio(counter(r, "ls_dispatch.store_dispatch"), n)),
    ("Load-plus-store per operation", lambda r, n: _sum_ratio(r, n)),
    ("Integer register-file stall cycles per 1000 cycles",
     lambda r, n: _scaled(counter(r, "de_dis_dispatch_token_stalls1.int_phy_reg_file_rsrc_stall"),
                          counter(r, "cycles"), 1000)),
    ("Store-queue stall cycles per 1000 cycles",
     lambda r, n: _scaled(counter(r, "de_dis_dispatch_token_stalls1.store_queue_rsrc_stall"),
                          counter(r, "cycles"), 1000)),
]


def _ratio(numerator, denominator):
    if numerator is None or not denominator:
        return None
    return numerator / denominator


def _scaled(numerator, denominator, factor):
    ratio = _ratio(numerator, denominator)
    return None if ratio is None else ratio * factor


def _sum_ratio(record, count):
    loads = counter(record, "ls_dispatch.ld_dispatch")
    stores = counter(record, "ls_dispatch.store_dispatch")
    if loads is None or stores is None or not count:
        return None
    return (loads + stores) / count


def read_resolution(directory):
    """Offset -> resolved symbol, from the session's symbol-resolution record."""
    path = os.path.join(directory, "symbol-resolution.json")
    if not os.path.isfile(path):
        return {}
    with open(path, encoding="utf-8") as handle:
        record = json.load(handle)
    return {
        (entry["object"], entry["offset"]): entry
        for entry in record["entries"]
    }


def read_report(path):
    """Symbols, exact sample counts and the report's own totals."""
    symbols, total, event, event_count = [], None, None, None
    header, calls, lost = None, None, None
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            match = REPORT_HEADER.match(line.rstrip("\n"))
            if match:
                header = match.groups()
                continue
            match = REPORT_CALLS.match(line.rstrip("\n"))
            if match:
                calls = int(match.group(1))
                continue
            match = LOST_SAMPLES.match(line.rstrip("\n"))
            if match:
                lost = int(match.group(1))
                continue
            match = TOTAL_SAMPLES.match(line.rstrip("\n"))
            if match:
                total, event = match.group(1), match.group(2)
                continue
            match = EVENT_COUNT.match(line.rstrip("\n"))
            if match:
                event_count = int(match.group(1))
                continue
            match = SAMPLE_LINE.match(line.rstrip("\n"))
            if match:
                symbols.append({
                    "share": float(match.group(1)) / 100.0,
                    "samples": int(match.group(2)),
                    "object": match.group(3),
                    "level": match.group(4),
                    "symbol": TRAILING_COLUMNS.sub("", match.group(5)),
                })
    return symbols, {
        "header": header,
        "calls": calls,
        "lost": lost,
        "displayed_total": total,
        "event": event,
        "event_count": event_count,
    }


def validate(directory, declarations, runs_list, counters, resolution):
    """Reject incomplete or internally inconsistent profile evidence.

    The validation uses only committed-shaped session files. It deliberately
    does not reopen the profiled executables or host libraries: their observed
    identities remain immutable strings and digests in the session records,
    so this generator reproduces on a fresh checkout.
    """
    problems = []
    labels = [row["label"] for row in declarations]
    runs_by_label = {}
    for run in runs_list:
        label = run.get("label")
        if label in runs_by_label:
            problems.append("duplicate run record for {}".format(label))
        runs_by_label[label] = run
    if set(runs_by_label) != set(labels):
        problems.append("run labels differ from profile-cases.tsv: missing={} extra={}".format(
            sorted(set(labels) - set(runs_by_label)),
            sorted(set(runs_by_label) - set(labels)),
        ))
    if [run.get("label") for run in runs_list] != labels:
        problems.append("run record order differs from profile-cases.tsv")

    for declared in declarations:
        label = declared["label"]
        run = runs_by_label.get(label)
        if run is None:
            continue
        for field in ("cell", "build", "arm", "crossover_path", "case"):
            if run.get(field) != declared[field]:
                problems.append("{} run {} differs from profile-cases.tsv".format(label, field))
        if run.get("schema") != "clmul-crossover-profile-run-v1":
            problems.append("{} has the wrong run schema".format(label))
        if run.get("warmup_calls") != 1 or run.get("measured_calls", 0) <= 0:
            problems.append("{} has an invalid warmup or measured call count".format(label))
        inner = run.get("inner_per_call")
        expected_total = (1 + run.get("measured_calls", 0)) * inner if inner else None
        if expected_total != run.get("total_operations"):
            problems.append("{} total_operations does not match calls and inner".format(label))
        if not run.get("selected_path"):
            problems.append("{} records no runtime-selected path".format(label))

    counter_keys = set()
    grouped = {}
    for record in counters:
        key = (record.get("label"), record.get("event_set"), record.get("repetition"))
        if key in counter_keys:
            problems.append("duplicate counter record {}".format(key))
        counter_keys.add(key)
        grouped.setdefault((record.get("label"), record.get("event_set")), []).append(record)
        label = record.get("label")
        run = runs_by_label.get(label)
        if run is None:
            problems.append("counter record names undeclared row {}".format(label))
            continue
        if record.get("schema") != "clmul-crossover-profile-counters-v1":
            problems.append("{} has the wrong counter schema".format(key))
        for field in ("cell", "build", "arm", "crossover_path"):
            if record.get(field) != run.get(field):
                problems.append("{} counter {} differs from its run".format(label, field))
        if record.get("calls") != run.get("measured_calls"):
            problems.append("{} counter calls differ from its run".format(label))
        for event in EVENTS.get(record.get("event_set"), ()):
            value = record.get("counters", {}).get(event + ":u")
            enabled = record.get("counters", {}).get(event + ":u.enabled_pct")
            if value is None or value < 0:
                problems.append("{} has no usable {} counter".format(key, event))
            if enabled is None or not (0 < enabled <= 100):
                problems.append("{} has invalid {} enablement".format(key, event))

    expected_counter_keys = {
        (label, event_set, repetition)
        for label in labels
        for event_set in EVENTS
        for repetition in range(1, DECLARED_REPETITIONS + 1)
    }
    if counter_keys != expected_counter_keys:
        problems.append("counter record keys differ from the declared matrix: missing={} extra={}".format(
            sorted(expected_counter_keys - counter_keys),
            sorted(counter_keys - expected_counter_keys),
        ))

    for label in labels:
        for event_set in EVENTS:
            records = grouped.get((label, event_set), [])
            repetitions = sorted(record.get("repetition") for record in records)
            if repetitions != list(range(1, DECLARED_REPETITIONS + 1)):
                problems.append("{} {} repetitions are {}".format(
                    label, event_set, repetitions
                ))

    report_dir = os.path.join(directory, "report")
    report_names = sorted(name for name in os.listdir(report_dir) if name.endswith(".txt"))
    expected_reports = sorted(label + ".txt" for label in labels)
    if report_names != expected_reports:
        problems.append("profile reports differ from declarations: missing={} extra={}".format(
            sorted(set(expected_reports) - set(report_names)),
            sorted(set(report_names) - set(expected_reports)),
        ))

    offsets = set()
    for declared in declarations:
        label = declared["label"]
        path = os.path.join(report_dir, label + ".txt")
        if not os.path.isfile(path):
            continue
        symbols, meta = read_report(path)
        expected_header = (label, declared["arm"], declared["cell"])
        if meta["header"] != expected_header:
            problems.append("{} report header is {}".format(label, meta["header"]))
        run = runs_by_label.get(label, {})
        if meta["calls"] != run.get("measured_calls"):
            problems.append("{} report call count differs from its run".format(label))
        if meta["lost"] != 0:
            problems.append("{} report lost {} samples".format(label, meta["lost"]))
        if meta["event"] != "cycles:u" or not meta["event_count"]:
            problems.append("{} report has incomplete cycle metadata".format(label))
        if not symbols or sum(entry["samples"] for entry in symbols) <= 0:
            problems.append("{} report has no listed samples".format(label))
        if sum(entry["share"] for entry in symbols) > 1.01:
            problems.append("{} report shares exceed 101 percent".format(label))
        offsets.update(
            (entry["object"], normalized_offset(entry["symbol"]))
            for entry in symbols if OFFSET.fullmatch(entry["symbol"])
        )

    resolution_keys = set()
    for entry in resolution.values():
        key = (entry.get("object"), normalized_offset(entry.get("offset", "")))
        if key in resolution_keys:
            problems.append("duplicate symbol resolution {}".format(key))
        resolution_keys.add(key)
        if not entry.get("object_sha256") or not entry.get("object_path"):
            problems.append("symbol resolution {} pins no object identity".format(key))
        if not entry.get("symbol"):
            problems.append("symbol resolution {} remains unresolved".format(key))
    if offsets != resolution_keys:
        problems.append("symbol resolutions differ from report offsets: missing={} extra={}".format(
            sorted(offsets - resolution_keys), sorted(resolution_keys - offsets)
        ))

    log_path = os.path.join(directory, "profile.log")
    with open(log_path, encoding="utf-8") as handle:
        log = handle.read().splitlines()
    identities = [line for line in log if line.startswith("identity ")]
    if len(identities) != 1:
        problems.append("profile log has {} identity lines".format(len(identities)))
    case_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "profile-cases.tsv")
    if identities and "profile-cases.tsv={}".format(digest(case_path)) not in identities[0]:
        problems.append("profile log does not pin the current profile-cases.tsv")
    for label in labels:
        done = sum(line.startswith("row {} done ".format(label)) for line in log)
        if done != 1:
            problems.append("profile log has {} completion records for {}".format(done, label))
    if not log or not log[-1].startswith("session complete "):
        problems.append("profile log has no terminal complete record")
    if any("calibration found no readable" in line for line in log):
        problems.append("profile log records a failed calibration")

    host_path = os.path.join(directory, "host.txt")
    with open(host_path, encoding="utf-8") as handle:
        host = handle.read()
    for heading in ("## uname", "## perf", "## lscpu", "## smt", "## governors",
                    "## shared objects the arms resolve", "## load average at start"):
        if heading not in host:
            problems.append("host.txt lacks {}".format(heading))

    transient = sorted(
        os.path.relpath(os.path.join(root, name), directory)
        for root, _dirs, names in os.walk(directory)
        for name in names
        if name.startswith(".stat-") or name.endswith(".data")
    )
    if transient:
        problems.append("profile directory retains transient files: {}".format(transient))
    if problems:
        raise ValueError("profile validation failed:\n- " + "\n- ".join(problems))

    artifacts = {}
    for name in ("profile.log", "runs.jsonl", "counters.jsonl", "host.txt",
                 "symbol-resolution.json"):
        artifacts[name] = digest(os.path.join(directory, name))
    for name in report_names:
        artifacts["report/" + name] = digest(os.path.join(report_dir, name))
    return {
        "schema": "clmul-crossover-profile-validation-v1",
        "passed": True,
        "artifacts": artifacts,
        "checks": {
            "declared_rows": len(labels),
            "run_records": len(runs_list),
            "counter_records": len(counters),
            "counter_sets_per_row": len(EVENTS),
            "repetitions_per_set": DECLARED_REPETITIONS,
            "reports": len(report_names),
            "resolved_offsets": len(resolution_keys),
            "lost_samples": 0,
            "terminal_complete": True,
        },
    }


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    directory = os.path.abspath(sys.argv[1])
    case_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "profile-cases.tsv")
    declarations = expected_rows(case_path)
    runs_list = read_jsonl(os.path.join(directory, "runs.jsonl"))
    runs = {record["label"]: record for record in runs_list}
    counters = read_jsonl(os.path.join(directory, "counters.jsonl"))
    order = [row["label"] for row in declarations]
    resolution = read_resolution(directory)
    validation = validate(directory, declarations, runs_list, counters, resolution)
    validation_path = os.path.join(directory, "validation.json")
    with open(validation_path, "w", encoding="utf-8") as handle:
        json.dump(validation, handle, indent=2, sort_keys=True)
        handle.write("\n")

    grouped = {}
    for record in counters:
        grouped.setdefault((record["label"], record["event_set"]), []).append(record)

    lines = [
        "# Profiler attribution of the measured carry-less paths",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `dev/active/53c5a8c0/survey/summarize-profile.py` from this session's",
        "own records in this directory. Nothing here is a timing comparison and no receipt",
        "depends on it: the receipts answer which path is faster, and these tables answer",
        "where a path's cycles go.",
        "",
        "`validation.json` records a passing structural check and SHA-256 identity for every",
        "session log, run record, counter record, symbol report and symbol-resolution input",
        "used below. The generator refuses incomplete rows, counters, reports, runtime path",
        "records, lost samples, unresolved offsets and a nonterminal session.",
        "",
        "A per-operation figure divides a counter by `(1 + calls) x inner`, the operations the",
        "counted process performed: one warm-up logical call plus the measured ones, each",
        "logical call being the cell's `inner` operations. Each counter ratio is the median",
        "over the session's repetitions with the order-statistic interval of the ordered",
        "repetitions; the repetition count is in the row-inventory table. A symbol's share of",
        "a row's listed cycle samples carries a Wilson interval at 95 percent over the exact",
        "listed counts. Perf's rounded coverage percentage and approximate event count are",
        "descriptive one-run observations.",
        "",
        "## Row inventory",
        "",
        "Source: `runs.jsonl` and `counters.jsonl`. The selected path is what the arm reported",
        "at run time, not what a build setting implies.",
        "",
    ]

    rows = []
    for label in order:
        run = runs[label]
        sets = {name: grouped.get((label, name), []) for name in ("throughput", "memory")}
        repetitions = sorted({len(records) for records in sets.values() if records})
        calls = sorted({record["calls"] for records in sets.values() for record in records})
        enabled = []
        for records in sets.values():
            for record in records:
                enabled += [
                    value for key, value in record["counters"].items()
                    if key.endswith(".enabled_pct") and value is not None
                ]
        rows.append([
            "`{}`".format(label),
            "`{}`".format(run["cell"]),
            run["arm"],
            run["build"],
            "`{}`".format(run["selected_path"].split(" library=")[0]),
            str(run["inner_per_call"]),
            ", ".join(str(value) for value in calls),
            ", ".join(str(value) for value in repetitions),
            "{:.2f}".format(min(enabled)) if enabled else "-",
        ])
    table(lines, [
        "Row", "Cell", "Arm", "Build", "Selected path", "Operations per call",
        "Logical calls counted", "Repetitions", "Lowest counter enablement %",
    ], rows)

    if any(row[-2] != str(DECLARED_REPETITIONS) for row in rows):
        lines += [
            "At least one row carries a repetition count other than the {} the session "
            "declares, so its interval is the widest its order statistics support rather "
            "than the declared 96.1 percent bracket.".format(DECLARED_REPETITIONS),
            "",
        ]

    for title, definitions, event_set in (
        ("Instruction throughput and branch behaviour", THROUGHPUT, "throughput"),
        ("Load and store traffic and dispatch-token stalls", MEMORY, "memory"),
    ):
        lines += ["## {}".format(title), "",
                  "Source: the `{}` event set of `counters.jsonl`.".format(event_set), ""]
        rows = []
        for label in order:
            records = grouped.get((label, event_set), [])
            if not records:
                continue
            rows.append(
                ["`{}`".format(label), "`{}`".format(runs[label]["cell"]), runs[label]["arm"]]
                + derived(records, runs[label]["inner_per_call"], definitions)
            )
        table(lines, ["Row", "Cell", "Arm"] + [name for name, _ in definitions], rows)

    lines += [
        "## Cycle attribution by symbol",
        "",
        "Source: the per-row reports under `report/`, and `symbol-resolution.json` where an",
        "object carries no dynamic symbol for an offset. A symbol below the session's report",
        "threshold is elided, so the covered share states how much of the row's samples the",
        "listed symbols account for. An offset resolved through the object's own symbol table",
        "names the covering symbol and the offset into it, with the offset it was resolved",
        "from; `dev/active/53c5a8c0/survey/resolve-symbols.py` records the object path and",
        "digest each resolution used.",
        "",
    ]
    rows = []
    for label in order:
        path = os.path.join(directory, "report", label + ".txt")
        symbols, meta = read_report(path)
        sample_total = sum(entry["samples"] for entry in symbols)
        covered = sum(entry["share"] for entry in symbols)
        for entry in symbols:
            share, low, high = wilson(entry["samples"], sample_total)
            name = entry["symbol"]
            resolved = resolution.get((entry["object"], normalized_offset(name)))
            if resolved is not None and resolved.get("symbol"):
                name = "{} + {} (resolved from {})".format(
                    resolved["symbol"], resolved["offset_into_symbol"], name
                )
            rows.append([
                "`{}`".format(label),
                "`{}`".format(runs[label]["cell"]) if label in runs else "-",
                "`{}`".format(entry["object"]),
                entry["level"],
                "`{}`".format(name),
                str(entry["samples"]),
                str(sample_total),
                "{:.2f} [{:.2f}, {:.2f}]".format(100.0 * share, 100.0 * low, 100.0 * high),
                "{:.2f}".format(100.0 * covered),
                "{}".format(meta["event_count"]),
            ])
    table(lines, [
        "Row", "Cell", "Object", "Level", "Symbol", "Samples", "Listed samples in row",
        "Share of listed samples % [Wilson 95%]",
        "Descriptive listed share of all samples %",
        "Descriptive approximate cycles recorded (one run)",
    ], rows)

    with open(os.path.join(directory, "host.txt"), encoding="utf-8") as handle:
        host = [line.rstrip("\n") for line in handle]
    lines += [
        "## Host and session identity",
        "",
        "Source: `host.txt` and `profile.log` in this directory. The identity line of the "
        "log pins the executables and the case table the session ran.",
        "",
    ]
    table(lines, ["Field", "Value"], [
        [line.split(":", 1)[0].lstrip("# ").strip(), line.split(":", 1)[1].strip()]
        for line in host
        if line.startswith("# ") and ":" in line
    ])

    out = os.path.join(directory, "profile.md")
    with open(out, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines).rstrip("\n") + "\n")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
