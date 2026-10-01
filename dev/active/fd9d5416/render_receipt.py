#!/usr/bin/env python3
"""Render the jit:fd9d5416 BCH performance receipt from one window run.

Reads the run directory `dev/active/d1b4f85e/run.sh` writes (`host.txt`,
`logs/<bench>-criterion.txt`, `dispatch.jsonl`, and the Criterion
`benchmark.json`/`estimates.json`/`sample.json` triples under `samples/`),
the pinned pre-cutover baseline run (`dev/bench_results/88ca7d2f/`), and the
committed external-baseline survey CSVs (`dev/bench_results/4e732b56/`), and
prints the receipt markdown to stdout. Every figure is computed from those
files at render time; the protocol it applies is
`dev/active/fd9d5416/receipt-protocol.md`.

Usage:
  render_receipt.py <run-dir> [--baseline-dir DIR] [--survey-dir DIR]

Exit status: 0 when every verdict-bearing check passes, 1 when one fails or a
verdict-bearing cell is missing, 2 on unreadable input.
"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import random
import re
import statistics
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
ISSUE = "fd9d5416"

# Protocol constants: plan.md `evidence-protocol` (resamples, confidence,
# threshold) and receipt-protocol.md (statistic, seed).
BOOTSTRAP_RESAMPLES = 10_000
CONFIDENCE = 0.95
ACCEPT_RATIO = 0.98
BOOTSTRAP_SEED = 0xAE03BCD0

BENCH_LABELS = ("batch_operations", "bch_parallel", "bch_encode_w1", "bch_genmatrix")

# Tier A: the pinned pre-cutover receipt, mapped by dev/active/591a1c5e/smoke-run.md.
TIER_A_KEPT = (
    "bch_batch_decode/1",
    "bch_batch_decode/10",
    "bch_batch_decode/50",
    "bch_batch_decode/100",
    "bch_single_vs_batch/single_loop",
    "bch_sequential_vs_batch/sequential_loop",
    "bch_sequential_vs_batch/batch_operation",
)
TIER_A_EXCLUDED = (
    ("bch_batch/1", "bch_encode_pns_16383_16215/1", "renamed; the baseline code (16200, 16008) has no canonical counterpart"),
    ("bch_batch/10", "bch_encode_pns_16383_16215/10", "renamed; the baseline code (16200, 16008) has no canonical counterpart"),
    ("bch_batch/50", "bch_encode_pns_16383_16215/50", "renamed; the baseline code (16200, 16008) has no canonical counterpart"),
    ("bch_batch/100", "bch_encode_pns_16383_16215/100", "renamed; the baseline code (16200, 16008) has no canonical counterpart"),
    ("bch_single_vs_batch/batch_api", None, "removed; the canonical DVB-T2 decoder has no batch call"),
    (None, "bch_single_vs_batch/decode_into_loop", "new; allocation-free decode, no baseline cell"),
)

# Tier B: the survey's pre-cutover gf2 column (legacy `encode_batch`,
# `generator_matrix`), contract rows and batches of workload-selection.md.
CONTRACT_ROWS = ("B1", "B2", "B3", "T2S", "T2N")
CONTRACT_BATCHES = (1, 16, 256, 4096)
DVB_T2_ROWS = ("T2S", "T2N")
TIER_B_W1_IN_VERDICT = True
TIER_B_W2_IN_VERDICT = False
TIER_B_W2_REASON = (
    "legacy timed region includes code construction "
    "(`cs.build().generator_matrix()`); the Criterion cell times "
    "`generator_matrix()` on a prebuilt code"
)

# External baselines selected in workload-selection.md section 8.
W1_EXTERNAL = (("bchlib", "table-remainder"), ("aff3ct", "lfsr-simd-inter"), ("aff3ct", "lfsr-scalar"))
W2_EXTERNAL = (("m4ri", "genmatrix-rref"), ("aff3ct", "basis-encode-pack"))


class InputError(Exception):
    pass


# ----------------------------------------------------------------- reading


def sha256_of(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def parse_host_facts(host_file: Path) -> dict:
    facts: dict = {}
    sections: dict[str, list[str]] = {}
    section = None
    for line in host_file.read_text().splitlines():
        for prefix, key in (
            ("# command: ", "command"),
            ("# generated: ", "generated"),
            ("# gf2 revision: ", "revision"),
            ("# tree state at run start: ", "tree_state"),
        ):
            if line.startswith(prefix):
                facts[key] = line[len(prefix):]
        if line.startswith("## "):
            section = line[3:]
            sections[section] = []
        elif section is not None and line.strip():
            sections[section].append(line.strip())
    get = lambda name: " ".join(sections.get(name, []))
    facts["uptime"] = get("uptime at run start")
    facts["uname"] = get("uname")
    facts["cpu_model"] = get("CPU model (/proc/cpuinfo)")
    facts["cpu_flags"] = get("CPU flags relevant to the encode kernel bundle")
    facts["nproc"] = get("nproc")
    facts["governor"] = get("CPU frequency governor (cpu0)")
    toolchain = sections.get("toolchain", [])
    facts["rustc"] = next((l for l in toolchain if l.startswith("rustc ")), "")
    facts["cargo"] = next((l for l in toolchain if l.startswith("cargo ")), "")
    for line in sections.get("measurement window", []):
        if line.startswith("start_utc: "):
            facts["start_utc"] = line[len("start_utc: "):]
        elif line.startswith("end_utc: "):
            facts["end_utc"] = line[len("end_utc: "):]
    return facts


def parse_log_header(log_file: Path) -> dict:
    fields = {}
    for line in log_file.read_text().splitlines():
        for prefix in ("command", "started_utc", "load_avg_start", "finished_utc", "load_avg_end"):
            if line.startswith(f"# {prefix}: "):
                fields[prefix] = line[len(prefix) + 4:]
    return fields


def load_criterion(samples_dir: Path) -> dict:
    """Every Criterion output under `samples_dir`, keyed by full ID."""
    cells = {}
    if not samples_dir.is_dir():
        raise InputError(f"no samples directory at {samples_dir}")
    for d in sorted(p for p in samples_dir.iterdir() if p.is_dir()):
        bench = json.loads((d / "benchmark.json").read_text())
        est = json.loads((d / "estimates.json").read_text())
        sample = json.loads((d / "sample.json").read_text())
        per_iter = [t / i for t, i in zip(sample["times"], sample["iters"])]
        median = est["median"]
        cells[bench["full_id"]] = {
            "dir": d,
            "samples": per_iter,
            "mode": sample.get("sampling_mode", ""),
            "median_ns": median["point_estimate"],
            "ci_lower_ns": median["confidence_interval"]["lower_bound"],
            "ci_upper_ns": median["confidence_interval"]["upper_bound"],
        }
    return cells


def load_dispatch(record: Path) -> list[dict]:
    return [json.loads(l) for l in record.read_text().splitlines() if l.strip()]


def load_survey(survey_dir: Path) -> tuple[dict, list[Path]]:
    """Survey trials keyed by (lib, workload, algorithm, row, batch).

    Each value holds per-call wall times in ns and the shape; per-call time
    is the cell's work units over its throughput column (Mbit/s).
    """
    trials: dict = {}
    used = []
    for path in sorted(survey_dir.glob("*-4e732b56-*.csv")):
        used.append(path)
        with path.open() as fh:
            for row in csv.DictReader(fh):
                lib, workload = row["lib"], row["workload"]
                n, k, batch = int(row["n"]), int(row["k"]), int(row["batch"])
                if workload == "W2" and batch != k:
                    continue  # M4RI's `matmul-m4rm` substrate cells, not a W2 materialization
                mbit = float(row.get("info_mbit_per_s") or row.get("bits_per_s_scaled"))
                units = batch * k if workload == "W1" else k * n
                key = (lib, workload, row["algorithm"], row["code"], batch if workload == "W1" else None)
                cell = trials.setdefault(key, {"n": n, "k": k, "units": units, "times": [], "files": set()})
                cell["times"].append(units / mbit * 1e3)
                cell["files"].add(path.name)
    return trials, used


# ------------------------------------------------------------- statistics


def lower_bound_ratio(base: list[float], new: list[float], cell: str) -> tuple[float, float]:
    """Point ratio and one-sided lower bound of throughput new/baseline.

    Throughput at fixed work is the reciprocal of time, so the ratio is
    median(base)/median(new) over per-iteration times. Two-sample percentile
    bootstrap: each resample draws both sides with replacement.
    """
    point = statistics.median(base) / statistics.median(new)
    rng = random.Random(f"{BOOTSTRAP_SEED}:{cell}")
    nb, nn = len(base), len(new)
    ratios = sorted(
        statistics.median(rng.choices(base, k=nb)) / statistics.median(rng.choices(new, k=nn))
        for _ in range(BOOTSTRAP_RESAMPLES)
    )
    return point, ratios[math.floor((1 - CONFIDENCE) * BOOTSTRAP_RESAMPLES)]


# ------------------------------------------------------------- formatting


def fmt_ns(ns: float) -> str:
    for unit, scale in (("s", 1e9), ("ms", 1e6), ("µs", 1e3)):
        if ns >= scale:
            return f"{ns / scale:.4g} {unit}"
    return f"{ns:.4g} ns"


def fmt_mbit(rate: float) -> str:
    return f"{rate:,.0f}" if rate >= 1000 else f"{rate:.4g}"


def fmt_rate(units: float, ns: float) -> str:
    return fmt_mbit(units / ns * 1e3)


def table(header: list[str], rows: list[list]) -> list[str]:
    out = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    out += ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]
    return out


def resolve_seed(revision: str) -> str:
    try:
        src = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "show", f"{revision}:crates/gf2-coding/src/test_support.rs"],
            capture_output=True, text=True, check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return f"not resolvable at revision `{revision}`"
    m = re.search(r"pub const BCH_CORPUS_SEED: u64 = ([0-9A-Fa-fx_]+);", src)
    return f"`{m.group(1)}` (`BCH_CORPUS_SEED` at `{revision}`)" if m else "constant not found"


# ----------------------------------------------------------------- render


def render(run_dir: Path, baseline_dir: Path, survey_dir: Path) -> tuple[list[str], bool]:
    host_file = run_dir / "host.txt"
    record_file = run_dir / "dispatch.jsonl"
    for p in (host_file, record_file, run_dir / "samples"):
        if not p.exists():
            raise InputError(f"missing {p}")
    facts = parse_host_facts(host_file)
    logs = {b: run_dir / "logs" / f"{b}-criterion.txt" for b in BENCH_LABELS}
    headers = {b: parse_log_header(p) if p.exists() else {} for b, p in logs.items()}
    new = load_criterion(run_dir / "samples")
    dispatch = load_dispatch(record_file)
    by_id = {r["id"]: r for r in dispatch}
    base = load_criterion(baseline_dir / "samples")
    survey, survey_files = load_survey(survey_dir)
    ok = True
    out: list[str] = []
    w = out.append

    w(f"# Receipt: BCH performance after the cutover (jit:{ISSUE})")
    w("")
    w("Rendered by `dev/active/fd9d5416/render_receipt.py` under "
      "`dev/active/fd9d5416/receipt-protocol.md`; every figure is computed at "
      "render time from the files listed under *Pinned inputs*.")
    w("")
    out += table(["Field", "Value"], [
        ["Issue", f"`{ISSUE}`"],
        ["gf2 revision", f"`{facts.get('revision', '')}`"],
        ["Tree state", facts.get("tree_state", "")],
        ["Host", facts.get("cpu_model", "")],
        ["CPU flags (kernel bundle)", facts.get("cpu_flags", "")],
        ["nproc", facts.get("nproc", "")],
        ["Governor (cpu0)", facts.get("governor", "")],
        ["Kernel", facts.get("uname", "")],
        ["Rust", facts.get("rustc", "")],
        ["Cargo", facts.get("cargo", "")],
        ["Measurement window (UTC)", f"{facts.get('start_utc', '')} to {facts.get('end_utc', '')}"],
        ["Load average at run start", facts.get("uptime", "")],
        ["Message seed (W1/W2)", resolve_seed(facts.get("revision", ""))],
        ["Bootstrap", f"{BOOTSTRAP_RESAMPLES} resamples, one-sided {CONFIDENCE:.0%}, seed `{BOOTSTRAP_SEED:#x}` per cell ID, acceptance >= {ACCEPT_RATIO}"],
    ])
    w("")

    w("## Invocations")
    w("")
    out += table(["Bench target", "Invocation", "Started (UTC)", "Load avg at start", "Finished (UTC)", "Load avg at end"], [
        [f"`{b}`", f"`{h.get('command', 'missing log')}`", h.get("started_utc", ""), h.get("load_avg_start", ""),
         h.get("finished_utc", ""), h.get("load_avg_end", "")]
        for b, h in headers.items()
    ])
    w("")

    # ------------------------------------------------ non-regression, tier A
    w("## REQ-01 non-regression against the pinned pre-cutover receipt (`88ca7d2f`)")
    w("")
    w("Cell mapping: `dev/active/591a1c5e/smoke-run.md`. Ratio is throughput "
      "new/baseline = median(baseline)/median(new) over per-iteration times; "
      "*Lower bound* is the one-sided lower 95% bootstrap bound.")
    w("")
    rows = []
    for cid in TIER_A_KEPT:
        b, n = base.get(cid), new.get(cid)
        if b is None or n is None:
            rows.append([f"`{cid}`", "missing" if n is None else fmt_ns(n["median_ns"]),
                         "missing" if b is None else fmt_ns(b["median_ns"]), "", "", "**FAIL** (missing)"])
            ok = False
            continue
        point, lower = lower_bound_ratio(b["samples"], n["samples"], cid)
        passed = lower >= ACCEPT_RATIO
        ok &= passed
        rows.append([f"`{cid}`", fmt_ns(n["median_ns"]), fmt_ns(b["median_ns"]), f"{point:.4f}", f"{lower:.4f}",
                     "pass" if passed else "**FAIL**"])
    out += table(["Cell (same ID both sides)", "New median", "Baseline median", "Ratio", "Lower bound", "Verdict"], rows)
    w("")
    w("Baseline cells outside the comparison, reported only:")
    w("")
    out += table(["Baseline ID", "New ID", "New median", "Reason"], [
        [f"`{bid}`" if bid else "—", f"`{nid}`" if nid else "—",
         fmt_ns(new[nid]["median_ns"]) if nid in new else ("missing" if nid else "—"), reason]
        for bid, nid, reason in TIER_A_EXCLUDED
    ])
    w("")

    # ------------------------------------------------ non-regression, tier B
    w("## REQ-01 non-regression against the survey's pre-cutover gf2 cells (`4e732b56`)")
    w("")
    w("Baseline: the survey's `gf2` rows (legacy `BchEncoder::encode_batch` and "
      "`generator_matrix` at the survey revision), per-call time from each trial. "
      "New: the `fresh-alloc`, `W1` Criterion cell of the same row and batch.")
    w("")
    rows = []
    for row in CONTRACT_ROWS:
        for batch in CONTRACT_BATCHES:
            sel = "route" if row in DVB_T2_ROWS else "selected"
            cands = [r for r in dispatch if r.get("workload") == "W1" and r.get("row") == row
                     and r.get("batch") == batch and r.get("workers") == 1
                     and r.get("cache") == "fresh-alloc" and r.get("selection") == sel]
            legacy = survey.get(("gf2", "W1", "encode-batch", row, batch))
            cid = cands[0]["id"] if len(cands) == 1 else None
            label = f"`{cid}`" if cid else f"W1 {row} B={batch} (no unique cell)"
            if legacy is None:
                rows.append([label, fmt_ns(new[cid]["median_ns"]) if cid in new else "missing",
                             "projection only", "", "", "excluded (no measured baseline)"])
                continue
            if cid is None or cid not in new:
                rows.append([label, "missing", fmt_ns(statistics.median(legacy["times"])), "", "", "**FAIL** (missing)"])
                ok &= not TIER_B_W1_IN_VERDICT
                continue
            rec = by_id[cid]
            if (rec["n"], rec["k"]) != (legacy["n"], legacy["k"]):
                raise InputError(f"{cid}: shape {(rec['n'], rec['k'])} differs from the survey's {(legacy['n'], legacy['k'])}")
            point, lower = lower_bound_ratio(legacy["times"], new[cid]["samples"], cid)
            passed = lower >= ACCEPT_RATIO
            if TIER_B_W1_IN_VERDICT:
                ok &= passed
            verdict = ("pass" if passed else "**FAIL**") if TIER_B_W1_IN_VERDICT else "reported"
            rows.append([label, fmt_ns(new[cid]["median_ns"]),
                         f"{fmt_ns(statistics.median(legacy['times']))} ({len(legacy['times'])} trials)",
                         f"{point:.4f}", f"{lower:.4f}", verdict])
    for row in CONTRACT_ROWS:
        cid = f"bch_genmatrix_w2/materialize/fresh-alloc/{row}"
        legacy = survey.get(("gf2", "W2", "generator-matrix", row, None))
        if legacy is None:
            rows.append([f"`{cid}`", fmt_ns(new[cid]["median_ns"]) if cid in new else "missing",
                         "projection only", "", "", "excluded (no measured baseline)"])
            continue
        if cid not in new:
            rows.append([f"`{cid}`", "missing", fmt_ns(statistics.median(legacy["times"])), "", "", "missing"])
            ok &= not TIER_B_W2_IN_VERDICT
            continue
        point, lower = lower_bound_ratio(legacy["times"], new[cid]["samples"], cid)
        passed = lower >= ACCEPT_RATIO
        if TIER_B_W2_IN_VERDICT:
            ok &= passed
        verdict = ("pass" if passed else "**FAIL**") if TIER_B_W2_IN_VERDICT else f"reported; excluded: {TIER_B_W2_REASON}"
        rows.append([f"`{cid}`", fmt_ns(new[cid]["median_ns"]),
                     f"{fmt_ns(statistics.median(legacy['times']))} ({len(legacy['times'])} trials)",
                     f"{point:.4f}", f"{lower:.4f}", verdict])
    out += table(["New cell", "New median", "Survey gf2 median", "Ratio", "Lower bound", "Verdict"], rows)
    w("")

    # ---------------------------------------------------------- determinism
    w("## Determinism across worker counts and paths")
    w("")
    w("One group per W1 row and batch and per W2 group and row, from "
      "`dispatch.jsonl`: every path of a group must record one output digest.")
    w("")
    groups: dict = {}
    for r in dispatch:
        if r.get("workload") not in ("W1", "W2", "W2-parity-check"):
            continue
        key = (r["id"].split("/")[0], r["row"], r.get("batch"))
        groups.setdefault(key, []).append(r)
    rows = []
    for (grp, row, batch), recs in sorted(groups.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2] or 0)):
        digests = sorted({r["output_fnv1a"] for r in recs})
        workers = sorted({r["workers"] for r in recs})
        pools = sorted({r["rayon_pool_width"] for r in recs})
        kernels = sorted({str(r.get("kernel")) for r in recs if r.get("kernel")})
        agree = len(digests) == 1
        narrow = any(r["workers"] > 1 and r["rayon_pool_width"] != r["workers"] for r in recs)
        ok &= agree and not narrow
        rows.append([f"`{grp}`", row, batch if batch is not None else "—", len(recs),
                     ", ".join(map(str, workers)), ", ".join(map(str, pools)), ", ".join(kernels) or "—",
                     ", ".join(f"`{d}`" for d in digests),
                     "agree" if agree and not narrow else "**DISAGREE**" if not agree else "**POOL**"])
    out += table(["Group", "Row", "B", "Paths", "Workers", "Pool width", "Kernels", "Digest", "Verdict"], rows)
    w("")

    # ------------------------------------------------------ scalar fallback
    w("## Scalar-fallback coverage")
    w("")
    detected = sorted({r["kernel"] for r in dispatch if r.get("kernel") and r["kernel"] != "scalar"})
    w(f"Detected kernel bundle: {', '.join(f'`{k}`' for k in detected) or 'none recorded'}. "
      "Each row pairs a family's `scalar` arm with its detected arm at one row and batch; "
      "the digest comparison is the determinism group above.")
    w("")
    arms: dict = {}
    for r in dispatch:
        if r.get("workload") == "W1" and r.get("selection") == "family" and r.get("kernel"):
            arms.setdefault((r["row"], r["batch"], r["family"]), {})[r["kernel"]] = r
    rows = []
    for (row, batch, family), by_kernel in sorted(arms.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2])):
        scalar = by_kernel.get("scalar")
        others = [k for k in by_kernel if k != "scalar"]
        same = scalar is not None and all(by_kernel[k]["output_fnv1a"] == scalar["output_fnv1a"] for k in others)
        measured = scalar is not None and scalar["id"] in new
        ok &= same and measured
        rows.append([row, batch, f"`{family}`", ", ".join(sorted(by_kernel)),
                     fmt_ns(new[scalar["id"]]["median_ns"]) if measured else "missing",
                     "covered" if same and measured else "**GAP**"])
    out += table(["Row", "B", "Family", "Arms", "Scalar-arm median", "Verdict"], rows)
    w("")

    # ------------------------------------------------- external comparison
    w("## REQ-02 external-baseline comparison (aspirational, `W = 1`)")
    w("")
    w("External medians are the committed survey trials (`4e732b56`, same host); "
      "gf2 is this run's Criterion median. W1 in information Mbit/s ($Bk/T$), W2 in "
      "matrix Mbit/s ($kn/T$). The aspirational target is gf2 at or above the "
      "strongest external cell; the outcome is reported either way.")
    w("")
    rows = []
    met = total = 0
    for row in CONTRACT_ROWS:
        for batch in CONTRACT_BATCHES:
            cands = [r for r in dispatch if r.get("workload") == "W1" and r.get("row") == row
                     and r.get("batch") == batch and r.get("workers") == 1
                     and r.get("cache") == "warm-reuse" and r["id"] in new]
            ext = {f"{lib} `{alg}`": survey[(lib, "W1", alg, row, batch)] for lib, alg in W1_EXTERNAL
                   if (lib, "W1", alg, row, batch) in survey}
            rows.append(external_row(f"W1 {row} B={batch}", cands, new, ext, row, batch))
            if rows[-1][-1] in ("met", "not met"):
                total += 1
                met += rows[-1][-1] == "met"
    for row in CONTRACT_ROWS:
        cid = f"bch_genmatrix_w2/materialize/fresh-alloc/{row}"
        cands = [by_id[cid]] if cid in by_id and cid in new else []
        ext = {f"{lib} `{alg}`": survey[(lib, "W2", alg, row, None)] for lib, alg in W2_EXTERNAL
               if (lib, "W2", alg, row, None) in survey}
        rows.append(external_row(f"W2 {row}", cands, new, ext, row, None))
        if rows[-1][-1] in ("met", "not met"):
            total += 1
            met += rows[-1][-1] == "met"
    out += table(["Cell", "gf2 path", "gf2 Mbit/s", "Strongest external", "External Mbit/s", "gf2/external", "Aspirational target"], rows)
    w("")
    w(f"Aspirational outcome: target met on {met} of {total} comparable cells.")
    w("")

    # ------------------------------------------------------------ all cells
    w("## Every measured cell")
    w("")
    rows = []
    for cid, c in sorted(new.items()):
        rec = by_id.get(cid)
        units = cell_units(rec)
        rows.append([f"`{cid}`", len(c["samples"]), c["mode"], f"{c['median_ns']:.2f}",
                     f"{c['ci_lower_ns']:.2f}", f"{c['ci_upper_ns']:.2f}",
                     fmt_rate(units, c["median_ns"]) if units else "—"])
    out += table(["Benchmark ID", "Samples", "Sampling", "Median (ns)", "95% CI lower (ns)", "95% CI upper (ns)", "M units/s"], rows)
    w("")
    missing = sorted(set(by_id) - set(new))
    if missing:
        ok = False
        w("Dispatch-record IDs without Criterion output: " + ", ".join(f"`{m}`" for m in missing))
        w("")

    # ------------------------------------------------------- pinned inputs
    w("## Pinned inputs")
    w("")
    files = [host_file, record_file] + [p for p in logs.values() if p.exists()]
    files += [f for c in new.values() for f in (c["dir"] / n for n in ("benchmark.json", "estimates.json", "sample.json"))]
    files += [f for cid in TIER_A_KEPT if cid in base
              for f in (base[cid]["dir"] / n for n in ("benchmark.json", "estimates.json", "sample.json"))]
    files += survey_files
    rows = []
    for f in files:
        try:
            label = f.resolve().relative_to(REPO_ROOT)
        except ValueError:
            label = f
        rows.append([f"`{label}`", f"`{sha256_of(f)}`", f.stat().st_size])
    out += table(["File", "SHA-256", "Bytes"], rows)
    w("")

    w("## Verdict")
    w("")
    w(f"REQ-01 checks (non-regression, determinism, scalar-fallback coverage): {'**pass**' if ok else '**FAIL**'}.")
    return out, ok


def cell_units(rec: dict | None) -> int | None:
    if rec is None:
        return None
    if rec.get("workload") == "W1":
        return rec["batch"] * rec["k"]
    if rec.get("workload") in ("W2", "W2-parity-check"):
        return rec["rows"] * rec["cols"]
    if rec.get("workload") == "baseline-comparable":
        return rec["batch"] * rec["k"]
    return None


def external_row(label, cands, new, ext, row, batch):
    if not cands:
        return [label, "missing", "", "", "", "", "missing"]
    best = min(cands, key=lambda r: new[r["id"]]["median_ns"])
    units = cell_units(best)
    gf2 = units / new[best["id"]]["median_ns"] * 1e3
    path = "/".join(best["id"].split("/")[1:2])
    if not ext:
        return [label, f"`{path}`", fmt_mbit(gf2), "none for this cell", "", "", "no external cell"]
    name, cell = max(ext.items(), key=lambda kv: kv[1]["units"] / statistics.median(kv[1]["times"]))
    if cell["units"] != units:
        raise InputError(f"{label}: external work {cell['units']} differs from gf2 work {units}")
    ext_rate = cell["units"] / statistics.median(cell["times"]) * 1e3
    ratio = gf2 / ext_rate
    return [label, f"`{path}`", fmt_mbit(gf2), name, fmt_mbit(ext_rate), f"{ratio:.3g}", "met" if ratio >= 1 else "not met"]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("run_dir", type=Path)
    ap.add_argument("--baseline-dir", type=Path, default=REPO_ROOT / "dev/bench_results/88ca7d2f")
    ap.add_argument("--survey-dir", type=Path, default=REPO_ROOT / "dev/bench_results/4e732b56")
    args = ap.parse_args()
    try:
        lines, ok = render(args.run_dir, args.baseline_dir, args.survey_dir)
    except (InputError, OSError, KeyError, ValueError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
