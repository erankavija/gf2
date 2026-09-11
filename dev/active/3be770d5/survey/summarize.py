#!/usr/bin/env python3
"""Project the survey's evidence tables (jit:3be770d5).

Writes `dev/bench_results/3be770d5/tables.md` from committed evidence only:
the structural costs, the preparation allocation census and validations, and
every finalized steady-state receipt with its independently recomputed
acceptance summary and journaled worker placement. Every number in the
survey's prose lives here or in the profile summary; nothing is typed in.

Usage (from the worktree root): summarize.py
"""

import json
import pathlib
import statistics
from collections import defaultdict

BASE = pathlib.Path("dev/bench_results/3be770d5")
PREPARATION = BASE / "preparation"


def structural(lines):
    data = json.loads((PREPARATION / "structural-costs.json").read_text())
    lines += ["## Structural work per flooding iteration", "",
              "Source: `preparation/structural-costs.json` (`dev/active/3be770d5/survey/edge-costs.py`). "
              "Exact counts derived from each recorded graph and the loop structure of both decoders; "
              "not timings.", "",
              "| Quantity | " + " | ".join(data["codes"]) + " |", "|---|" + "---:|" * len(data["codes"])]
    codes = list(data["codes"].values())
    rows = [("edges", lambda c: c["edges"])]
    for side in ("gf2_per_iteration", "aff3ct_per_iteration"):
        for key in codes[0][side]:
            rows.append((f"{side.split('_')[0]} {key.replace('_', ' ')}", lambda c, s=side, k=key: c[s][k]))
    rows += [
        ("gf2 position-search comparisons per edge",
         lambda c: round((c["gf2_per_iteration"]["check_position_search_comparisons"]
                          + c["gf2_per_iteration"]["variable_position_search_comparisons"]) / c["edges"], 3)),
        ("gf2 check inputs gathered per edge",
         lambda c: round(c["gf2_per_iteration"]["check_inputs_gathered"] / c["edges"], 3)),
    ]
    for name, value in rows:
        lines.append(f"| {name} | " + " | ".join(str(value(c)) for c in codes) + " |")
    lines.append("")


def census(lines):
    data = json.loads((PREPARATION / "structural-costs.json").read_text())["codes"]
    keys = {"dvb-t2-r12": "dvb-t2-r12", "nr-bg1-r12": "nr-bg1-z384"}
    records = defaultdict(list)
    for line in (PREPARATION / "alloc-census.jsonl").read_text().splitlines():
        record = json.loads(line)
        records[record["code"]].append(record)
    lines += ["## Allocation census (untimed preparation run)", "",
              "Source: `preparation/alloc-census.jsonl`. Counting global allocator around each decoded "
              "frame of one reused gf2 decoder; counts are deterministic.", "",
              "| Code | Frames | Allocations | Equal to edges x iterations + 2 x (iterations + 1) | "
              "Deallocations equal allocations | Reallocations | Distinct request sizes |",
              "|---|---:|---:|---|---|---:|---:|"]
    for code, rows in records.items():
        edges = data[keys[code]]["edges"]
        matches = all(r["allocations"] == edges * r["iterations"] + 2 * (r["iterations"] + 1) for r in rows)
        sizes = {size for r in rows for size, _ in r["sizes"]}
        lines.append(f"| {code} | {len(rows)} | {sum(r['allocations'] for r in rows)} | {matches} | "
                     f"{all(r['allocations'] == r['deallocations'] for r in rows)} | "
                     f"{sum(r['reallocations'] for r in rows)} | {len(sizes)} |")
    lines.append("")


def validations(lines):
    replay = [json.loads(line) for line in (PREPARATION / "validation.jsonl").read_text().splitlines()]
    arms = [json.loads(line) for line in (PREPARATION / "arm-validation.jsonl").read_text().splitlines()]
    lines += ["## Untimed validation", "",
              "Sources: `preparation/validation.jsonl` (every recorded frame through the reused-decoder, "
              "multi-worker path) and `preparation/arm-validation.jsonl` (validation-role run of each arm "
              "of each pilot cell with all placement and prepared-quality checks).", "",
              "| Arm | Code | Workers | Frames | Error vector equals prepared | Iterations equal prepared |",
              "|---|---|---:|---:|---|---|"]
    for record in replay:
        verdict = record["verdict"]
        lines.append(f"| {record['arm']} | {record['code_key']} | {verdict['workers']} | {verdict['frames']} | "
                     f"{verdict['frame_bit_errors_match_prepared']} | "
                     f"{'not observed' if verdict['iterations_match_prepared'] is None else verdict['iterations_match_prepared']} |")
    passed = sum(record["passed"] for record in arms)
    lines += ["", f"Validation-role arm executions passed: {passed} of {len(arms)}.", ""]


def placements(receipt_dir):
    """Placement reports the runner journaled, by (cell, arm)."""
    reports = defaultdict(list)
    for line in (receipt_dir / "execution.log").read_text().splitlines():
        record = json.loads(line)
        if record["event"] != "child-diagnostic":
            continue
        for text in record["details"].get("stderr", "").splitlines():
            if text.startswith('{"kind":"worker-placement"'):
                reports[(record["case"]["cell_id"], record["case"]["arm"])].append(json.loads(text))
    return reports


def receipts(lines):
    found = sorted(BASE.glob("*-3be770d5-ldpc-steady-*/receipt.json"))
    lines += ["## Steady-state campaigns", ""]
    if not found:
        lines += ["No finalized receipt is available.", ""]
        return
    for path in found:
        directory = path.parent
        receipt = json.loads(path.read_text())
        summary = json.loads((directory / "acceptance-summary.json").read_text())
        plan = json.loads((directory / "plan.json").read_text())
        cases = {cell["cell_id"]: cell["case"] for cell in plan["cells"]}
        verdicts = {cell["cell_id"]: cell for cell in summary["cells"]}
        quality_notes = {f["cell"] for f in summary["findings"] if f["rule"] == "P-19"}
        placed = placements(directory)
        lines += [f"### `{directory.name}`", "",
                  f"Source: `{directory.name}/receipt.json` and its acceptance summary. "
                  f"Family `{receipt['family_id']}`; label **{receipt['label']}**; acceptance "
                  f"**{summary['verdict']}**; ledger-derived attempt alpha {summary['family']['family_alpha']}, "
                  f"reserved comparisons {summary['family']['comparisons']}. SMT active: "
                  f"{receipt['host']['smt_active']}. Nothing is adopted.", "",
                  "| Cell | Core arm | CPUs | Workers observed | Pairs | Frames/call | gf2 median ms/call | "
                  "AFF3CT median ms/call | gf2 / AFF3CT time [interval] | Confidence | Flagged windows | Outcome |",
                  "|---|---|---:|---|---:|---:|---:|---:|---|---:|---:|---|"]
        for cell in receipt["cells"]:
            verdict = verdicts[cell["cell_id"]]
            interval = verdict.get("interval")
            outcome = verdict["outcome"] + ("; quality admission unestablished (P-19)"
                                            if cell["cell_id"] in quality_notes else "")
            pairs = cell["pairs"]
            if not pairs:
                lines.append(f"| {cell['cell_id']} | {cell['core_arm']} | 0 | — | 0 | — | — | — | — | — | — | "
                             f"{outcome}: {cell.get('unavailable_reason')} |")
                continue
            workers = sorted({p[side]["workers_observed"] for p in pairs for side in ("baseline", "candidate")})
            frames = workers[0] * cases[cell["cell_id"]]["batch_size"]
            median = lambda side: statistics.median(p[side]["ns_per_call"] for p in pairs) / 1e6
            text = (f"{interval['estimate']:.4g} [{interval['lower']:.4g}, {interval['upper']:.4g}]"
                    if interval else "unavailable")
            lines.append(f"| {cell['cell_id']} | {cell['core_arm']} | {len(cell['resolved_cpus'])} | "
                         f"{','.join(map(str, workers))} | {len(pairs)} | {frames} | {median('baseline'):.3f} | "
                         f"{median('candidate'):.3f} | {text} | {interval['confidence'] if interval else '—'} | "
                         f"{verdict['flagged_windows']}/{verdict['total_windows']} | {outcome} |")
        lines += ["", "The time ratio is the protocol's speedup of medians with gf2 as baseline: values above "
                  "one mean AFF3CT is faster. Medians are descriptive; the paired bootstrap interval is the "
                  "estimate. Frames/call is workers times the per-worker batch.", "",
                  "Untimed diagnostics (median over pairs) and journaled placement:", "",
                  "| Cell | Arm | Setup ms | Pack ms | Dispatch us | Placement reports | Process threads | "
                  "All workers pinned |", "|---|---|---:|---:|---:|---:|---|---|"]
        for cell in receipt["cells"]:
            for side in ("baseline", "candidate"):
                conversions = [p[side]["conversion"] for p in cell["pairs"]]
                if not conversions:
                    continue
                arm = cell[f"{side}_arm"]
                reports = placed.get((cell["cell_id"], arm), [])
                threads = sorted({(r["threads"]["ready"], r["threads"]["after"]) for r in reports})
                pinned = all(w["affinity"] == [w["cpu"]] and set(w["cpus_seen"]) <= {w["cpu"]}
                             for r in reports for w in r["workers"])
                value = lambda key: statistics.median(c[key] for c in conversions)
                lines.append(f"| {cell['cell_id']} | {arm} | {value('setup_ns') / 1e6:.3f} | "
                             f"{value('pack_ns') / 1e6:.3f} | {value('dispatch_ns') / 1e3:.1f} | {len(reports)} | "
                             f"{threads} | {pinned and bool(reports)} |")
        lines += ["", "Decoder quality carried by the receipt (prepared `c077a88b` evidence; FER Wilson 95%, "
                  "BER independent-frame Hoeffding 95%):", "",
                  "| Cell | Arm | FER (errors/frames) | FER interval | BER (errors/bits) | BER interval | "
                  "Iterations mean / p50 / p90 / max | Wave |", "|---|---|---|---|---|---|---|---:|"]
        for cell in receipt["cells"]:
            quality = cell.get("decoder_quality")
            if not quality:
                continue
            for side in ("baseline", "candidate"):
                q = quality[side]
                it = q["iterations"]
                lines.append(f"| {cell['cell_id']} | {side} | {q['fer']:.6g} ({q['frame_errors']}/{q['frames']}) | "
                             f"[{q['fer_interval'][0]:.6g}, {q['fer_interval'][1]:.6g}] | "
                             f"{q['ber']:.6g} ({q['bit_errors']}/{q['bits']}) | "
                             f"[{q['ber_interval'][0]:.6g}, {q['ber_interval'][1]:.6g}] | "
                             f"{it['mean']:.3f} / {it['p50']} / {it['p90']} / {it['max']} | "
                             f"{q['settings']['batch_size']} |")
        lines.append("")


def main():
    lines = ["# Steady-state LDPC survey tables", "", "> **Diátaxis Type:** Reference", "",
             "Generated by `dev/active/3be770d5/survey/summarize.py` from committed evidence; each table "
             "names its source. The profile series has its own generated `profile.md` in its directory.", ""]
    structural(lines)
    census(lines)
    validations(lines)
    receipts(lines)
    (BASE / "tables.md").write_text("\n".join(lines).rstrip("\n") + "\n")


if __name__ == "__main__":
    main()
