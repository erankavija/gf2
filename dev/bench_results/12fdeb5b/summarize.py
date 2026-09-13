#!/usr/bin/env python3
"""Generate the NR rate-matched encoder evidence tables (jit:12fdeb5b).

Usage:
  summarize.py <output.md> [receipt-dir ...]

Every figure is read from a committed artifact: the configuration grid and the
derived code parameters from the survey's parameter dump, the equivalence
outcomes from its validation record, and the measured cells from each receipt
directory's acceptance summary, receipt and receipt-local ledger snapshot. The
generator computes only presentation quantities (relative half-width, the
reciprocal of a speedup, medians over the receipt's own per-execution values)
and states the derivation of each.
"""

import json
import pathlib
import statistics
import sys

SURVEY = pathlib.Path("dev/active/12fdeb5b/survey")


def load(path):
    return json.loads(pathlib.Path(path).read_text())


def relative_half_width(interval):
    estimate = interval["estimate"]
    return max(estimate - interval["lower"], interval["upper"] - estimate) / estimate


def fmt(value, places=4):
    return f"{value:.{places}f}"


def configuration_rows(out):
    parameters = load(SURVEY / "nr-encode-parameters.json")
    out.append("## Configuration grid and derived code parameters\n")
    out.append(
        "Source: `dev/active/12fdeb5b/survey/nr-encode-parameters.json`, the dump "
        "`derive-nr-encode-parameters` produced from each project's own derivation. "
        "`Kb for Z` is the lifting selection constant TS 38.212 Section 5.2.2 makes "
        "depend on the message length. A row is a descriptive parameter listing and "
        "carries no measurement.\n"
    )
    out.append("| Configuration | RV | BG | Z | K | N | K_LDPC | mother N | fillers | Kb for Z | rate |")
    out.append("|---|---|---|---|---|---|---|---|---|---|---|")
    for row in parameters["rows"]:
        gf2 = row["gf2"]
        out.append(
            f"| `{row['configuration']}` | {row['redundancy_version']} | "
            f"{gf2['base_graph']} | {gf2['lifting_factor']} | {gf2['target_k']} | "
            f"{gf2['target_n']} | {gf2['full_k']} | {gf2['full_n']} | "
            f"{gf2['num_shortened']} | {gf2['kb_for_z_selection']} | "
            f"{gf2['effective_rate']:.3f} |"
        )
    out.append("")


def validation_rows(out):
    record = load(SURVEY / "nr-encode-validation.json")
    out.append("## Bit-exact equivalence outcomes\n")
    out.append(
        f"Source: `dev/active/12fdeb5b/survey/nr-encode-validation.json`. Each arm "
        f"encodes the same {record['messages_per_configuration']} messages as gf2 — "
        f"the all-zero word, the all-one word and "
        f"{record['messages_per_configuration'] - 2} words from seed "
        f"{record['message_seed']} — and the record counts the messages whose "
        f"rate-matched codeword is bit-identical. `min positions` is the smallest "
        f"number of agreeing bit positions over those messages; it equals the "
        f"codeword length exactly when every message agrees.\n"
    )
    out.append("| Configuration | Arm | Status | Identical | min positions / N | Differing parameter |")
    out.append("|---|---|---|---|---|---|")
    for row in record["rows"]:
        for arm in row["arms"]:
            differing = arm.get("differing_parameter") or "-"
            positions = (
                f"{arm['min_identical_positions']} / {arm['codeword_bits']}"
                if arm["messages_compared"]
                else "-"
            )
            out.append(
                f"| `{row['configuration']}` | {arm['arm']} | {arm['status']} | "
                f"{arm['messages_identical']}/{arm['messages_compared']} | {positions} | "
                f"{differing} |"
            )
    out.append("")
    matched = sum(
        1 for row in record["rows"] for arm in row["arms"] if arm["status"] == "matched"
    )
    non_equivalent = sum(
        1 for row in record["rows"] for arm in row["arms"] if arm["status"] == "non-equivalent"
    )
    unavailable = sum(
        1 for row in record["rows"] for arm in row["arms"] if arm["status"] == "unavailable"
    )
    out.append("| Outcome | Arms |")
    out.append("|---|---|")
    out.append(f"| matched | {matched} |")
    out.append(f"| non-equivalent | {non_equivalent} |")
    out.append(f"| unavailable | {unavailable} |")
    out.append("")


def ledger_rows(out, directory, receipt):
    snapshot = directory / "inputs" / "trial-ledger.jsonl"
    if not snapshot.exists():
        return
    entries = [json.loads(line) for line in snapshot.read_text().splitlines() if line.strip()]
    out.append("### Family accounting (P-20)\n")
    out.append(
        f"Source: `{snapshot.relative_to(directory.parent.parent.parent)}`, the "
        f"receipt-local ledger prefix pinned by `receipt.trial_ledger.sha256` = "
        f"`{receipt['trial_ledger']['sha256']}`.\n"
    )
    out.append("| Sequence | Campaign | Protocol | Comparisons | Candidate identities |")
    out.append("|---|---|---|---|---|")
    for entry in entries:
        out.append(
            f"| {entry['sequence']} | `{entry['campaign']}` | {entry['protocol_version']} | "
            f"{entry['comparisons']} | {len(entry['candidates'])} |"
        )
    out.append("")


def arm_rows(out, receipt):
    out.append("### Per-arm call time and adapter stages\n")
    out.append(
        "Median over the cell's pairs of each arm's per-execution nanoseconds per "
        "call, and of the conversion costs the arm reported. These are descriptive: "
        "they carry no interval and decide nothing. `setup` is one untimed "
        "construction. `unpack` and `pack` are the external adapter's input and "
        "output stages, each timed alone after the measured windows; they are not "
        "additive parts of the timed call.\n"
    )
    out.append("| Cell | Arm | Pairs | Median ns/call | setup (ms) | unpack (ns) | pack (ns) |")
    out.append("|---|---|---|---|---|---|---|")
    for cell in receipt["cells"]:
        pairs = cell.get("pairs") or []
        if not isinstance(pairs, list) or not pairs:
            continue
        for side in ("baseline", "candidate"):
            values = [pair[side]["ns_per_call"] for pair in pairs]
            setups = [pair[side]["conversion"]["setup_ns"] for pair in pairs]
            unpacks = [pair[side]["conversion"]["unpack_ns"] for pair in pairs]
            packs = [pair[side]["conversion"]["pack_ns"] for pair in pairs]
            out.append(
                f"| `{cell['cell_id']}` | {pairs[0][side]['arm']} | {len(pairs)} | "
                f"{statistics.median(values):.1f} | "
                f"{statistics.median(setups) / 1e6:.3f} | "
                f"{statistics.median(unpacks):.0f} | {statistics.median(packs):.0f} |"
            )
    out.append("")


def receipt_section(out, directory):
    directory = pathlib.Path(directory)
    summary = load(directory / "acceptance-summary.json")
    receipt = load(directory / "receipt.json")
    family = summary["family"]
    out.append(f"## {summary['label'].capitalize()} campaign `{summary['campaign_id']}`\n")
    out.append(
        f"Source: `{directory.name}/acceptance-summary.json` (receipt "
        f"`{summary['receipt_sha256']}`), label **{summary['label']}**, verdict "
        f"**{summary['verdict']}**, qualifies {str(summary['qualifies']).lower()}, "
        f"{len(summary['findings'])} findings, {summary['sessions']} sessions. "
        f"Family `{family['family_id']}`: {family['comparisons']} comparisons, "
        f"attempt alpha {family['family_alpha']}, per-comparison confidence "
        f"{family['per_comparison_confidence']}.\n"
    )
    if summary["findings"]:
        out.append("| Finding | Severity | Detail |")
        out.append("|---|---|---|")
        for finding in summary["findings"]:
            out.append(
                f"| {finding.get('rule', '-')} | {finding.get('severity', '-')} | "
                f"{finding.get('message', '-')} |"
            )
        out.append("")
    out.append(
        "| Cell | Role | Pairs | Flagged windows | Speedup [interval] | gf2 faster by "
        "(gap cells) | Relative half-width | Decision | Outcome |"
    )
    out.append("|---|---|---|---|---|---|---|---|---|")
    for cell in summary["cells"]:
        interval = cell.get("interval")
        if interval is None:
            out.append(
                f"| `{cell['cell_id']}` | {cell['role']} | {cell['pairs']} | - | - | - | - | "
                f"- | {cell['outcome']} |"
            )
            continue
        speedup = (
            f"{fmt(interval['estimate'])} [{fmt(interval['lower'])}, "
            f"{fmt(interval['upper'])}]"
        )
        if "-gap-" in cell["cell_id"] and interval["estimate"] < 1:
            inverse = (
                f"{fmt(1 / interval['estimate'], 3)} [{fmt(1 / interval['upper'], 3)}, "
                f"{fmt(1 / interval['lower'], 3)}]"
            )
        else:
            inverse = "-"
        out.append(
            f"| `{cell['cell_id']}` | {cell['role']} | {cell['pairs']} | "
            f"{cell['flagged_windows']}/{cell['total_windows']} | {speedup} | {inverse} | "
            f"{fmt(relative_half_width(interval))} | {cell['decision']} | {cell['outcome']} |"
        )
    out.append("")
    measured = [cell for cell in summary["cells"] if cell.get("interval")]
    if measured:
        widest = max(measured, key=lambda cell: relative_half_width(cell["interval"]))
        out.append("| Quantity | Value | Source |")
        out.append("|---|---|---|")
        out.append(
            f"| widest relative half-width | {fmt(relative_half_width(widest['interval']), 6)} "
            f"(`{widest['cell_id']}`) | `{directory.name}/acceptance-summary.json` |"
        )
        addendum = load(directory / receipt["addendum"]["snapshot"])
        resolution = addendum["effect"]["measurement_resolution"]
        evidence = addendum["effect"]["resolution_evidence"]
        out.append(
            f"| declared measurement resolution | "
            f"{'none declared' if resolution is None else resolution} | "
            f"`{directory.name}/{receipt['addendum']['snapshot']}` |"
        )
        out.append(
            f"| resolution evidence | "
            f"{'none' if evidence is None else '`' + evidence['sha256'] + '`'} | "
            f"{'-' if evidence is None else '`' + evidence['receipt'] + '`'} |"
        )
        out.append("")
    ledger_rows(out, directory, receipt)
    arm_rows(out, receipt)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    output = pathlib.Path(sys.argv[1])
    directories = [pathlib.Path(argument) for argument in sys.argv[2:]]

    out = ["# 5G NR rate-matched encoder evidence tables\n",
           "> **Diátaxis Type:** Reference\n",
           "Generated by `dev/bench_results/12fdeb5b/summarize.py` from the committed "
           "survey records and receipts named in each section. Speedup is "
           "`median(baseline) / median(candidate)`; below 1 in a `-gap-` cell means gf2 "
           "is faster. Relative half-width is "
           "`max(estimate - lower, upper - estimate) / estimate`.\n",
           "Regenerate, from the worktree root, with:\n",
           "```\n" + " ".join(["dev/bench_results/12fdeb5b/summarize.py", *sys.argv[1:]]) + "\n```\n"]
    configuration_rows(out)
    validation_rows(out)
    if directories:
        for directory in directories:
            receipt_section(out, directory)
    else:
        out.append("## Measured cells\n")
        out.append(
            "The generator was given no receipt directory. Each directory passed on "
            "the command line contributes one section of measured cells here.\n"
        )
    output.write_text("\n".join(out) + "\n")
    print(f"tables -> {output}")


if __name__ == "__main__":
    main()
