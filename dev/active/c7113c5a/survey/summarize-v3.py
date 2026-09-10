#!/usr/bin/env python3
"""Project the c7113c5a receipts and the dot-reduction diagnostic into tables.

Usage: summarize-v3.py <output.md> [--dot-diagnostic <dir>] <receipt-dir>...

Every number below is read from a receipt, its acceptance summary or the
diagnostic directory named on the command line; nothing is transcribed. Each
receipt directory must hold receipt.json and acceptance-summary.json written by
benchmark-acceptance. The speedup and its interval are the acceptance tool's
recomputation. Every other figure is a median of per-execution values, or a
quotient of such medians, with a percentile bootstrap interval computed here:
each draw resamples, with replacement and independently, the pairs of every
receipt cell the figure uses and the processes of the diagnostic, then
recomputes the figure. The draws come from Python's `random.Random`
(Mersenne Twister) seeded with SEED and the figure's label.
"""

import argparse
import hashlib
import json
import platform
import random
import re
import statistics

DRAWS = 10000
CONFIDENCE = 0.95
SEED = 20260911
V3_MANIFEST = "dev/active/c7113c5a/survey/producing-inputs.json"
V3_ARM_SOURCE = "38d2c091"
CONVERSION_KEYS = ("setup_ns", "pack_ns", "batch_fill_ns", "dispatch_ns", "unpack_ns")


def load(directory):
    with open(f"{directory}/receipt.json", "rb") as handle:
        raw = handle.read()
    with open(f"{directory}/acceptance-summary.json") as handle:
        summary = json.load(handle)
    return json.loads(raw), summary, hashlib.sha256(raw).hexdigest()


def plan_cases(directory):
    with open(f"{directory}/plan.json") as handle:
        return {cell["cell_id"]: cell["case"] for cell in json.load(handle)["cells"]}


def fmt(value, digits=4):
    if value is None:
        return "-"
    return f"{value:.{digits}g}"


def unit_of(value):
    for unit, scale in (("ms", 1e6), ("us", 1e3)):
        if value >= scale:
            return unit, scale
    return "ns", 1.0


def ns_interval(estimate):
    point, lower, upper = estimate
    unit, scale = unit_of(point)
    return f"{point / scale:.4g} [{lower / scale:.4g}, {upper / scale:.4g}] {unit}"


def interval(estimate, digits=4):
    point, lower, upper = estimate
    return f"{point:.{digits}g} [{lower:.{digits}g}, {upper:.{digits}g}]"


def bootstrap(label, groups, statistic):
    """Point estimate and percentile interval of `statistic` over `groups`.

    `groups` maps a name to its resampling units (a cell's pairs or the
    diagnostic's processes); every draw resamples each group's units with
    replacement, independently across groups.
    """
    rng = random.Random(f"{SEED}:{label}")
    draws = sorted(
        statistic({name: rng.choices(units, k=len(units)) for name, units in groups.items()})
        for _ in range(DRAWS))
    tail = round(DRAWS * (1 - CONFIDENCE) / 2)
    return statistic(groups), draws[tail], draws[DRAWS - 1 - tail]


def arm_median(pairs, side):
    return statistics.median(pair[side]["ns_per_call"] for pair in pairs)


def conversion_median(pairs, side, key):
    return statistics.median(pair[side]["conversion"][key] for pair in pairs)


ARM_ESTIMATES = {}


def arm_estimate(directory, cell, side):
    key = (directory, cell["cell_id"], side)
    if key not in ARM_ESTIMATES:
        ARM_ESTIMATES[key] = bootstrap(f"{directory}:{cell['cell_id']}:{side}", {"cell": cell["pairs"]},
                                       lambda groups: arm_median(groups["cell"], side))
    return ARM_ESTIMATES[key]


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
    return sorted({pair[side].get("selected_path") or "-" for pair in cell["pairs"]})


def method(command):
    return ["## Method", "",
            f"Command: `{command}`.", "",
            "The speedup and its interval are the acceptance tool's paired bootstrap at the receipt's "
            "per-comparison confidence. Every other figure is a median of per-execution values (one value per "
            "arm and pair), or a quotient of such medians, with a "
            f"{CONFIDENCE:.0%} percentile bootstrap interval: {DRAWS} draws each resample, with replacement "
            "and independently, the pairs of every cell the figure uses (and the processes of the "
            "dot-reduction diagnostic), then recompute the figure. Draws come from Python "
            f"{platform.python_version()} `random.Random` (Mersenne Twister) seeded with `{SEED}:<figure "
            "label>`. These intervals are explanatory: they carry no family-wise correction and no "
            "comparison is confirmed by them.", ""]


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
             "gf2 is the baseline and gf2x the candidate: a speedup above 1 means gf2x is faster. Per-arm "
             f"medians carry {CONFIDENCE:.0%} bootstrap intervals over the cell's pairs.", "",
             "`Flagged` is the acceptance tool's count under the receipt's protocol version; `Per-execution` "
             "recounts the raw windows under the protocol-v3 rule.", "",
             "| Cell | Pairs | gf2 median | gf2x median | Speedup | Interval | gf2 faster by | Decision | Outcome | Flagged | Per-execution | gf2 path |",
             "|---|---:|---|---|---:|---|---:|---|---|---:|---:|---|"]
    for cell in receipt["cells"]:
        verdict = verdicts[cell["cell_id"]]
        estimate_interval = verdict.get("interval")
        if not cell["pairs"] or estimate_interval is None:
            lines.append(f"| `{cell['cell_id']}` | 0 | - | - | - | - | - | - | {verdict['outcome']} | - | - | {cell.get('unavailable_reason') or '-'} |")
            continue
        estimate = estimate_interval["estimate"]
        faster = (f"{1 / estimate:.4g} [{1 / estimate_interval['upper']:.4g}, {1 / estimate_interval['lower']:.4g}]"
                  if estimate < 1 else "-")
        flagged, total = execution_flags(cell)
        lines.append(
            f"| `{cell['cell_id']}` | {len(cell['pairs'])} | {ns_interval(arm_estimate(directory, cell, 'baseline'))} | "
            f"{ns_interval(arm_estimate(directory, cell, 'candidate'))} | {estimate:.4g} | "
            f"[{estimate_interval['lower']:.4g}, {estimate_interval['upper']:.4g}] "
            f"at {estimate_interval['confidence']:.5f} | {faster} | {verdict.get('decision') or '-'} | {verdict['outcome']} | "
            f"{verdict['flagged_windows']}/{verdict['total_windows']} | {flagged}/{total} | "
            f"`{'; '.join(paths(cell, 'baseline'))}` |")
    return lines


def conversion_semantics():
    return ["## Costs outside the timed windows", "",
            f"The v3 receipts' conversion fields come from the arm source of survey commit `{V3_ARM_SOURCE}`, "
            "which every v3 receipt's producing closure pins. Each arm reports one value per execution; the "
            f"tables give the median over the cell's executions with its {CONFIDENCE:.0%} bootstrap interval. "
            "What each field measured:", "",
            "- `setup`: in the gf2 long-product cells, zeroing the destination, a step of the schoolbook call "
            "rather than state (the gf2 arm builds none); in the gf2 raw batch, an empty timed region; in the "
            "gf2 dot product, `Gf2mField::new`; in every gf2x cell, `gf2x_mul_pool_init` and "
            "`gf2x_mul_pool_clear`, which zero a pool handle and free a null pointer (`gf2x-pool-init-zeroes`); "
            "gf2x allocates its scratch inside `gf2x_mul_r` (`gf2x-pool-lazy-alloc`).",
            "- `pack` and `batch fill`: dot product only, building the operand representation and one "
            "complete call.",
            "- `dispatch`: gf2's two runtime capability detections.",
            "- `unpack`: in the 4-word and 9-word cells, the `BarrettReducerWide` reduction of the arm's own "
            "unreduced product, amortised over 4096 repetitions and rounded up; in the dot product, the "
            "Barrett reduction of the already reduced dot product, which returns at the reducer's early exit "
            "(`barrett-early-exit`), so these values, marked `superseded`, do not measure the reduction and "
            "the dot-reduction diagnostic below replaces them; in the gf2 raw batch, copying a product buffer "
            "the probe instance never filled into a destination it had not touched.", "",
            "Every field other than the amortised reductions is one pass taken in a fresh process before the "
            "first timed window, so it includes first-touch and cold-code effects; those medians are "
            "descriptive and support no claim. `-` marks a field the arm does not probe for the cell. The v1 "
            "receipts' conversion fields come from the v1 arm source and are not tabulated.", ""]


def conversion_table(directory, receipt):
    lines = [f"### `{directory}`", "",
             "| Cell | Arm | Executions | setup | pack | batch fill | dispatch | unpack |",
             "|---|---|---:|---|---|---|---|---|"]
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            pairs = [pair for pair in cell["pairs"] if pair[side].get("conversion")]
            if not pairs:
                continue
            fields = []
            for key in CONVERSION_KEYS:
                if all(pair[side]["conversion"][key] == 0 for pair in pairs):
                    fields.append("-")
                    continue
                estimate = bootstrap(f"{directory}:{cell['cell_id']}:{side}:{key}", {"cell": pairs},
                                     lambda groups, key=key: conversion_median(groups["cell"], side, key))
                text = ns_interval(estimate)
                if key == "unpack_ns" and cell["cell_id"].startswith("internal-gf2m-dot"):
                    text += " superseded"
                fields.append(text)
            lines.append(f"| `{cell['cell_id']}` | `{pairs[0][side]['arm']}` | {len(pairs)} | " + " | ".join(fields) + " |")
    return lines


def per_product_table(directory, receipt):
    lines = [f"### Cost per schoolbook word product: `{directory}`", "",
             "Each arm's median call divided by the $N^2$ word products the schoolbook definition performs per "
             "operation, times `inner` and the worker count. For gf2x this is a normalised rate, because its "
             "recursion avoids most of those products.", "",
             "| Cell | Words | Workers | Products per call | Pairs | gf2 ns per product | gf2x ns per product |",
             "|---|---:|---:|---:|---:|---|---|"]
    cases = plan_cases(directory)
    for cell in receipt["cells"]:
        case = cases[cell["cell_id"]]
        if case["kind"] != "poly-mul" or not cell["pairs"]:
            continue
        workers = cell["pairs"][0]["baseline"]["workers_observed"]
        products = case["words"] ** 2 * case["inner"] * workers
        rates = [interval(tuple(value / products for value in arm_estimate(directory, cell, side)))
                 for side in ("baseline", "candidate")]
        lines.append(f"| `{cell['cell_id']}` | {case['words']} | {workers} | {products} | {len(cell['pairs'])} | "
                     f"{rates[0]} | {rates[1]} |")
    return lines


def estimates_table(directory, receipt):
    """Quotients of medians from several cells of one native-family receipt,
    each with a bootstrap interval that resamples every cell it uses."""
    cells = {cell["cell_id"]: cell["pairs"] for cell in receipt["cells"] if cell["pairs"]}
    batch, w4, w9 = "internal-clmul-batch-1024-1core", "poly-mul-4w-1core", "poly-mul-9w-1core"
    w256, w2048 = "poly-mul-256w-1core", "poly-mul-2048w-streaming-1core"
    public = "poly-mul-4w-public-api-1core"
    if not all(name in cells for name in (batch, w4, w9, w256, w2048, public)):
        return []
    cases = plan_cases(directory)

    def gf2(groups, name):
        return arm_median(groups[name], "baseline")

    def gf2x(groups, name):
        return arm_median(groups[name], "candidate")

    def hardware(groups):
        return gf2(groups, batch) / cases[batch]["count"]

    def scalar(groups, name):
        return gf2(groups, name) / cases[name]["words"] ** 2

    # A single-cell row reuses that arm's median interval, scaled.
    scaled = [
        ("gf2 sequential-PCLMULQDQ word product in the raw batch, unpack included", batch, "baseline",
         cases[batch]["count"]),
        ("gf2 YMM 4-limb kernel per schoolbook word product", w4, "baseline", 16),
        ("gf2 scalar schoolbook word product at 256 words", w256, "baseline", 256 ** 2),
        ("gf2 scalar schoolbook word product at 2048 words", w2048, "baseline", 2048 ** 2),
        ("gf2x one-word gf2x_mul_r call in the raw-batch arm", batch, "candidate", cases[batch]["count"]),
        ("gf2x four-word gf2x_mul_r call", w4, "candidate", 1),
    ]
    by_id = {cell["cell_id"]: cell for cell in receipt["cells"]}
    rows = [
        ("instruction factor: scalar word product at 256 words over the raw-batch word product", [w256, batch],
         lambda g: scalar(g, w256) / hardware(g)),
        ("256-word gap divided by that instruction factor", [w256, batch],
         lambda g: gf2(g, w256) / gf2x(g, w256) / (scalar(g, w256) / hardware(g))),
        ("2048-word gap divided by the 2048-word instruction factor", [w2048, batch],
         lambda g: gf2(g, w2048) / gf2x(g, w2048) / (scalar(g, w2048) / hardware(g))),
        ("gf2 public API over dispatched kernel at 4 words", [public, w4],
         lambda g: gf2(g, public) / gf2(g, w4)),
    ]
    for words, name in ((4, w4), (9, w9)):
        for side, arm in (("baseline", "gf2"), ("candidate", "gf2x")):
            rows.append((f"{words}-word {arm}: separated reduction over unreduced product plus reduction", [name],
                         lambda g, name=name, side=side: conversion_median(g[name], side, "unpack_ns")
                         / (arm_median(g[name], side) + conversion_median(g[name], side, "unpack_ns"))))
    for name in ("poly-mul-256w-6core", "poly-mul-256w-12core", "poly-mul-256w-24smt"):
        if name not in cells:
            continue
        workers = cells[name][0]["baseline"]["workers_observed"]
        scale = cases[name]["inner"] * workers
        for side, arm in (("baseline", "gf2"), ("candidate", "gf2x")):
            rows.append((f"{workers}-worker aggregate throughput over one core, {arm}", [w256, name],
                         lambda g, name=name, side=side, scale=scale:
                         arm_median(g[w256], side) * scale / arm_median(g[name], side)))
    lines = [f"### Derived estimates: `{directory}`", "",
             "Quotients of per-arm medians from the named cells of this receipt, 24 pairs each; the interval "
             "resamples every named cell.", "",
             "| Quantity | Cells | Value [95% interval] |", "|---|---|---|"]
    for label, name, side, divisor in scaled:
        estimate = tuple(value / divisor for value in arm_estimate(directory, by_id[name], side))
        lines.append(f"| {label} | `{name}` | {ns_interval(estimate)} |")
    for label, names, statistic in rows:
        estimate = bootstrap(f"{directory}:{label}", {name: cells[name] for name in names}, statistic)
        lines.append(f"| {label} | {', '.join(f'`{name}`' for name in names)} | {interval(estimate)} |")
    return lines


def diagnostic_table(directory, rows):
    """The dot-reduction diagnostic: per-reduction medians over every
    repetition of every process, resampled by process."""
    with open(f"{directory}/diagnostic.jsonl") as handle:
        records = [json.loads(line) for line in handle if line.strip()]
    with open(f"{directory}/identity.json") as handle:
        identity = json.load(handle)
    fixture = records[0]["fixture"]
    assert all(record["fixture"] == fixture for record in records), "processes measured different fixtures"
    per_reduction = {probe: [[value / record["reductions_per_value"] for value in record["total_ns"][probe]]
                             for record in records]
                     for probe in records[0]["total_ns"]}
    repetitions = sorted({record["repetitions"] for record in records})

    def pooled(groups, probe):
        return statistics.median(value for process in groups[probe] for value in process)

    confirmation = next((receipt for _, (receipt, summary, _) in rows
                         if summary["label"] == "confirmation"
                         and receipt.get("family_id") == "polynomial-multiplication-baselines"
                         and receipt["source"]["producing"]["manifest_path"] == V3_MANIFEST), None)
    dot = next(cell for cell in confirmation["cells"] if cell["cell_id"] == "internal-gf2m-dot-1024-1core")
    lines = [f"## Dot-product reduction diagnostic: `{directory}`", "",
             f"Revision `{identity['revision']}`, executable SHA-256 `{identity['executable']['sha256']}`, "
             f"gf2x library SHA-256 `{identity['gf2x_library']['sha256']}`, {identity['toolchain'][0]}, "
             f"RUSTFLAGS `{identity['rustflags']}`; host `{records[0]['host']['hostname']}`, CPUs "
             f"{'; '.join(sorted({','.join(map(str, record['cpus_observed'])) for record in records}))}. Fixture: case "
             f"`{json.dumps(fixture['case'])}`, worker {fixture['worker']}, bank {fixture['bank']} "
             f"({fixture['generator']}); raw accumulator `{fixture['accumulator']}` of degree "
             f"{fixture['accumulator_degree']}, reduced dot product {fixture['reduced']}. "
             f"{len(records)} processes of {', '.join(map(str, repetitions))} repetitions; each value is the "
             f"mean of {records[0]['reductions_per_value']} reductions. The interval resamples processes.", "",
             "| Probe | What it reduces | Values | ns per reduction [95% interval] |", "|---|---|---:|---|"]
    for probe in ("consumer", "composed", "superseded"):
        text = records[0]["probes"][probe]
        estimate = bootstrap(f"{directory}:{probe}", {probe: per_reduction[probe]},
                             lambda groups, probe=probe: pooled(groups, probe))
        count = sum(len(process) for process in per_reduction[probe])
        lines.append(f"| `{probe}` | {text} | {count} | {interval(estimate, 5)} |")
    lines += ["", "| Share of the timed dot-product call | Value [95% interval] |", "|---|---|"]
    for probe, side, arm in (("consumer", "baseline", "gf2"), ("composed", "candidate", "gf2x")):
        estimate = bootstrap(f"{directory}:{probe}:share", {probe: per_reduction[probe], "cell": dot["pairs"]},
                             lambda groups, probe=probe, side=side:
                             pooled(groups, probe) / arm_median(groups["cell"], side))
        lines.append(f"| `{probe}` reduction over the {arm} arm's median call in "
                     f"`internal-gf2m-dot-1024-1core` | {interval(estimate)} |")
    return lines


def ladder_table(rows):
    """Conservative, tuned and native legs of the two ladder sizes, each from
    its own confirmation cell, with each level's per-arm median over native."""
    confirmations = {summary["family"]["family_id"]: (directory, receipt, summary)
                     for directory, (receipt, summary, _) in rows
                     if summary["label"] == "confirmation" and receipt.get("family_id")
                     and receipt["source"]["producing"]["manifest_path"] == V3_MANIFEST}
    targeting = confirmations.get("polynomial-host-targeting")
    native = confirmations.get("polynomial-multiplication-baselines")
    if not (targeting and native):
        return []
    lines = ["## Host-targeting ladder", "",
             "Each leg is its own cell with its own pairs; conservative and tuned come from the "
             "host-targeting confirmation, native from the native-family confirmation. `Over native` divides "
             "the leg's per-arm median by the native leg's, resampling both cells.", "",
             "| Size | Level | gf2 median | gf2x median | Speedup | Interval | Outcome | gf2 over native | gf2x over native |",
             "|---|---|---|---|---:|---|---|---|---|"]
    legs = [("conservative", targeting, "poly-mul-{w}w-conservative-1core"),
            ("tuned", targeting, "poly-mul-{w}w-tuned-1core"),
            ("native", native, "poly-mul-{w}w-1core")]
    for words in (4, 256):
        native_cell = next(c for c in native[1]["cells"] if c["cell_id"] == f"poly-mul-{words}w-1core")
        for level, (directory, receipt, summary), pattern in legs:
            cell_id = pattern.format(w=words)
            cell = next(c for c in receipt["cells"] if c["cell_id"] == cell_id)
            verdict = next(c for c in summary["cells"] if c["cell_id"] == cell_id)
            estimate_interval = verdict["interval"]
            ratios = ["-", "-"]
            if level != "native":
                ratios = [interval(bootstrap(f"ladder:{cell_id}:{side}:over-native",
                                             {"leg": cell["pairs"], "native": native_cell["pairs"]},
                                             lambda g, side=side: arm_median(g["leg"], side)
                                             / arm_median(g["native"], side)))
                          for side in ("baseline", "candidate")]
            lines.append(f"| {words} words | {level} | {ns_interval(arm_estimate(directory, cell, 'baseline'))} | "
                         f"{ns_interval(arm_estimate(directory, cell, 'candidate'))} | {estimate_interval['estimate']:.4g} | "
                         f"[{estimate_interval['lower']:.4g}, {estimate_interval['upper']:.4g}] at "
                         f"{estimate_interval['confidence']:.5f} | {verdict['outcome']} | {ratios[0]} | {ratios[1]} |")
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output")
    parser.add_argument("--dot-diagnostic")
    parser.add_argument("receipts", nargs="+")
    args = parser.parse_args()
    rows = [(directory.rstrip("/"), load(directory.rstrip("/"))) for directory in args.receipts]
    command = " ".join(["dev/active/c7113c5a/survey/summarize-v3.py", args.output]
                       + (["--dot-diagnostic", args.dot_diagnostic] if args.dot_diagnostic else [])
                       + [directory for directory, _ in rows])
    lines = ["# Polynomial-multiplication survey tables (jit:c7113c5a)", "",
             "Generated by `dev/active/c7113c5a/survey/summarize-v3.py` from the directories below.", ""]
    lines += method(command)
    lines += overview(rows) + [""]
    lines += ["## Cells", ""]
    for directory, (receipt, summary, _) in rows:
        lines += cells_table(directory, receipt, summary) + [""]
    v3_confirmations = [(directory, receipt) for directory, (receipt, summary, _) in rows
                        if summary["label"] == "confirmation"
                        and receipt["source"]["producing"]["manifest_path"] == V3_MANIFEST]
    if v3_confirmations:
        lines += conversion_semantics()
        for directory, receipt in v3_confirmations:
            lines += conversion_table(directory, receipt) + [""]
    if args.dot_diagnostic:
        lines += diagnostic_table(args.dot_diagnostic.rstrip("/"), rows) + [""]
    for directory, (receipt, summary, _) in rows:
        if summary["label"] == "confirmation":
            lines += per_product_table(directory, receipt) + [""]
    for directory, receipt in v3_confirmations:
        estimates = estimates_table(directory, receipt)
        if estimates:
            lines += estimates + [""]
    ladder = ladder_table(rows)
    if ladder:
        lines += ladder + [""]
    lines += library_table(rows) + [""]
    lines += sessions_table(rows) + [""]
    with open(args.output, "w") as handle:
        handle.write("\n".join(lines))
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
