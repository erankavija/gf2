#!/usr/bin/env python3
"""Project this issue's protocol receipts into evidence tables.

Usage: dev/bench_results/eda07788/summarize-v3.py

Reads only committed receipts, acceptance summaries and receipt-local input
snapshots under `dev/bench_results/eda07788/` and writes, beside this script,
`tables-v3.md` for the DVB-T2 bit-interleave family and `tables-nr-derate.md`
for the NR LLR de-rate-matching family. A family receipt directory that does
not exist yet is left out. Every number in the output is recomputed from those
files; nothing is transcribed by hand.

Speedups, intervals, decisions and outcomes come from the acceptance summaries
that `benchmark-acceptance` recomputes from raw pairs. Per-arm medians,
conversion spans and pair counts are descriptive summaries of the per-execution
values in `receipt.json`; they carry no confidence interval and decide nothing.
"""

import json
import math
import os
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
V1_PILOT = "2026-09-08-eda07788-dvb-t2-pilot"
V1_CONFIRMATION = "2026-09-08-eda07788-dvb-t2-confirmation"
V3_PILOT = "2026-09-08-eda07788-dvb-t2-v3-pilot"
V3_PILOT_R2 = "2026-09-08-eda07788-dvb-t2-v3-pilot-r2"
V3_CONFIRMATION = "2026-09-08-eda07788-dvb-t2-v3-confirmation"
NR_PILOT = "2026-09-10-eda07788-nr-derate-pilot"
NR_CONFIRMATION = "2026-09-10-eda07788-nr-derate-confirmation"
TAIL_DRAWS_REQUIRED = 20.0


def load(directory, name):
    with open(os.path.join(HERE, directory, name), encoding="utf-8") as handle:
        return json.load(handle)


def short(cell_id):
    return cell_id.removeprefix("dvb-t2-").removeprefix("nr-")


def relative_half_width(interval):
    estimate = interval["estimate"]
    return max(estimate - interval["lower"], interval["upper"] - estimate) / estimate


def fmt_interval(interval):
    return f"{interval['estimate']:.4f} [{interval['lower']:.4f}, {interval['upper']:.4f}]"


def micro(value_ns):
    return f"{value_ns / 1000.0:.1f}"


def table(lines, header, rows):
    lines.append("| " + " | ".join(header) + " |")
    lines.append("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        lines.append("| " + " | ".join(str(value) for value in row) + " |")
    lines.append("")


def summary_cells(lines, directory):
    summary = load(directory, "acceptance-summary.json")
    family = summary["family"]
    lines.append(
        f"Source: `{directory}/acceptance-summary.json` (receipt "
        f"`{summary['receipt_sha256']}`), label **{summary['label']}**, verdict "
        f"**{summary['verdict']}**, qualifies {str(summary['qualifies']).lower()}, "
        f"{len(summary['findings'])} findings. Family `{family['family_id']}`: "
        f"{family['comparisons']} comparisons, attempt alpha {family['family_alpha']:.6g}, "
        f"per-comparison confidence {family['per_comparison_confidence']:.6f}."
    )
    lines.append("")
    rows = []
    for cell in summary["cells"]:
        interval = cell["interval"]
        # In a comparator-gap cell the external arm is the candidate, so the
        # reciprocal is how many times faster the gf2 baseline arm is.
        reciprocal = (
            f"{1.0 / interval['estimate']:.3f} [{1.0 / interval['upper']:.3f}, "
            f"{1.0 / interval['lower']:.3f}]"
            if "-gap-" in cell["cell_id"] else "-"
        )
        rows.append([
            f"`{short(cell['cell_id'])}`",
            cell["role"],
            cell["pairs"],
            f"{cell['flagged_windows']}/{cell['total_windows']}",
            fmt_interval(interval),
            reciprocal,
            f"{relative_half_width(interval):.4f}",
            cell["decision"],
            cell["outcome"],
        ])
    table(lines, ["Cell", "Role", "Pairs", "Flagged windows", "Speedup [interval]",
                  "gf2 faster by (gap cells)", "Relative half-width", "Decision",
                  "Outcome"], rows)
    return summary


def arm_medians(lines, directory):
    receipt = load(directory, "receipt.json")
    rows = []
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            executions = [pair[side] for pair in cell["pairs"]]
            calls = [execution["ns_per_call"] for execution in executions]
            unpack = [execution["conversion"]["unpack_ns"] for execution in executions]
            pack = [execution["conversion"]["pack_ns"] for execution in executions]
            setup = [execution["conversion"]["setup_ns"] for execution in executions]
            residual = [c - u - p for c, u, p in zip(calls, unpack, pack)]
            rows.append([
                f"`{short(cell['cell_id'])}`",
                side,
                f"`{executions[0]['arm']}`",
                len(executions),
                micro(statistics.median(calls)),
                f"{micro(min(calls))}-{micro(max(calls))}",
                micro(statistics.median(unpack)),
                micro(statistics.median(pack)),
                micro(statistics.median(residual)),
                micro(statistics.median(setup)),
            ])
    table(lines, ["Cell", "Side", "Arm", "Executions", "Median call µs", "Call range µs",
                  "Median unpack µs", "Median pack µs", "Median call - unpack - pack µs",
                  "Median setup µs (once, untimed)"], rows)
    return receipt


def paired_attribution(lines, receipt):
    rows = []
    for cell in receipt["cells"]:
        if "-gap-" not in cell["cell_id"]:
            continue
        exceeds = 0
        ratios = []
        for pair in cell["pairs"]:
            baseline = pair["baseline"]["ns_per_call"]
            candidate = pair["candidate"]["ns_per_call"]
            conversion = (pair["candidate"]["conversion"]["unpack_ns"]
                          + pair["candidate"]["conversion"]["pack_ns"])
            if conversion > candidate - baseline:
                exceeds += 1
            ratios.append((candidate - conversion) / baseline)
        rows.append([
            f"`{short(cell['cell_id'])}`",
            len(cell["pairs"]),
            exceeds,
            f"{statistics.median(ratios):.4f}",
            f"{min(ratios):.4f}-{max(ratios):.4f}",
        ])
    table(lines, ["Cell", "Pairs", "Pairs where external conversion > paired call gap",
                  "Median external (call - unpack - pack) / paired gf2 call", "Range"], rows)


def family_accounting(lines, directory, summary):
    receipt = load(directory, "receipt.json")
    addendum = load(directory, "inputs/family-addendum.json")
    with open(os.path.join(HERE, directory, "inputs/trial-ledger.jsonl"), encoding="utf-8") as handle:
        ledger = [json.loads(line) for line in handle if line.strip()]
    rows = [[entry["sequence"], f"`{entry['campaign']}`", entry["protocol_version"],
             entry["comparisons"], len(entry["candidates"])] for entry in ledger]
    lines.append(
        f"Source: `{directory}/inputs/trial-ledger.jsonl`, the receipt-local ledger prefix "
        f"pinned by `receipt.trial_ledger.sha256` = `{receipt['trial_ledger']['sha256']}`."
    )
    lines.append("")
    table(lines, ["Sequence", "Campaign", "Protocol", "Comparisons", "Candidate identities"], rows)
    alpha = addendum["family_wise"]["alpha"]
    m = max(1, sum(entry["comparisons"] for entry in ledger))
    t = max(1, sum(1 for entry in ledger if entry["comparisons"] > 0))
    attempt_alpha = alpha / (t * (t + 1))
    corrected = attempt_alpha / m
    resamples = summary["family"]["bootstrap_resamples"]
    tail_draws = resamples * corrected / 2.0
    family = summary["family"]
    agrees = (math.isclose(family["family_alpha"], attempt_alpha, rel_tol=1e-12)
              and family["comparisons"] == m
              and math.isclose(family["per_comparison_confidence"], 1.0 - corrected, rel_tol=1e-12))
    table(lines, ["Quantity", "Value", "Derivation"], [
        ["family alpha", f"{alpha}", "frozen addendum `family_wise.alpha`"],
        ["m (reserved comparisons)", m, "sum of ledger `comparisons`"],
        ["t (non-exploratory attempts)", t, "ledger entries with `comparisons` > 0"],
        ["attempt alpha", f"{attempt_alpha:.6f}", "alpha / (t (t + 1))"],
        ["corrected alpha", f"{corrected:.7f}", "attempt alpha / m"],
        ["confidence", f"{1.0 - corrected:.6f}", "1 - corrected alpha"],
        ["bootstrap resamples", resamples, "frozen shared setting"],
        ["expected draws per tail", f"{tail_draws:.2f}", "resamples x corrected alpha / 2"],
        ["required draws per tail", f"{TAIL_DRAWS_REQUIRED:.0f}", "protocol P-20"],
        ["tail condition", "fails" if tail_draws < TAIL_DRAWS_REQUIRED else "holds", ""],
        ["summary agrees", "yes" if agrees else "NO", "`family` block of the acceptance summary"],
    ])


def resolution(lines, pilot_directory, confirmation_directory, confirmation_summary):
    addendum = load(confirmation_directory, "inputs/family-addendum.json")
    effect = addendum["effect"]
    pilot = load(pilot_directory, "acceptance-summary.json")
    widest_pilot = max(
        (relative_half_width(cell["interval"]), cell["cell_id"]) for cell in pilot["cells"]
    )
    widest_confirmation = max(
        (relative_half_width(cell["interval"]), cell["cell_id"])
        for cell in confirmation_summary["cells"]
    )
    table(lines, ["Quantity", "Value", "Source"], [
        ["declared measurement resolution", effect["measurement_resolution"],
         f"`{confirmation_directory}/inputs/family-addendum.json`"],
        ["resolution evidence", f"`{effect['resolution_evidence']['sha256']}`",
         f"`{effect['resolution_evidence']['receipt']}`"],
        ["pilot widest relative half-width", f"{widest_pilot[0]:.6f} (`{short(widest_pilot[1])}`)",
         f"`{pilot_directory}/acceptance-summary.json`"],
        ["confirmation widest relative half-width",
         f"{widest_confirmation[0]:.6f} (`{short(widest_confirmation[1])}`)",
         f"`{confirmation_directory}/acceptance-summary.json`"],
    ])


def sessions(lines, directory):
    receipt = load(directory, "receipt.json")
    lines.append(
        f"Source: `{directory}/receipt.json` `session_hosts`, one runtime host observation per "
        f"bounded session; toolchain `{receipt['toolchain']}`."
    )
    lines.append("")
    rows = []
    for index, host in enumerate(receipt["session_hosts"], 1):
        rows.append([
            index,
            host["observed_utc"][:19] + "Z",
            host["hostname"],
            host["cpu_model"],
            host["os_kernel"],
            ", ".join(sorted(set(host["governors"].values()))),
            "active" if host["smt_active"] else "inactive",
            len(host["affinity"]),
            " / ".join(f"{value:.2f}" for value in host["load_average"]),
        ])
    table(lines, ["Session", "Observed", "Host", "CPU", "Kernel", "Governors", "SMT",
                  "CPUs in mask", "Load average 1/5/15 min"], rows)


def executables(lines, directories):
    rows = []
    for directory in directories:
        receipt = load(directory, "receipt.json")
        for name, arm in sorted(receipt["arms"].items()):
            rows.append([f"`{directory}`", f"`{name}`", arm["build"], f"`{arm['executable_sha256']}`"])
    table(lines, ["Receipt", "Arm", "Build", "Executable SHA-256"], rows)


PREAMBLE = [
    "> **Diátaxis Type:** Reference",
    "",
    "Generated by `dev/bench_results/eda07788/summarize-v3.py` from the committed receipts,",
    "acceptance summaries and receipt-local snapshots named in each table. Speedup is",
    "`median(baseline) / median(candidate)`; below 1 in a `-gap-` cell means gf2 is faster.",
    "Relative half-width is `max(estimate - lower, upper - estimate) / estimate`.",
    "Per-arm medians and conversion spans are descriptive: they carry no interval and",
    "decide nothing. `setup` is one untimed construction.",
]


def write(path, lines):
    with open(path, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines).rstrip("\n") + "\n")
    print(path)


def dvb_tables():
    lines = ["# DVB-T2 bit-interleave evidence tables", ""] + PREAMBLE + [
        "`unpack` and `pack` are the external arm's mean per-call conversion time inside",
        "the measured windows.",
        "",
        "## Protocol-v3 confirmation",
        "",
    ]
    summary = summary_cells(lines, V3_CONFIRMATION)
    lines += ["### Family accounting (P-20)", ""]
    family_accounting(lines, V3_CONFIRMATION, summary)
    lines += ["### Measurement resolution", ""]
    resolution(lines, V3_PILOT_R2, V3_CONFIRMATION, summary)
    lines += ["### Per-arm call time and conversion spans", ""]
    receipt = arm_medians(lines, V3_CONFIRMATION)
    lines += ["### Paired conversion attribution", ""]
    paired_attribution(lines, receipt)
    lines += ["### Sessions and host", ""]
    sessions(lines, V3_CONFIRMATION)
    lines += ["## Protocol-v3 exploratory pilots", ""]
    summary_cells(lines, V3_PILOT)
    arm_medians(lines, V3_PILOT)
    summary_cells(lines, V3_PILOT_R2)
    lines += ["## Protocol-v1 history (immutable, superseded)", ""]
    summary_cells(lines, V1_CONFIRMATION)
    arm_medians(lines, V1_CONFIRMATION)
    summary_cells(lines, V1_PILOT)
    lines += ["## Arm executables", ""]
    executables(lines, (V1_PILOT, V1_CONFIRMATION, V3_PILOT, V3_PILOT_R2, V3_CONFIRMATION))
    write(os.path.join(HERE, "tables-v3.md"), lines)


def nr_tables():
    present = [d for d in (NR_PILOT, NR_CONFIRMATION) if os.path.isdir(os.path.join(HERE, d))]
    if not present:
        return
    lines = ["# NR LLR de-rate-matching evidence tables", ""] + PREAMBLE + [
        "`unpack` and `pack` are the AFF3CT adapter's input and output stages, each timed",
        "alone after the measured windows; they are not additive parts of the timed call.",
        "",
    ]
    if NR_CONFIRMATION in present:
        lines += ["## Protocol-v3 confirmation", ""]
        summary = summary_cells(lines, NR_CONFIRMATION)
        lines += ["### Family accounting (P-20)", ""]
        family_accounting(lines, NR_CONFIRMATION, summary)
        lines += ["### Measurement resolution", ""]
        resolution(lines, NR_PILOT, NR_CONFIRMATION, summary)
        lines += ["### Per-arm call time and adapter stages", ""]
        receipt = arm_medians(lines, NR_CONFIRMATION)
        lines += ["### Paired adapter attribution", ""]
        paired_attribution(lines, receipt)
        lines += ["### Sessions and host", ""]
        sessions(lines, NR_CONFIRMATION)
    lines += ["## Protocol-v3 exploratory pilot", ""]
    summary_cells(lines, NR_PILOT)
    arm_medians(lines, NR_PILOT)
    if NR_CONFIRMATION not in present:
        lines += ["### Sessions and host", ""]
        sessions(lines, NR_PILOT)
    lines += ["## Arm executables", ""]
    executables(lines, present)
    write(os.path.join(HERE, "tables-nr-derate.md"), lines)


def main():
    dvb_tables()
    nr_tables()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
