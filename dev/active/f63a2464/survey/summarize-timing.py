#!/usr/bin/env python3
"""Project the QC campaigns' committed receipts into their tables (jit:f63a2464).

Reads only committed evidence: each family's append-only ledger and, for every
finalized stage, its receipt, saved plan, pinned addendum snapshot, execution
log and acceptance summary. Every value is derived here from those bytes and no
clock is read, so a rerun reproduces the file exactly.

The completeness line of each stage is the conclusion of the shared
`verify-campaign-log.py`, located by content like the other shared helpers, so
a stage whose log does not verify stops the generator instead of being tabled.

Usage (from the worktree root):
  summarize-timing.py RESULTS_DIR > RESULTS_DIR/timing-tables.md
"""

import hashlib
import importlib.util
import json
import pathlib
import statistics
import subprocess
import sys

SURVEY = pathlib.Path(__file__).resolve().parent
DECISION_RECORD = SURVEY.parent / "decision-record.md"
VERIFIER = "verify-campaign-log.py"
VERIFIER_OPENING = '#!/usr/bin/env python3\n"""Verify a finished campaign from its own execution log.'
FAMILIES = ["intra-frame-single-worker", "intra-frame-multicore", "comparator-single-worker"]
STAGES = ["pilot", "confirmation"]
RUN = "v4-r1"


def read_json(path):
    return json.loads(pathlib.Path(path).read_text(encoding="utf-8"))


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def output(command):
    return subprocess.run(command, check=True, capture_output=True, text=True).stdout.strip()


def log_verifier():
    """The shared campaign-log verifier's `verify`, located by name and opening."""
    helper = output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "--",
                     ":(glob)**/repository_files.py"])
    path = output([sys.executable, "-B", helper, "document", VERIFIER, VERIFIER_OPENING])
    spec = importlib.util.spec_from_file_location("verify_campaign_log", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.verify


def ending_outcomes():
    """The cell outcomes stop rule S4 of the decision record names."""
    for line in DECISION_RECORD.read_text(encoding="utf-8").split("- **S4")[1].split("\n- **")[0:1]:
        text = " ".join(line.split())
        clause = text.split(" cell ends the family")[0].split("outcome. A ")[1]
        return [word.strip("`,") for word in clause.replace(" or ", " ").split()]
    sys.exit(f"{DECISION_RECORD} states no S4 rule")


def stage_dir(results, family, stage):
    return results / f"{RUN}-f63a2464-ldpc-qc-{family}-{stage}"


def fmt(value):
    return f"{value:.4f}"


def prepared_record(arm):
    """The prepared quality record an arm's plan environment names, if any."""
    path = arm["environment"].get("GF2_LDPC_QUALITY")
    return None if path is None else pathlib.Path(path).name


def stage_tables(lines, results, family, stage, verify):
    directory = stage_dir(results, family, stage)
    lines += [f"### `{directory.name}`", ""]
    if not (directory / "receipt.json").is_file():
        lines += ["No committed receipt.", ""]
        return None
    receipt = read_json(directory / "receipt.json")
    summary = read_json(directory / "acceptance-summary.json")
    plan = read_json(directory / "plan.json")
    addendum = read_json(directory / receipt["addendum"]["snapshot"])
    declared = {cell["cell_id"]: cell for cell in addendum["cells"]}
    cases = {cell["cell_id"]: cell["case"] for cell in plan["cells"]}
    verdicts = {cell["cell_id"]: cell for cell in summary["cells"]}
    conclusion = verify(
        str(directory / "execution.log"), str(directory / "receipt.json"),
        str(directory / "plan.json"),
    )
    group = summary["family"]
    lines += [
        f"Family `{receipt['family_id']}`; label `{receipt['label']}`; acceptance "
        f"`{summary['verdict']}`; qualifies `{str(summary['qualifies']).lower()}`; findings "
        f"{len(summary['findings'])}; sessions {summary['sessions']}. Reserved comparisons "
        f"{group['comparisons']}, attempt alpha {group['attempt_alpha']}, corrected alpha "
        f"{group['corrected_alpha']}, {group['bootstrap_resamples']} bootstrap resamples.",
        "",
        f"Completeness, from the stage's own journal: {conclusion}.",
        "",
        "| Cell | Objective | Scaling | Core arm | Workers | Frames/call | Pairs | "
        "Baseline-position arm | Candidate-position arm | Baseline median ms/call | "
        "Candidate median ms/call | Speedup of medians [interval] | Confidence | "
        "Improvement margin | Equivalence margin | Decision | Outcome | Flagged windows |",
        "|---|---|---|---|---:|---:|---:|---|---|---:|---:|---|---:|---:|---:|---|---|---:|",
    ]
    for cell in receipt["cells"]:
        verdict = verdicts[cell["cell_id"]]
        spec = declared[cell["cell_id"]]
        pairs = cell["pairs"]
        interval = verdict["interval"]
        margins = verdict.get("margins") or {}
        workers = sorted({p[s]["workers_observed"] for p in pairs for s in ("baseline", "candidate")})
        median = lambda side: statistics.median(p[side]["ns_per_call"] for p in pairs) / 1e6
        lines.append(
            f"| `{cell['cell_id']}` | {spec['objective']} | {spec['scaling']} | "
            f"{cell['core_arm']} | {','.join(map(str, workers))} | "
            f"{workers[0] * cases[cell['cell_id']]['batch_size']} | {len(pairs)} | "
            f"`{cell['baseline_arm']}` | `{cell['candidate_arm']}` | {median('baseline'):.3f} | "
            f"{median('candidate'):.3f} | {fmt(interval['estimate'])} "
            f"[{fmt(interval['lower'])}, {fmt(interval['upper'])}] | {interval['confidence']} | "
            f"{margins.get('improvement', '—')} | {margins.get('equivalence', '—')} | "
            f"{verdict.get('decision') or '—'} | {verdict['outcome']} | "
            f"{verdict['flagged_windows']}/{verdict['total_windows']} |"
        )
    lines += [
        "",
        "The speedup is the protocol's ratio of the baseline-position median to the "
        "candidate-position median over the pairs, so a value above one means the "
        "candidate-position arm is faster and a value below one means the baseline-position arm "
        "is faster. Its interval is the paired percentile bootstrap at the stated confidence "
        "over the stated pairs. The per-arm medians are descriptive, over the same pairs, and "
        "carry no interval. Frames per call is workers times the per-worker batch of the plan "
        "case.",
        "",
        "Costs outside the timed windows, median over each arm's executions (descriptive, one "
        "execution per pair):",
        "",
        "| Cell | Arm | Executions | Setup ms | Pack us | Unpack us | Batch fill us | Dispatch us |",
        "|---|---|---:|---:|---:|---:|---:|---:|",
    ]
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            costs = [p[side]["conversion"] for p in cell["pairs"]]
            value = lambda key: statistics.median(c[key] for c in costs)
            lines.append(
                f"| `{cell['cell_id']}` | `{cell[side + '_arm']}` | {len(costs)} | "
                f"{value('setup_ns') / 1e6:.3f} | {value('pack_ns') / 1e3:.1f} | "
                f"{value('unpack_ns') / 1e3:.1f} | {value('batch_fill_ns') / 1e3:.1f} | "
                f"{value('dispatch_ns') / 1e3:.1f} |"
            )
    lines += [
        "",
        "Decoder quality the receipt carries. Each arm's plan environment names a prepared "
        "`c077a88b` record; the arm checks every worker's per-frame information-bit errors "
        "against that record on every execution and carries the record itself, so the memory "
        "and iteration columns describe the arm that produced the prepared record, named in "
        "the last column. FER interval: Wilson; BER interval: frame-bounded Hoeffding; both at "
        "the confidence of the interval method named in the receipt.",
        "",
        "| Cell | Arm | FER (errors/frames) | FER interval | BER (errors/bits) | BER interval | "
        "Interval method | Iterations mean / p50 / p90 / max | Stopping | Iteration cap | "
        "Memory bytes | Prepared record carried |",
        "|---|---|---|---|---|---|---|---|---|---:|---:|---|",
    ]
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            q = cell["decoder_quality"][side]
            it = q["iterations"]
            arm = cell[f"{side}_arm"]
            lines.append(
                f"| `{cell['cell_id']}` | `{arm}` | {q['fer']:.6g} "
                f"({q['frame_errors']}/{q['frames']}) | "
                f"[{q['fer_interval'][0]:.6g}, {q['fer_interval'][1]:.6g}] | "
                f"{q['ber']:.6g} ({q['bit_errors']}/{q['bits']}) | "
                f"[{q['ber_interval'][0]:.6g}, {q['ber_interval'][1]:.6g}] | "
                f"{q['interval_method']} | "
                f"{it['mean']:.3f} / {it['p50']} / {it['p90']} / {it['max']} | "
                f"{q['settings']['stopping']['kind']} | {q['settings']['iteration_cap']} | "
                f"{q['memory_bytes']} | `{prepared_record(receipt['arms'][arm])}` |"
            )
    lines.append("")
    return summary


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: summarize-timing.py RESULTS_DIR")
    results = pathlib.Path(sys.argv[1])
    verify = log_verifier()
    ending = ending_outcomes()
    lines = [
        "# QC-aware intra-frame candidate: timed campaigns",
        "",
        f"<!-- Generated by {pathlib.Path(__file__).name} of jit:f63a2464. Do not edit. -->",
        "",
        "Source: each family's append-only ledger and every finalized stage's receipt, saved "
        "plan, pinned addendum snapshot, execution log and acceptance summary, listed with "
        "their digests under Source digests.",
        "",
    ]
    confirmations = {}
    sources = []
    for family in FAMILIES:
        ledger = results / f"v4-qc-{family}-family-ledger.jsonl"
        entries = [json.loads(line) for line in ledger.read_text().splitlines() if line.strip()]
        sources.append(ledger)
        lines += [
            f"## Ledger `{ledger.name}`",
            "",
            "| Sequence | Campaign | Protocol | Reserved comparisons | Candidate identities | "
            "Addendum sha256 |",
            "|---:|---|---:|---:|---:|---|",
        ]
        for entry in entries:
            lines.append(
                f"| {entry['sequence']} | `{entry['campaign']}` | {entry['protocol_version']} | "
                f"{entry['comparisons']} | {len(entry['candidates'])} | "
                f"`{entry['addendum_sha256']}` |"
            )
        lines.append("")
        for stage in STAGES:
            summary = stage_tables(lines, results, family, stage, verify)
            if summary is None:
                continue
            directory = stage_dir(results, family, stage)
            sources += [directory / name for name in
                        ("receipt.json", "plan.json", "execution.log", "acceptance-summary.json")]
            if stage == "confirmation":
                confirmations[family] = (ledger, entries, summary)

    lines += [
        "## Stop rule S4 per ledger",
        "",
        "The decision record's rule S4 names the cell outcomes that end a family: "
        + ", ".join(f"`{outcome}`" for outcome in ending)
        + ". One row per ledger, from its confirmation's acceptance summary and its own chain.",
        "",
        "| Ledger | Confirmatory reservations in the chain | Confirmatory cells | "
        "Cell outcomes | Cells with an S4 ending outcome | Qualifies |",
        "|---|---:|---:|---|---:|---|",
    ]
    for family in FAMILIES:
        if family not in confirmations:
            lines.append(f"| `v4-qc-{family}-family-ledger.jsonl` | — | — | no confirmation | — | — |")
            continue
        ledger, entries, summary = confirmations[family]
        outcomes = [cell["outcome"] for cell in summary["cells"]]
        tally = ", ".join(f"{outcomes.count(o)} `{o}`" for o in sorted(set(outcomes)))
        lines.append(
            f"| `{ledger.name}` | {sum(1 for e in entries if e['comparisons'] > 0)} | "
            f"{len(outcomes)} | {tally} | {sum(1 for o in outcomes if o in ending)} | "
            f"`{str(summary['qualifies']).lower()}` |"
        )
    lines += ["", "## Source digests", "", "| Path | sha256 |", "|---|---|"]
    lines += [f"| `{path}` | `{digest(path)}` |" for path in sources]
    sys.stdout.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
