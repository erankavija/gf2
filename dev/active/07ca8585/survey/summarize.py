#!/usr/bin/env python3
"""Project this issue's evidence tables (jit:07ca8585).

Writes `dev/bench_results/07ca8585/tables.md` from committed evidence only: the
derived structural costs, the preparation allocation census, the steady-state
allocation counter's own run record, and every finalized receipt with its
independently recomputed acceptance summary and journaled worker placement.
Every number in this issue's prose lives here; nothing is typed in.

Re-running over unchanged evidence reproduces the file byte for byte.

Usage (from the worktree root): summarize.py
"""

import json
import pathlib
import statistics
from collections import defaultdict

# What each family's baseline and candidate arm is, for the column headings.
ARM_LABELS = {
    "ldpc-update-single-worker-v1": ("gf2 before", "gf2 after"),
    "ldpc-update-multicore-v1": ("gf2 before", "gf2 after"),
    "ldpc-update-comparator-single-worker-v1": ("gf2 after", "AFF3CT"),
    "ldpc-update-comparator-multicore-v1": ("gf2 after", "AFF3CT"),
    "ldpc-update-checknode-v1": ("gf2 check pass", "AFF3CT check pass"),
    "ldpc-update-fixed-iteration-v1": ("gf2 after at the cap", "AFF3CT at the cap"),
}

BASE = pathlib.Path("dev/bench_results/07ca8585")
PREPARATION = BASE / "preparation"
SURVEY = pathlib.Path("dev/active/07ca8585/survey")


def read_json(path):
    return json.loads(pathlib.Path(path).read_text(encoding="utf-8"))


def read_jsonl(path):
    return [
        json.loads(line)
        for line in pathlib.Path(path).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def structural(lines):
    data = read_json(PREPARATION / "structural-costs.json")
    codes = data["codes"]
    keys = list(codes)
    lines += [
        "## Structural work per flooding iteration",
        "",
        "Source: `preparation/structural-costs.json` "
        "(`dev/active/07ca8585/survey/edge-costs.py`). Exact counts derived from each recorded "
        "graph and the loop structure of each generation; not timings. The `before` counts are "
        "the predecessor's, read from the file that derivation names rather than recomputed.",
        "",
        "| Quantity | " + " | ".join(keys) + " |",
        "|---|" + "---:|" * len(keys),
    ]
    rows = [("edges", lambda c: c["edges"]), ("checks", lambda c: c["m"])]
    for side in ("before_per_iteration", "after_per_iteration", "aff3ct_per_iteration"):
        label = side.split("_")[0]
        for key in codes[keys[0]][side]:
            rows.append(
                (f"{label} {key.replace('_', ' ')}", lambda c, s=side, k=key: c[s].get(k, "—"))
            )
    rows += [
        (
            "before position-search comparisons per edge",
            lambda c: round(
                (
                    c["before_per_iteration"]["check_position_search_comparisons"]
                    + c["before_per_iteration"]["variable_position_search_comparisons"]
                )
                / c["edges"],
                3,
            ),
        ),
        (
            "before check inputs gathered per edge",
            lambda c: round(c["before_per_iteration"]["check_inputs_gathered"] / c["edges"], 3),
        ),
        (
            "after check inputs read per edge",
            lambda c: round(c["after_per_iteration"]["check_inputs_read"] / c["edges"], 3),
        ),
    ]
    for name, value in rows:
        lines.append(f"| {name} | " + " | ".join(str(value(codes[key])) for key in keys) + " |")
    lines += [
        "",
        "A `—` marks a quantity one loop structure has and another does not.",
        "",
    ]


def census(lines):
    records = defaultdict(list)
    for record in read_jsonl(PREPARATION / "alloc-census.jsonl"):
        records[(record["code"], record["generation"])].append(record)
    lines += [
        "## Allocation census of the measured harness (untimed)",
        "",
        "Source: `preparation/alloc-census.jsonl` "
        "(`dev/active/07ca8585/survey/record-alloc-census.py`). A counting global allocator around "
        "each decoded frame of the `3be770d5` harness, one row per code and generation over the "
        "same recorded frames. Counts are exact and deterministic for a fixed decoder and input; "
        "they are not timings. Both generations decode through the harness's owning entry point, "
        "which returns a codeword vector, so the `after` floor here is one request per frame; the "
        "zero steady-state claim is the separate table below.",
        "",
        "| Code | Generation | Frames | Allocations per frame | Reallocations | Deallocations | "
        "Bytes per frame | Iterations per frame | Total bit errors against the frozen evidence |",
        "|---|---|---:|---|---:|---|---|---|---:|",
    ]
    for (code, generation), rows in sorted(records.items()):
        allocations = sorted({row["allocations"] for row in rows})
        deallocations = sorted({row["deallocations"] for row in rows})
        byte_counts = sorted({row["bytes_requested"] for row in rows})
        iterations = [row["iterations"] for row in rows]
        span = lambda values: (
            str(values[0]) if len(values) == 1 else f"{values[0]}–{values[-1]}"
        )
        lines.append(
            f"| {code} | {generation} | {len(rows)} | {span(allocations)} | "
            f"{sum(row['reallocations'] for row in rows)} | {span(deallocations)} | "
            f"{span(byte_counts)} | {span(sorted(set(iterations)))} | "
            f"{sum(row['bit_errors'] for row in rows)} |"
        )
    iterations = defaultdict(dict)
    errors = defaultdict(dict)
    for (code, generation), rows in records.items():
        for row in rows:
            iterations[(code, row["frame"])][generation] = row["iterations"]
            errors[(code, row["frame"])][generation] = row["bit_errors"]
    agreeing = sum(1 for counts in iterations.values() if len(set(counts.values())) == 1)
    clean = sum(1 for counts in errors.values() if set(counts.values()) == {0})
    lines += [
        "",
        f"Frames censused per code and generation: {len(iterations)} frame identities. Frames whose "
        f"two generations agree on the iteration count: {agreeing}. Frames both generations decode "
        f"with no bit error against the frozen `c077a88b` per-frame evidence: {clean}. That "
        "agreement is the behavioural check this table carries beside the allocation counts; it is "
        "computed here from the census rather than asserted.",
        "",
    ]


def allocation_counter(lines):
    path = PREPARATION / "allocation-counter.jsonl"
    lines += [
        "## Steady-state allocation counter (REQ-07)",
        "",
        "Source: `preparation/allocation-counter.jsonl`, the records "
        "`crates/gf2-coding/tests/ldpc_decode_allocations.rs` prints from its own counted "
        "sections. A thread-local counting global allocator records only what the thread inside a "
        "counted section requests. Steady state is a decode through the prepared-workspace entry "
        "point with the decoder and the caller's buffer already used once.",
        "",
    ]
    if not path.exists():
        lines += ["No run record is available.", ""]
        return
    records = read_jsonl(path)
    lines += [
        "| Phase | Case | Allocations | Reallocations | Deallocations | Bytes requested |",
        "|---|---|---:|---:|---:|---:|",
    ]
    for record in records:
        lines.append(
            f"| {record['phase']} | {record['case']} | {record['allocations']} | "
            f"{record['reallocations']} | {record['deallocations']} | "
            f"{record['bytes_requested']} |"
        )
    steady = [r for r in records if r["phase"].startswith("steady-state")]
    zero = sum(
        1
        for r in steady
        if r["allocations"] == 0 and r["reallocations"] == 0 and r["deallocations"] == 0
    )
    lines += [
        "",
        f"Steady-state sections recorded: {len(steady)}; sections that requested nothing: {zero}. "
        "The test asserts that relation, so a run that does not hold it fails rather than "
        "publishing.",
        "",
    ]


def placements(receipt_dir):
    """Placement reports the runner journaled, by (cell, arm)."""
    reports = defaultdict(list)
    for line in (receipt_dir / "execution.log").read_text(encoding="utf-8").splitlines():
        record = json.loads(line)
        if record["event"] != "child-diagnostic":
            continue
        for text in record["details"].get("stderr", "").splitlines():
            if text.startswith('{"kind":"worker-placement"'):
                reports[(record["case"]["cell_id"], record["case"]["arm"])].append(
                    json.loads(text)
                )
    return reports


def receipts(lines):
    found = sorted(BASE.glob("*-07ca8585-ldpc-update-*/receipt.json"))
    lines += ["## Campaigns", ""]
    if not found:
        lines += ["No finalized receipt is available.", ""]
        return
    for path in found:
        directory = path.parent
        receipt = read_json(path)
        summary = read_json(directory / "acceptance-summary.json")
        plan = read_json(directory / "plan.json")
        cases = {cell["cell_id"]: cell["case"] for cell in plan["cells"]}
        verdicts = {cell["cell_id"]: cell for cell in summary["cells"]}
        quality_notes = {f["cell"] for f in summary["findings"] if f["rule"] == "P-19"}
        placed = placements(directory)
        baseline_label, candidate_label = ARM_LABELS[receipt["family_id"]]
        lines += [
            f"### `{directory.name}`",
            "",
            f"Source: `{directory.name}/receipt.json` and its acceptance summary. Family "
            f"`{receipt['family_id']}`; label **{receipt['label']}**; acceptance "
            f"**{summary['verdict']}**; qualifies **{summary['qualifies']}**; findings on this "
            f"line: {len(summary['findings'])}; ledger-derived attempt alpha "
            f"{summary['family']['family_alpha']}, reserved comparisons "
            f"{summary['family']['comparisons']}. SMT active: {receipt['host']['smt_active']}.",
            "",
            f"| Cell | Core arm | CPUs | Workers observed | Pairs | Frames/call | "
            f"{baseline_label} median ms/call | {candidate_label} median ms/call | "
            f"{baseline_label} / {candidate_label} time [interval] | Confidence | "
            f"Flagged windows | Outcome |",
            "|---|---|---:|---|---:|---:|---:|---:|---|---:|---:|---|",
        ]
        for cell in receipt["cells"]:
            verdict = verdicts[cell["cell_id"]]
            interval = verdict.get("interval")
            outcome = verdict["outcome"] + (
                "; quality admission unestablished (P-19)"
                if cell["cell_id"] in quality_notes
                else ""
            )
            pairs = cell["pairs"]
            if not pairs:
                lines.append(
                    f"| {cell['cell_id']} | {cell['core_arm']} | 0 | — | 0 | — | — | — | — | — | "
                    f"— | {outcome}: {cell.get('unavailable_reason')} |"
                )
                continue
            workers = sorted(
                {p[side]["workers_observed"] for p in pairs for side in ("baseline", "candidate")}
            )
            case = cases[cell["cell_id"]]
            # A whole-decoding cell declares its per-worker batch of frames; an
            # isolated check-node cell declares the prepared frames it passes over.
            frames = workers[0] * case.get("batch_size", case.get("frames", 0))
            median = lambda side: statistics.median(p[side]["ns_per_call"] for p in pairs) / 1e6
            text = (
                f"{interval['estimate']:.4g} [{interval['lower']:.4g}, {interval['upper']:.4g}]"
                if interval
                else "unavailable"
            )
            lines.append(
                f"| {cell['cell_id']} | {cell['core_arm']} | {len(cell['resolved_cpus'])} | "
                f"{','.join(map(str, workers))} | {len(pairs)} | {frames} | "
                f"{median('baseline'):.3f} | {median('candidate'):.3f} | {text} | "
                f"{interval['confidence'] if interval else '—'} | "
                f"{verdict['flagged_windows']}/{verdict['total_windows']} | {outcome} |"
            )
        lines += [
            "",
            "The time ratio is the protocol's speedup of medians with the baseline arm as "
            "reference: values above one mean the candidate arm is faster. Medians are "
            "descriptive; the paired bootstrap interval is the estimate. Frames per call is "
            "workers times the per-worker batch.",
            "",
            "Untimed diagnostics (median over pairs) and journaled placement:",
            "",
            "| Cell | Arm | Setup ms | Pack ms | Dispatch us | Placement reports | "
            "Process threads | All workers pinned |",
            "|---|---|---:|---:|---:|---:|---|---|",
        ]
        for cell in receipt["cells"]:
            for side in ("baseline", "candidate"):
                conversions = [p[side]["conversion"] for p in cell["pairs"]]
                if not conversions:
                    continue
                arm = cell[f"{side}_arm"]
                reports = placed.get((cell["cell_id"], arm), [])
                threads = sorted({(r["threads"]["ready"], r["threads"]["after"]) for r in reports})
                pinned = all(
                    w["affinity"] == [w["cpu"]] and set(w["cpus_seen"]) <= {w["cpu"]}
                    for r in reports
                    for w in r["workers"]
                )
                value = lambda key: statistics.median(c[key] for c in conversions)
                lines.append(
                    f"| {cell['cell_id']} | {arm} | {value('setup_ns') / 1e6:.3f} | "
                    f"{value('pack_ns') / 1e6:.3f} | {value('dispatch_ns') / 1e3:.1f} | "
                    f"{len(reports)} | {threads} | {pinned and bool(reports)} |"
                )
        if not any(cell.get("decoder_quality") for cell in receipt["cells"]):
            lines += [
                "",
                "This family's cells declare no decoder, so the receipt carries no per-frame "
                "decoder quality: an isolated check-node pass decodes no frame. What stands in its "
                "place is the matched-ness receipt above and the checksums every worker of either "
                "arm reproduces.",
                "",
            ]
            continue
        corpus = (
            "this issue's fixed-iteration"
            if "fixed-iteration" in receipt["family_id"]
            else "prepared `c077a88b`"
        )
        lines += [
            "",
            f"Decoder quality carried by the receipt ({corpus} evidence; FER Wilson "
            "95%, BER independent-frame Hoeffding 95%), with the iteration and early-exit "
            "distribution of each arm:",
            "",
            "| Cell | Arm | FER (errors/frames) | FER interval | BER (errors/bits) | "
            "BER interval | Iterations mean / p50 / p90 / max | Wave |",
            "|---|---|---|---|---|---|---|---:|",
        ]
        for cell in receipt["cells"]:
            quality = cell.get("decoder_quality")
            if not quality:
                continue
            for side in ("baseline", "candidate"):
                q = quality[side]
                it = q["iterations"]
                lines.append(
                    f"| {cell['cell_id']} | {side} | {q['fer']:.6g} "
                    f"({q['frame_errors']}/{q['frames']}) | "
                    f"[{q['fer_interval'][0]:.6g}, {q['fer_interval'][1]:.6g}] | "
                    f"{q['ber']:.6g} ({q['bit_errors']}/{q['bits']}) | "
                    f"[{q['ber_interval'][0]:.6g}, {q['ber_interval'][1]:.6g}] | "
                    f"{it['mean']:.3f} / {it['p50']} / {it['p90']} / {it['max']} | "
                    f"{q['settings']['batch_size']} |"
                )
        lines.append("")


def checknode_parity(lines):
    path = PREPARATION / "checknode-parity.jsonl"
    lines += [
        "## Matched-ness of the isolated check-node arms",
        "",
        "Source: `preparation/checknode-parity.jsonl` "
        "(`dev/active/07ca8585/survey/arms/src/bin/ldpc-checknode-verify.rs`). Untimed: both "
        "check-node passes run in one process over the prepared array the timed cells declare, and "
        "their outputs are compared bit for bit. The checksums are the 64-bit FNV-1a of the "
        "canonical check-major message array; the timed cells declare them and every worker of "
        "either arm reproduces them or the arm fails.",
        "",
    ]
    if not path.exists():
        lines += ["No matched-ness receipt is available.", ""]
        return
    lines += [
        "| Code | Checks | Edges | Max check degree | Frames | Warm-up rounds | Input checksum | "
        "gf2 output | AFF3CT output | Differing outputs | AFF3CT transpose is the canonical map |",
        "|---|---:|---:|---:|---:|---:|---|---|---|---:|---|",
    ]
    rows = read_jsonl(path)
    for row in rows:
        lines.append(
            f"| {row['code']} | {row['checks']} | {row['edges']} | {row['max_check_degree']} | "
            f"{row['frames']} | {row['warmup_rounds']} | `{row['input_checksum']}` | "
            f"`{row['gf2_output_checksum']}` | `{row['aff3ct_output_checksum']}` | "
            f"{row['differing_outputs']} | {row['aff3ct_transpose_is_the_canonical_map']} |"
        )
    identical = sum(1 for row in rows if row["outputs_bit_identical"])
    rules = sorted({row["aff3ct_rule"] for row in rows})
    lines += [
        "",
        f"Codes compared: {len(rows)}; codes whose two passes write bit-identical outputs: "
        f"{identical}. The AFF3CT side is {', '.join(f'`{rule}`' for rule in rules)}. The tool "
        "exits nonzero when a code's outputs differ, so a run that does not hold this fails rather "
        "than publishing.",
        "",
    ]


def fixed_quality(lines):
    directory = PREPARATION / "quality-fixed"
    lines += [
        "## Prepared quality at the iteration cap (untimed)",
        "",
        "Source: `preparation/quality-fixed/` "
        "(`dev/active/07ca8585/survey/arms/src/bin/ldpc-fixed-quality.rs`). The corpus REQ-10's "
        "full-iteration cells decode against: every recorded frame of each frozen workload decoded "
        "once per arm with syndrome stopping off, so each arm performs exactly the declared cap on "
        "every frame. The reused `c077a88b` corpus was produced under syndrome stopping, and the "
        "arms refuse a corpus whose settings differ from theirs. Every field below is a "
        "deterministic function of the decoder and the recorded input.",
        "",
    ]
    files = sorted(directory.glob("*.json"))
    if not files:
        lines += ["No fixed-iteration corpus is available.", ""]
        return
    lines += [
        "| Corpus | Frames | Frame errors | Bit errors / bits | Iterations mean / p50 / max | "
        "Stopping | Iteration cap |",
        "|---|---:|---:|---|---|---|---:|",
    ]
    for path in files:
        record = read_json(path)
        it = record["iterations"]
        settings = record["settings"]
        lines.append(
            f"| `{path.name}` | {record['frames']} | {record['frame_errors']} | "
            f"{record['bit_errors']}/{record['bits']} | "
            f"{it['mean']:.3f} / {it['p50']} / {it['max']} | {settings['stopping']['kind']} | "
            f"{settings['iteration_cap']} |"
        )
    lines.append("")


def degrees(lines):
    data = read_json(PREPARATION / "structural-costs.json")["codes"]
    lines += [
        "## Representative degree distributions",
        "",
        "Source: `preparation/structural-costs.json`, the recorded graphs of the two frozen "
        "workloads. The check degree is what the shared reduction's two passes are linear in.",
        "",
        "| Code | Check degrees (degree: checks) | Variable degrees (degree: variables) |",
        "|---|---|---|",
    ]
    for key, code in data.items():
        checks = ", ".join(f"{d}: {c}" for d, c in code["check_degree_histogram"])
        variables = ", ".join(f"{d}: {c}" for d, c in code["variable_degree_histogram"])
        lines.append(f"| {key} | {checks} | {variables} |")
    lines.append("")


def main():
    lines = [
        "# Min-sum update evidence tables",
        "",
        "Generated by `dev/active/07ca8585/survey/summarize.py` from committed evidence. Do not "
        "edit: re-running the generator over unchanged evidence reproduces this file byte for "
        "byte.",
        "",
    ]
    structural(lines)
    degrees(lines)
    census(lines)
    allocation_counter(lines)
    checknode_parity(lines)
    fixed_quality(lines)
    receipts(lines)
    BASE.mkdir(parents=True, exist_ok=True)
    (BASE / "tables.md").write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
    print(BASE / "tables.md")


if __name__ == "__main__":
    main()
