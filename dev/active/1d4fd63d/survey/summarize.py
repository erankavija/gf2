#!/usr/bin/env python3
"""Project the committed transpose-lane receipts into one generated table file.

Usage: summarize.py [output]   (default dev/bench_results/1d4fd63d/tables.md)

Every figure in `dev/active/1d4fd63d/findings.md` lives here or in the receipts
this script reads; the report carries the argument and points at a section,
block and row. The script reads only committed bytes — each receipt's
`receipt.json` and `acceptance-summary.json`, and each addendum snapshot the
receipt pins — derives every number it prints from them, and writes nothing
else. Running it twice over unchanged receipts reproduces the file byte for
byte.

Interval methods, stated once here and named in each block:

* **Speedup of medians** is the acceptance tool's percentile bootstrap of
  `median(baseline) / median(candidate)` over the cell's paired executions, at
  the per-comparison alpha the family ledger derives. Values above one favour
  the candidate arm. The tool recomputes it from the raw pairs; this script
  copies its recomputation and never re-estimates.
* **Per-arm median** is the median of that arm's per-execution nanoseconds per
  call, with the distribution-free order-statistic interval at the same alpha:
  the widest pair of order statistics whose binomial tail mass is at most
  alpha/2 on each side. It is descriptive of one arm and makes no comparison.
* **Probes** are the conversion fields each child reports. Each is a mean over
  the repetitions that child ran outside its timed windows, and the value shown
  is the median over the cell's executions of that arm. A probe is not a share
  of the timed call unless a row says what it is a share of.
"""

import hashlib
import json
import math
import pathlib
import subprocess
import sys

# (receipt directory, section title, family). The lane-selection family
# compares gf2 lanes against each other; the comparator family compares them
# against M4RI and Bitshuffle. Each has its own ledger and its own resolution.
RECEIPT_ORDER = [
    ("2026-09-13-1d4fd63d-transpose-lane-smoke", "Smoke", "selection"),
    ("2026-09-13-1d4fd63d-transpose-lane-pilot", "Lane ranking stage", "selection"),
    ("2026-09-13-1d4fd63d-transpose-lane-selected", "Selected-lane stage", "selection"),
    ("2026-09-13-1d4fd63d-transpose-lane-confirmation", "Lane confirmation", "selection"),
    ("2026-09-13-1d4fd63d-external-pilot", "Comparator pilot", "external"),
    ("2026-09-13-1d4fd63d-external-confirmation", "Comparator confirmation", "external"),
]


def repo_root():
    return pathlib.Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )


def median(values):
    ordered = sorted(values)
    count = len(ordered)
    if count == 0:
        return None
    middle = count // 2
    if count % 2:
        return ordered[middle]
    return 0.5 * (ordered[middle - 1] + ordered[middle])


def order_statistic_interval(values, alpha):
    """The widest distribution-free interval for the median at `alpha`.

    Returns `(low, high)` of the sample, or `None` when no pair of order
    statistics carries at most `alpha / 2` of the binomial tail on each side,
    which is what a sample too small for the confidence means.
    """
    ordered = sorted(values)
    count = len(ordered)
    if count == 0:
        return None
    tail = alpha / 2.0
    mass = 0.0
    rank = 0
    while rank < count:
        term = math.comb(count, rank) * 0.5**count
        if mass + term > tail:
            break
        mass += term
        rank += 1
    if rank < 1 or rank > count - rank:
        return None
    return ordered[rank - 1], ordered[count - rank]


def fmt(value, digits=4):
    return "—" if value is None else f"{value:.{digits}g}"


def interval_cell(pair, digits=4):
    if pair is None:
        return "—"
    return f"[{fmt(pair[0], digits)}, {fmt(pair[1], digits)}]"


def read_receipt(root, name):
    directory = root / "dev/bench_results/1d4fd63d" / name
    receipt_bytes = (directory / "receipt.json").read_bytes()
    receipt = json.loads(receipt_bytes)
    summary = json.loads((directory / "acceptance-summary.json").read_text())
    addendum = json.loads(
        (directory / receipt["addendum"]["snapshot"]).read_text()
    )
    return {
        "name": name,
        "directory": directory,
        "receipt": receipt,
        "receipt_sha256": hashlib.sha256(receipt_bytes).hexdigest(),
        "summary": summary,
        "addendum": addendum,
    }


def heading_block(entry, title, lines):
    receipt, summary, addendum = entry["receipt"], entry["summary"], entry["addendum"]
    family = summary["family"]
    effect = addendum["effect"]
    host = receipt["host"]
    lines.append(f"## {title} — `{entry['name']}`")
    lines.append("")
    lines.append("| Field | Value |")
    lines.append("|---|---|")
    lines.append(f"| Campaign | `{summary['campaign_id']}` |")
    lines.append(f"| Label | {summary['label']} |")
    lines.append(f"| Receipt SHA-256 | `{entry['receipt_sha256']}` |")
    lines.append(f"| Verdict | {summary['verdict']} |")
    lines.append(f"| Findings | {len(summary['findings'])} |")
    lines.append(f"| Qualifies for production selection | {str(summary['qualifies']).lower()} |")
    lines.append(f"| Sessions (resumed) | {summary['sessions']} ({str(summary['resumed']).lower()}) |")
    lines.append(f"| Family | `{family['family_id']}` |")
    lines.append(f"| Ledger comparisons (m) | {family['comparisons']} |")
    lines.append(f"| Attempt alpha | {family['family_alpha']:.10g} |")
    lines.append(f"| Per-comparison confidence | {family['per_comparison_confidence']:.10g} |")
    lines.append(f"| Bootstrap resamples | {family['bootstrap_resamples']} |")
    lines.append(f"| Addendum SHA-256 | `{receipt['addendum']['sha256']}` |")
    lines.append(f"| Measurement resolution | {fmt(effect['measurement_resolution'], 3)} |")
    evidence = effect["resolution_evidence"]
    lines.append(
        "| Resolution evidence | "
        + (f"`{evidence['receipt']}` SHA-256 `{evidence['sha256']}` |" if evidence else "— |")
    )
    lines.append(f"| Worthwhile speedup | {fmt(effect['worthwhile_speedup'], 3)} |")
    lines.append(f"| Equivalence margin | {fmt(effect['equivalence_margin'], 3)} |")
    lines.append(f"| Toolchain | {receipt['toolchain']} |")
    lines.append(f"| Host | {host['hostname']}, {host['cpu_model']} |")
    lines.append(f"| Kernel | {host['os_kernel']} |")
    lines.append(f"| SMT active | {str(host['smt_active']).lower()} |")
    governors = sorted(set(host["governors"].values()))
    lines.append(
        f"| Governors (distinct over {len(host['governors'])} CPUs) | "
        + ", ".join(governors)
        + " |"
    )
    lines.append("")
    lines.append("Arm executables:")
    lines.append("")
    lines.append("| Arm | Build | Lane selected by the environment | Executable SHA-256 |")
    lines.append("|---|---|---|---|")
    for name in sorted(receipt["arms"]):
        arm = receipt["arms"][name]
        lane = arm["environment"].get("GF2_TRANSPOSE_LANE", "—")
        lines.append(
            f"| `{name}` | {arm['build']} | `{lane}` | `{arm['executable_sha256']}` |"
        )
    lines.append("")


def cells_block(entry, lines):
    summary = entry["summary"]
    addendum_cells = {c["cell_id"]: c for c in entry["addendum"]["cells"]}
    lines.append("### Cells")
    lines.append("")
    lines.append(
        "| Cell | Role | Objective | Pairs | Flagged / windows | Speedup of medians | "
        "Interval | Decision | Outcome |"
    )
    lines.append("|---|---|---|---:|---:|---:|---|---|---|")
    for cell in summary["cells"]:
        interval = cell["interval"]
        lines.append(
            f"| `{cell['cell_id']}` | {cell['role']} | {cell['objective']} | {cell['pairs']} | "
            f"{cell['flagged_windows']} / {cell['total_windows']} | {fmt(interval['estimate'])} | "
            f"{interval_cell((interval['lower'], interval['upper']))} | {cell['decision']} | "
            f"{cell['outcome']} |"
        )
    lines.append("")
    lines.append("### Arms, routes and per-arm medians")
    lines.append("")
    lines.append(
        "| Cell | Baseline arm | Candidate arm | Baseline ns/call | Baseline interval | "
        "Candidate ns/call | Candidate interval | Observed routes | Seed | CPUs |"
    )
    lines.append("|---|---|---|---:|---|---:|---|---|---:|---|")
    for cell in entry["receipt"]["cells"]:
        claimed = cell.get("claimed") or {}
        alpha = (claimed.get("interval") or {}).get("alpha")
        declared = addendum_cells.get(cell["cell_id"], {})
        seed = declared.get("workload", {}).get("seed", "—")
        values = {"baseline": [], "candidate": []}
        routes = set()
        for pair in cell["pairs"]:
            for side in ("baseline", "candidate"):
                values[side].append(pair[side]["ns_per_call"])
                if pair[side].get("selected_path"):
                    routes.add(pair[side]["selected_path"])
        lines.append(
            f"| `{cell['cell_id']}` | `{cell['baseline_arm']}` | `{cell['candidate_arm']}` | "
            f"{fmt(median(values['baseline']), 6)} | "
            f"{interval_cell(order_statistic_interval(values['baseline'], alpha) if alpha else None, 6)} | "
            f"{fmt(median(values['candidate']), 6)} | "
            f"{interval_cell(order_statistic_interval(values['candidate'], alpha) if alpha else None, 6)} | "
            f"{', '.join(sorted(routes)) or '—'} | {seed} | "
            f"{','.join(str(cpu) for cpu in cell['resolved_cpus'])} |"
        )
    lines.append("")


def probes_block(entry, lines):
    lines.append("### Probes")
    lines.append("")
    lines.append(
        "Each value is the median over the cell's executions of that arm of a "
        "mean the child measured outside its timed windows."
    )
    lines.append("")
    lines.append(
        "| Cell | Arm | setup ns | pack ns | unpack ns | batch fill ns | dispatch ns |"
    )
    lines.append("|---|---|---:|---:|---:|---:|---:|")
    fields = ("setup_ns", "pack_ns", "unpack_ns", "batch_fill_ns", "dispatch_ns")
    for cell in entry["receipt"]["cells"]:
        for side in ("baseline", "candidate"):
            arm = cell[f"{side}_arm"]
            gathered = {field: [] for field in fields}
            for pair in cell["pairs"]:
                conversion = pair[side].get("conversion")
                if not conversion:
                    continue
                for field in fields:
                    gathered[field].append(conversion[field])
            if not any(gathered.values()):
                continue
            row = " | ".join(
                fmt(median(gathered[field]), 6) if gathered[field] else "—"
                for field in fields
            )
            lines.append(f"| `{cell['cell_id']}` | `{arm}` | {row} |")
    lines.append("")


def resolution_block(entries, lines, title, pilot_suffix, confirmation_suffix):
    lines.append(f"## {title}")
    lines.append("")
    lines.append(
        "The frozen measurement resolution is the widest relative bootstrap "
        "half-width over every cell of the pilot the confirmation pins, rounded up "
        "to two decimals. Each row is recomputed here from that pilot's own "
        "acceptance summary."
    )
    lines.append("")
    pilot = next(entry for entry in entries if entry["name"].endswith(pilot_suffix))
    confirmation = next(
        entry for entry in entries if entry["name"].endswith(confirmation_suffix)
    )
    lines.append("| Cell | Estimate | Lower | Upper | Relative half-width |")
    lines.append("|---|---:|---:|---:|---:|")
    widest = 0.0
    for cell in pilot["summary"]["cells"]:
        interval = cell["interval"]
        estimate = interval["estimate"]
        half = (
            max(abs(estimate - interval["lower"]), abs(interval["upper"] - estimate))
            / estimate
        )
        widest = max(widest, half)
        lines.append(
            f"| `{cell['cell_id']}` | {fmt(estimate)} | {fmt(interval['lower'])} | "
            f"{fmt(interval['upper'])} | {half:.4f} |"
        )
    lines.append("")
    lines.append(f"Widest relative half-width: {widest:.6f}.")
    lines.append("")
    effect = confirmation["addendum"]["effect"]
    declared = [
        (label, effect[field])
        for label, field in (
            ("worthwhile speedup", "worthwhile_speedup"),
            ("equivalence margin", "equivalence_margin"),
            ("material-gap threshold", "material_gap_threshold"),
        )
        if effect[field] is not None
    ]
    margins = ", ".join(f"{label} {fmt(value, 3)}" for label, value in declared)
    lines.append(
        f"Frozen resolution: {fmt(effect['measurement_resolution'], 3)}. The margins "
        f"this addendum declares are {margins}; each is strictly above one plus that "
        "resolution, which is what P-03 requires of a margin the family can resolve."
    )
    lines.append("")


def tail_support_block(entries, lines, title, confirmation_suffix):
    confirmation = next(
        entry for entry in entries if entry["name"].endswith(confirmation_suffix)
    )
    family = confirmation["summary"]["family"]
    resamples = family["bootstrap_resamples"]
    alpha = family["family_alpha"] / family["comparisons"]
    lines.append(f"## {title}")
    lines.append("")
    lines.append(
        "P-20 accepts a confirmatory cell only where each bootstrap tail holds at "
        "least twenty expected draws at the corrected per-comparison rate. The rows "
        "below recompute that rate from the ledger the confirmation receipt pins and "
        "project the next attempt's rate from the same chain, with the attempt "
        "budget alpha / [t(t+1)] the protocol allocates."
    )
    lines.append("")
    lines.append(
        "| Attempt t | Reserved comparisons m | Attempt alpha | Per-comparison alpha | "
        "Expected draws per tail |"
    )
    lines.append("|---:|---:|---:|---:|---:|")
    lines.append(
        f"| 1 | {family['comparisons']} | {family['family_alpha']:.10g} | {alpha:.10g} | "
        f"{resamples * alpha / 2:.2f} |"
    )
    ledger_alpha = 0.05
    for extra in (1, 6):
        attempt = 2
        attempt_alpha = ledger_alpha / (attempt * (attempt + 1))
        comparisons = family["comparisons"] + extra
        per_comparison = attempt_alpha / comparisons
        lines.append(
            f"| 2 | {comparisons} | {attempt_alpha:.10g} | {per_comparison:.10g} | "
            f"{resamples * per_comparison / 2:.2f} |"
        )
    lines.append("")


# The lane width of the bit-sliced BCH encoding family, a structural constant
# of the library rather than a measured figure: `BITSLICE_LANES` in
# `crates/gf2-kernels-simd/src/bch_encode.rs`. A batch of B messages reduces in
# ceil(B / BITSLICE_LANES) lane groups, and one group absorbs its whole message
# once and unpacks its parity once, which is what the absorb and unpack cells
# each time.
BITSLICE_LANES = 64


def conversion_share_block(entries, lines):
    """What the bit-slice conversion costs inside the whole BCH consumer.

    Every median below comes from one stage, so both arms of every row ran in
    the same campaign under the same host observation. The share is arithmetic
    on those medians and on the declared batch size; it carries no interval of
    its own and is descriptive.
    """
    stage = next(
        entry for entry in entries if entry["name"].endswith("transpose-lane-selected")
    )
    declared = {c["cell_id"]: c for c in stage["addendum"]["cells"]}
    medians = {}
    for cell in stage["receipt"]["cells"]:
        for side in ("baseline", "candidate"):
            medians[(cell["cell_id"], cell[f"{side}_arm"])] = median(
                [pair[side]["ns_per_call"] for pair in cell["pairs"]]
            )
    control_id = next(key for key in declared if key.endswith("-control-1core"))
    batch = declared[control_id]["workload"]["size"]["batch"]
    groups = -(-batch // BITSLICE_LANES)

    lines.append("## Conversion share of the whole BCH consumer")
    lines.append("")
    lines.append(
        "The identity control times one whole bit-sliced BCH batch encode of "
        f"{batch} messages, which is {groups} lane groups of {BITSLICE_LANES} frames "
        "(`BITSLICE_LANES` in `crates/gf2-kernels-simd/src/bch_encode.rs`). Each "
        "group absorbs its whole message once and unpacks its parity once, which is "
        "what the absorb and unpack cells each time. The rows are medians of the "
        "selected-lane stage, so both arms of every row ran in one campaign under "
        "one host observation; the share is arithmetic on them and carries no "
        "interval of its own."
    )
    lines.append("")
    lines.append("| Quantity | Production lane | Candidate lane |")
    lines.append("|---|---:|---:|")
    rows = []
    for label, suffix in (
        ("One lane group's absorb (ns)", "-bitslice-absorb-m14-1core"),
        ("One lane group's parity unpack (ns)", "-bitslice-unpack-m14-1core"),
    ):
        cell_id = next(key for key in declared if key.endswith(suffix))
        cell = next(c for c in stage["receipt"]["cells"] if c["cell_id"] == cell_id)
        baseline = medians[(cell_id, cell["baseline_arm"])]
        candidate = medians[(cell_id, cell["candidate_arm"])]
        rows.append((baseline, candidate))
        lines.append(f"| {label} | {fmt(baseline, 6)} | {fmt(candidate, 6)} |")
    conversion = [groups * (rows[0][side] + rows[1][side]) for side in (0, 1)]
    control_cell = next(
        c for c in stage["receipt"]["cells"] if c["cell_id"] == control_id
    )
    control = medians[(control_id, control_cell["baseline_arm"])]
    lines.append(
        f"| Conversion over {groups} lane groups (ns) | {fmt(conversion[0], 6)} | "
        f"{fmt(conversion[1], 6)} |"
    )
    lines.append(
        f"| Whole batch encode, both arms production (ns) | {fmt(control, 6)} | "
        f"{fmt(control, 6)} |"
    )
    lines.append(
        f"| Conversion as a share of the whole encode | {conversion[0] / control:.4f} | "
        f"{conversion[1] / control:.4f} |"
    )
    lines.append("")
    saved = conversion[0] - conversion[1]
    lines.append(
        f"Nanoseconds the candidate lane removes from one batch's conversion: "
        f"{saved:.6g}, which is {saved / control:.4f} of the whole encode. The "
        "reciprocal of one minus the conversion share is an Amdahl ceiling on what "
        "any faster conversion can give this consumer, and is an estimate: "
        f"{1.0 / (1.0 - conversion[0] / control):.4f}."
    )
    lines.append("")


def main():
    root = repo_root()
    output = pathlib.Path(
        sys.argv[1] if len(sys.argv) > 1 else root / "dev/bench_results/1d4fd63d/tables.md"
    )
    entries = [read_receipt(root, name) for name, _, _ in RECEIPT_ORDER]

    lines = [
        "# Transpose-lane receipt tables (jit:1d4fd63d)",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `dev/active/1d4fd63d/survey/summarize.py` from the committed "
        "receipts alone. Every figure the issue's report cites lives here, in a "
        "receipt, or in the frozen derivation record beside the confirmation "
        "addendum. The generator's module docstring states each interval method; "
        "the short form is repeated with each block.",
        "",
    ]
    resolution_block(
        entries,
        lines,
        "Resolution of the lane confirmation",
        "transpose-lane-selected",
        "transpose-lane-confirmation",
    )
    tail_support_block(
        entries,
        lines,
        "Tail support and remaining budget of the lane-selection family",
        "transpose-lane-confirmation",
    )
    resolution_block(
        entries,
        lines,
        "Resolution of the comparator confirmation",
        "1d4fd63d-external-pilot",
        "1d4fd63d-external-confirmation",
    )
    tail_support_block(
        entries,
        lines,
        "Tail support and remaining budget of the comparator family",
        "1d4fd63d-external-confirmation",
    )
    conversion_share_block(entries, lines)
    for entry, (_, title, _family) in zip(entries, RECEIPT_ORDER):
        heading_block(entry, title, lines)
        cells_block(entry, lines)
        probes_block(entry, lines)
    output.write_text("\n".join(lines) + "\n")
    print(f"{len(entries)} receipts -> {output}", file=sys.stderr)


if __name__ == "__main__":
    main()
