#!/usr/bin/env python3
"""Tabulate host load and per-cell dispersion of the profile-family campaigns (jit:63bad95d).

Reads the plan record, each planned family's ledger and every committed
receipt directory of the family: `receipt.json`, `acceptance-summary.json`,
`launcher.log` and the addendum snapshot under `inputs/`. Writes
`disturbance-record.md` into the issue directory.

The relative half-width of an interval is `max(s - l, u - s) / s`, the
freezer's resolution statistic. A pair's ratio is the baseline's ns/call over
the candidate's; an execution's window spread is its slowest window over its
fastest, at the execution's one calibrated call count. Both are descriptive
quantiles of the raw pairs and carry their sample count.

Usage: make-disturbance-record.py, with no arguments
"""

import json
import statistics

from locate import HERE, ROOT, tracked

ISSUE = HERE.parent
LOAD_LINES = ("# started_utc:", "# load_avg_start:", "# load_avg_end:", "# finished_utc:")


def half_width(interval):
    estimate = interval["estimate"]
    return max(estimate - interval["lower"], interval["upper"] - estimate) / estimate


def quartiles(values):
    low, _, high = statistics.quantiles(values, n=4, method="inclusive")
    return low, statistics.median(values), high


def campaigns(family):
    """The family's committed campaigns in ledger order."""
    ledgers = tracked(f"**/{family['ledger']}")
    if len(ledgers) != 1:
        raise SystemExit(f"{family['id']} has {len(ledgers)} committed ledgers")
    order = [json.loads(line)["campaign"] for line in (ROOT / ledgers[0]).read_text().splitlines()]
    found = {}
    for path in tracked(f"**/{family['id']}/*/acceptance-summary.json"):
        directory = (ROOT / path).parent
        summary = json.loads((ROOT / path).read_text())
        found[summary["campaign_id"]] = {
            "directory": directory,
            "summary": summary,
            "receipt": json.loads((directory / "receipt.json").read_text()),
            "addendum": json.loads((directory / "inputs" / "family-addendum.json").read_text()),
            "launcher": (directory / "launcher.log").read_text().splitlines(),
        }
    if sorted(found) != sorted(order):
        raise SystemExit(f"{family['id']}: ledger campaigns and committed receipts differ")
    return [found[campaign] for campaign in order]


def relative(directory):
    return directory.relative_to(ROOT).as_posix()


def load_section(lines, entries):
    lines += [
        "## Host load each campaign records",
        "",
        "The launcher lines are quoted from each receipt directory's `launcher.log`; the session "
        "rows are `session_hosts[].load_average` of the receipt, which the runner observes when "
        "a session starts.",
        "",
    ]
    for entry in entries:
        lines += [f"`{relative(entry['directory'])}`:", "", "```"]
        lines += [line for line in entry["launcher"] if line.startswith(LOAD_LINES)]
        for host in entry["receipt"]["session_hosts"]:
            load = " ".join(f"{value:.2f}" for value in host["load_average"])
            lines.append(f"session observed_utc {host['observed_utc']} load_average {load}")
        lines += ["```", ""]


def raw_rows(entry):
    """Per cell: pair-ratio quartiles and window-spread median and maximum."""
    rows = {}
    for cell in entry["receipt"]["cells"]:
        ratios, spreads = [], []
        for pair in cell["pairs"]:
            ratios.append(pair["baseline"]["ns_per_call"] / pair["candidate"]["ns_per_call"])
            for side in ("baseline", "candidate"):
                elapsed = [window["elapsed_ns"] for window in pair[side]["windows"]]
                spreads.append(max(elapsed) / min(elapsed))
        low, median, high = quartiles(ratios)
        rows[cell["cell_id"]] = {
            "pairs": len(ratios),
            "ratio_median": median,
            "ratio_iqr": (high - low) / median,
            "executions": len(spreads),
            "spread_median": statistics.median(spreads),
            "spread_max": max(spreads),
        }
    return rows


def family_section(lines, family, entries):
    lines += [
        f"## `{family['id']}`",
        "",
        "Intervals are each campaign's acceptance summary rows at that campaign's corrected alpha.",
        "",
        "| Cell | Campaign | Label | Pairs | Alpha | Estimate | Interval | Relative half-width "
        "| Decision |",
        "|---|---|---|---:|---:|---:|---|---:|---|",
    ]
    cells = [cell["cell_id"] for cell in entries[0]["summary"]["cells"]]
    for cell_id in cells:
        for entry in entries:
            for cell in entry["summary"]["cells"]:
                if cell["cell_id"] != cell_id:
                    continue
                interval = cell["interval"]
                lines.append(
                    f"| `{cell_id}` | `{entry['summary']['campaign_id']}` "
                    f"| {entry['summary']['label']} | {cell['pairs']} | {interval['alpha']:.6f} "
                    f"| {interval['estimate']:.4f} "
                    f"| [{interval['lower']:.4f}, {interval['upper']:.4f}] "
                    f"| {half_width(interval):.4f} | {cell['decision']} |"
                )
    lines += [
        "",
        "Raw pairs of the same receipts, descriptive: the median and the relative interquartile "
        "range of the per-pair ratio over the stated pairs, and the median and the maximum window "
        "spread over the stated executions.",
        "",
        "| Cell | Campaign | Pairs | Ratio median | Ratio relative IQR | Executions "
        "| Window spread median | Window spread maximum |",
        "|---|---|---:|---:|---:|---:|---:|---:|",
    ]
    raw = [raw_rows(entry) for entry in entries]
    for cell_id in cells:
        for entry, rows in zip(entries, raw):
            if cell_id not in rows:
                continue
            row = rows[cell_id]
            lines.append(
                f"| `{cell_id}` | `{entry['summary']['campaign_id']}` | {row['pairs']} "
                f"| {row['ratio_median']:.4f} | {row['ratio_iqr']:.4f} | {row['executions']} "
                f"| {row['spread_median']:.4f} | {row['spread_max']:.4f} |"
            )
    lines.append("")
    for entry, rows in zip(entries, raw):
        if entry["summary"]["label"] == "confirmation":
            confirmation_section(lines, entry, rows, entries, raw)


def confirmation_section(lines, entry, rows, entries, raw):
    """Each confirmatory decision against the frozen resolution and the decision floor."""
    summary = entry["summary"]
    resolution = entry["addendum"]["effect"]["measurement_resolution"]
    evidence = entry["addendum"]["effect"]["resolution_evidence"]["sha256"]
    pilot = next(
        (rows for other, rows in zip(entries, raw)
         if other["summary"]["receipt_sha256"] == evidence),
        None,
    )
    if pilot is None:
        raise SystemExit(f"{summary['campaign_id']}: its resolution evidence is no family receipt")
    lines += [
        f"### Decisions of `{summary['campaign_id']}`",
        "",
        f"Verdict `{summary['verdict']}`, qualifies `{str(summary['qualifies']).lower()}`: a "
        "confirmation qualifies when every confirmatory and holdout cell passes. The frozen "
        f"measurement resolution of its addendum is {resolution}, the widest relative half-width "
        "of the pilot it pins at this confirmation's corrected alpha. A cell is not worse when "
        "its interval's lower bound reaches the floor, the reciprocal of its equivalence margin.",
        "",
        "| Cell | Decision | Outcome | Relative half-width | Exceeds the frozen resolution "
        "| Floor | Lower bound | Estimate reaches the floor | Ratio median reaches the floor "
        "| Ratio relative IQR over the pilot's |",
        "|---|---|---|---:|---|---:|---:|---|---|---:|",
    ]
    widened, shifted, within = [], [], []
    for cell in summary["cells"]:
        interval = cell["interval"]
        floor = 1.0 / cell["margins"]["equivalence"]
        width = half_width(interval)
        exceeds = width > resolution
        reaches = interval["estimate"] >= floor
        row, before = rows[cell["cell_id"]], pilot[cell["cell_id"]]
        lines.append(
            f"| `{cell['cell_id']}` | {cell['decision']} | {cell['outcome']} | {width:.4f} "
            f"| {'yes' if exceeds else 'no'} | {floor:.4f} | {interval['lower']:.4f} "
            f"| {'yes' if reaches else 'no'} "
            f"| {'yes' if row['ratio_median'] >= floor else 'no'} "
            f"| {row['ratio_iqr'] / before['ratio_iqr']:.1f} |"
        )
        if not exceeds:
            within.append(cell)
        elif reaches:
            widened.append(cell)
        else:
            shifted.append(cell)
    lines.append("")

    def names(cells):
        return ", ".join(f"`{cell['cell_id']}` ({cell['outcome']})" for cell in cells)

    if widened:
        lines += [
            f"Interval width alone withholds the decision of {names(widened)}: the estimate "
            "reaches the floor, the lower bound does not, and the half-width exceeds the "
            "resolution the addendum freezes from the pilot.",
            "",
        ]
    if shifted:
        lines += [
            f"The estimate of {names(shifted)} lies below the floor and the half-width exceeds "
            "the frozen resolution: the interval is too wide to decide the cell, and the ratio "
            "median column states whether the middle of its pairs reaches the floor.",
            "",
        ]
    if within:
        lines += [
            f"The half-width of {names(within)} stays within the frozen resolution: the "
            "dispersion the pilot predicts decides these cells.",
            "",
        ]
    failing = [cell for cell in summary["cells"] if cell["outcome"] != "pass"]
    if failing and all(cell in widened + shifted for cell in failing):
        lines += [
            "Every cell that does not pass has a half-width above the frozen resolution, so the "
            "qualification is withheld by cells whose dispersion exceeds what the pilot measures.",
            "",
        ]


def main():
    record = json.loads((HERE / "planned-families.json").read_text())
    families = [(family, campaigns(family)) for family in record["families"]]
    lines = [
        "# Profile-family campaigns: host load and dispersion",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `survey/make-disturbance-record.py` from the committed receipts of the "
        "families `survey/planned-families.json` plans. The relative half-width of an interval "
        "is `max(s - l, u - s) / s`. A pair's ratio is the baseline's ns/call over the "
        "candidate's; an execution's window spread is its slowest timing window over its "
        "fastest at one calibrated call count.",
        "",
    ]
    load_section(lines, [entry for _, entries in families for entry in entries])
    for family, entries in families:
        family_section(lines, family, entries)
    (ISSUE / "disturbance-record.md").write_text("\n".join(lines).rstrip("\n") + "\n")


if __name__ == "__main__":
    main()
