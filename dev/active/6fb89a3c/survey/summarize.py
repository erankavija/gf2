#!/usr/bin/env python3
"""Project the v3 receipts and acceptance summaries into `tables.md`.

Usage: dev/active/6fb89a3c/survey/summarize.py  (from the repo root)

Every number in the tables is read from a committed receipt or from the
acceptance summary the independent evaluator wrote for it, or is computed here
from the receipts' raw per-execution values; nothing is transcribed. Speedups
are median(baseline)/median(candidate) with the candidate always the external
arm, so a value above 1 means the external arm is ahead of gf2.

Probe and derived statistics use a percentile bootstrap [Efron1979] over the
committed per-execution values: 10000 resamples, two-sided 95% nearest-rank
quantiles, index draws from SplitMix64 [Steele2014] implemented below and
seeded with 0x6FB89A3C for every statistic. They are descriptive and support
no adoption decision; only the protocol intervals of the cell tables come
from the evaluator.
"""
from __future__ import annotations

import collections
import json
import math
import pathlib
import statistics

RESAMPLES = 10000
SEED = 0x6FB89A3C
MASK = (1 << 64) - 1

RESULTS = pathlib.Path("dev/bench_results/6fb89a3c")
FAMILIES = [
    ("Transpose and conversion (`transpose-vs-external`)", "transpose"),
    ("Logical buffers (`logical-buffer-vs-isal`)", "logical"),
    ("BCH generator-matrix conversion (`bch-genmatrix-consumer`)", "bch"),
]

# Pilot executables whose probes the confirmation builds replaced; see
# dev/bench_results/6fb89a3c/v3-pilot-to-confirmation-executables.txt.
PILOT_BITSHUFFLE = "3435c78accaae5e62cbe1da603a64af96b3de3a03d7ba58c7949be2fb5f6e3e7"
PILOT_ISAL = "21bf3c88463b9c5b039ea9e32b8661b0051c84989bb032c4e953d839eb9a80cb"


def probe_fields(arm: str, digest: str) -> list[tuple[str, str]]:
    """(conversion field, what that arm measured in it) for every field it fills."""
    if arm in ("gf2-transpose", "m4ri-transpose"):
        return [("setup_ns", "input matrix construction")]
    if arm == "bitshuffle-transpose":
        copies = "one cold pass" if digest == PILOT_BITSHUFFLE else "mean of 100000 warm repetitions"
        return [("setup_ns", "input and adapter buffers"), ("pack_ns", f"adapter pack copy, {copies}"),
                ("unpack_ns", f"adapter unpack copy, {copies}")]
    if arm == "gf2-logical":
        return [("setup_ns", "aligned buffers"), ("pack_ns", "destination copy, mean of 10^6 repetitions")]
    if arm == "isal-base":
        if digest == PILOT_ISAL:
            return [("setup_ns", "aligned buffers")]
        return [("setup_ns", "aligned buffers"), ("dispatch_ns", "pointer-array formation, mean of 10^6 repetitions")]
    if arm == "gf2-bch-genmatrix":
        return [("setup_ns", "BCH code construction")]
    if arm == "m4ri-bch-genmatrix":
        return [("setup_ns", "mzd_init and shifted-polynomial fill"),
                ("pack_ns", "one mzd_copy (fresh allocation and copy), single pass after one warm call"),
                ("dispatch_ns", "one mzd_echelonize_m4ri of that copy, single pass (the reduction, not a dispatch)")]
    raise SystemExit(f"no probe definition for arm {arm}")


# Within-execution shares: (arm, numerator fields, label). The denominator is
# the same execution's ns per call.
SHARES = {
    "bitshuffle-transpose": (("pack_ns", "unpack_ns"), "adapter copies / Bitshuffle call"),
    "gf2-logical": (("pack_ns",), "destination copy / gf2 call"),
    "isal-base": (("dispatch_ns",), "pointer-array formation / ISA-L call"),
}

# Cross-cell ratios of per-execution medians: (label, side, numerator cell, denominator cell).
CROSS_CELL = {
    "transpose": [
        ("gf2 BitMatrix::transpose 65x65 / 64x64", "baseline", "transpose-consumer-65-vs-m4ri", "transpose-consumer-64-vs-m4ri"),
        ("gf2 BitMatrix::transpose 63x63 / 64x64", "baseline", "transpose-consumer-63-vs-m4ri", "transpose-consumer-64-vs-m4ri"),
        ("M4RI mzd_transpose consumer 65x65 / 64x64", "candidate", "transpose-consumer-65-vs-m4ri", "transpose-consumer-64-vs-m4ri"),
        ("M4RI mzd_transpose consumer 63x63 / 64x64", "candidate", "transpose-consumer-63-vs-m4ri", "transpose-consumer-64-vs-m4ri"),
        ("gf2 BitMatrix::transpose 64x64 / gf2 64x64 kernel", "baseline", "transpose-consumer-64-vs-m4ri", "transpose-64-canonical-vs-m4ri"),
        ("gf2 BitMatrix::transpose 65x65 / gf2 64x64 kernel", "baseline", "transpose-consumer-65-vs-m4ri", "transpose-64-canonical-vs-m4ri"),
    ],
    "logical": [
        ("gf2 xor 8 words / 7 words (scalar to SIMD dispatch)", "baseline", "logical-xor-8w-vs-isal-base", "logical-xor-7w-vs-isal-base"),
        ("gf2 xor 9 words / 8 words", "baseline", "logical-xor-9w-vs-isal-base", "logical-xor-8w-vs-isal-base"),
        ("gf2 xor 63 words / 64 words", "baseline", "logical-xor-63w-vs-isal-base", "logical-xor-64w-vs-isal-base"),
        ("gf2 xor 65 words / 64 words", "baseline", "logical-xor-65w-vs-isal-base", "logical-xor-64w-vs-isal-base"),
        ("gf2 three-source parity / two-source xor, 64 words", "baseline", "logical-parity-3src-64w-vs-isal-base", "logical-xor-64w-vs-isal-base"),
        ("ISA-L xor 8 words / 7 words", "candidate", "logical-xor-8w-vs-isal-base", "logical-xor-7w-vs-isal-base"),
        ("ISA-L xor 65 words / 64 words", "candidate", "logical-xor-65w-vs-isal-base", "logical-xor-64w-vs-isal-base"),
        ("ISA-L three-source parity / two-source xor, 64 words", "candidate", "logical-parity-3src-64w-vs-isal-base", "logical-xor-64w-vs-isal-base"),
    ],
}


class SplitMix64:
    def __init__(self, seed: int):
        self.state = seed & MASK

    def below(self, n: int) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return (((z ^ (z >> 31)) & MASK) * n) >> 64


def resample_median(values: list[float], rng: SplitMix64) -> float:
    n = len(values)
    return statistics.median([values[rng.below(n)] for _ in range(n)])


def interval(replicates: list[float]) -> tuple[float, float]:
    replicates.sort()
    return replicates[math.ceil(0.025 * RESAMPLES) - 1], replicates[math.ceil(0.975 * RESAMPLES) - 1]


def median_interval(values: list[float]) -> tuple[float, float, float, int]:
    """(median, lower, upper, n): percentile bootstrap of one sample's median."""
    rng = SplitMix64(SEED)
    lower, upper = interval([resample_median(values, rng) for _ in range(RESAMPLES)])
    return statistics.median(values), lower, upper, len(values)


def ratio_interval(numerator: list[float], denominator: list[float]) -> tuple[float, float, float]:
    """Ratio of medians of two independent samples, each resampled on its own."""
    rng = SplitMix64(SEED)
    replicates = [resample_median(numerator, rng) / resample_median(denominator, rng) for _ in range(RESAMPLES)]
    lower, upper = interval(replicates)
    return statistics.median(numerator) / statistics.median(denominator), lower, upper


def fmt(value: float) -> str:
    return f"{value:.4g}"


def fmt_interval(triple) -> str:
    return f"{fmt(triple[0])} [{fmt(triple[1])}, {fmt(triple[2])}]"


def render(directory: pathlib.Path, family: str) -> list[str]:
    receipt = json.loads((directory / "receipt.json").read_text())
    summary = json.loads((directory / "acceptance-summary.json").read_text())
    verdicts = {cell["cell_id"]: cell for cell in summary["cells"]}
    fam = summary.get("family") or {}
    lines = [
        f"Source: `{directory.relative_to(RESULTS)}/receipt.json` (SHA-256 `{summary['receipt_sha256']}`) and its acceptance summary.",
        f"Label **{summary['label']}**; verdict **{summary['verdict']}**; qualifies {summary['qualifies']}; sessions {summary['sessions']}; "
        f"ledger comparisons m = {fam.get('comparisons')}; attempt alpha {fam.get('family_alpha')}; "
        f"per-comparison confidence {fam.get('per_comparison_confidence')}; findings {len(summary['findings'])}.",
        "",
        "**Cells.**",
        "",
        "| Cell | Role | Pairs | Flagged | gf2 median ns/call [95%] | External median ns/call [95%] | Speedup [interval] | Decision | Outcome | Note | gf2 path | External path |",
        "|---|---|---:|---:|---:|---:|---|---|---|---|---|---|",
    ]
    cell_findings: dict[str, list[str]] = {}
    for finding in summary["findings"]:
        cell_findings.setdefault(finding.get("cell") or "", []).append(f"{finding['rule']} {finding['severity']}")
    for cell in receipt["cells"]:
        verdict = verdicts[cell["cell_id"]]
        pairs = cell["pairs"]
        ci = verdict.get("interval")
        speedup = f"{fmt(ci['estimate'])} [{fmt(ci['lower'])}, {fmt(ci['upper'])}] at {ci['confidence']:.4g}" if ci else "-"
        note = ("; ".join(filter(None, [verdict.get("reason"), "; ".join(verdict.get("unresolved_settings") or []),
                                        "; ".join(cell_findings.get(cell["cell_id"], []))])) or "-")
        paths = {side: ", ".join(sorted({p[side]["selected_path"] for p in pairs})) for side in ("baseline", "candidate")}
        lines.append(
            f"| `{cell['cell_id']}` | {cell['role']} | {len(pairs)} | {verdict['flagged_windows']}/{verdict['total_windows']} | "
            f"{fmt_interval(median_interval([p['baseline']['ns_per_call'] for p in pairs]))} | "
            f"{fmt_interval(median_interval([p['candidate']['ns_per_call'] for p in pairs]))} | {speedup} | "
            f"{verdict.get('decision') or '-'} | **{verdict['outcome']}** | {note} | {paths['baseline']} | {paths['candidate']} |")

    probe_rows, share_rows = [], []
    notes = []
    if any(arm["executable_sha256"] == PILOT_ISAL for arm in receipt["arms"].values()):
        notes.append("This ISA-L build's `dispatch_ns` is omitted: GCC deleted its probe loop, so the 0 ns it reports records "
                     "no measurement.")
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            arm = cell[f"{side}_arm"]
            records = [p[side] for p in cell["pairs"] if p[side].get("conversion")]
            if not records:
                continue
            digest = receipt["arms"][arm]["executable_sha256"]
            for field, meaning in probe_fields(arm, digest):
                values = [r["conversion"][field] for r in records]
                if arm == "bitshuffle-transpose" and field != "setup_ns" and not any(values):
                    continue  # the layouts coincide, so the adapter makes no copy
                probe_rows.append(f"| `{cell['cell_id']}` | {arm} | {meaning} (`{field}`) | {len(values)} | {fmt_interval(median_interval(values))} |")
            share = SHARES.get(arm)
            # A one-cold-pass probe is not comparable with a warm call.
            if share and digest != PILOT_BITSHUFFLE and all(field in dict(probe_fields(arm, digest)) for field in share[0]):
                ratios = [sum(r["conversion"][f] for f in share[0]) / r["ns_per_call"] for r in records]
                if any(ratios):
                    share_rows.append(f"| `{cell['cell_id']}` | {share[1]} | {len(ratios)} | {fmt_interval(median_interval(ratios))} |")
    if probe_rows:
        lines += ["", "**Probes.** Probes outside the timed windows, in nanoseconds (whole-ns integers as the arms report them): median over "
                  "the cell's executions with its bootstrap 95% interval. The work each probe isolates is also inside every "
                  "timed call. Bitshuffle adapter copies are skipped, and not probed, where the layouts coincide (64x64). "
                  "Single-pass probes include timer overhead and first-use effects and can exceed the warm per-call median, "
                  "so they are not shares of the timed call. "
                  + " ".join(notes), "", "| Cell | Arm | Probe (field) | n | Median [95% interval] |", "|---|---|---|---:|---|"] + probe_rows
    if share_rows:
        lines += ["", "**Shares.** Within-execution shares: the probe divided by the same execution's ns per call, median over executions "
                  "with its bootstrap 95% interval. Integer-ns probes of a few nanoseconds carry up to half a nanosecond of "
                  "rounding in the numerator.", "", "| Cell | Share | n | Median [95% interval] |", "|---|---|---:|---|"] + share_rows
    cross = [row for row in CROSS_CELL.get(family, [])]
    if cross:
        cells = {cell["cell_id"]: cell for cell in receipt["cells"]}
        lines += ["", "**Cross-cell ratios.** Ratios of medians: per-execution ns per call of one arm in two cells, each cell's executions "
                  "resampled independently (the cells are not paired), bootstrap 95% interval.", "",
                  "| Ratio | n (numerator, denominator) | Ratio [95% interval] |", "|---|---|---|"]
        for label, side, top, bottom in cross:
            numerator = [p[side]["ns_per_call"] for p in cells[top]["pairs"]]
            denominator = [p[side]["ns_per_call"] for p in cells[bottom]["pairs"]]
            lines.append(f"| {label} | {len(numerator)}, {len(denominator)} | {fmt_interval(ratio_interval(numerator, denominator))} |")
    if summary["findings"]:
        lines += ["", "**Acceptance findings.**", ""]
    for finding in summary["findings"]:
        lines.append(f"- `{finding['rule']}` ({finding['severity']}) {finding.get('cell') or ''}: {finding['message']}")
    return lines


def overview() -> list[str]:
    """Per-family confirmation outcomes, resolution derivation and arm identity."""
    outcomes, resolutions, arms = [], [], []
    for _, family in FAMILIES:
        for directory in sorted(RESULTS.glob(f"v3-*-6fb89a3c-{family}-confirmation")):
            receipt = json.loads((directory / "receipt.json").read_text())
            summary = json.loads((directory / "acceptance-summary.json").read_text())
            fam = summary["family"]
            draws = fam["bootstrap_resamples"] * (1.0 - fam["per_comparison_confidence"]) / 2
            counts = collections.Counter(cell["outcome"] for cell in summary["cells"])
            leads = [f"`{cell['cell_id']}` ({cell['decision']})" for cell in summary["cells"]
                     if cell.get("interval") and cell["interval"]["estimate"] > 1]
            outcomes.append(
                f"| `{fam['family_id']}` | `{directory.name}` | {fam['comparisons']} | {fam['per_comparison_confidence']:.6g} | "
                f"{draws:.1f} | {', '.join(f'{outcome} x{count}' for outcome, count in sorted(counts.items()))} | "
                f"{'; '.join(leads) or 'none'} |")
            effect = json.loads((directory / receipt["addendum"]["snapshot"]).read_text())["effect"]
            pilot = pathlib.Path(effect["resolution_evidence"]["receipt"]).parent
            pilot_summary = json.loads((pilot / "acceptance-summary.json").read_text())
            widths = {cell["cell_id"]: max(abs(cell["interval"]["estimate"] - cell["interval"]["lower"]),
                                           abs(cell["interval"]["upper"] - cell["interval"]["estimate"])) / cell["interval"]["estimate"]
                      for cell in pilot_summary["cells"] if cell.get("interval")}
            widest = max(widths, key=widths.get)
            resolutions.append(
                f"| `{fam['family_id']}` | `{pilot.name}` | {1.0 - pilot_summary['family']['per_comparison_confidence']:.4g} | "
                f"{widths[widest]:.4f} (`{widest}`) | {effect['measurement_resolution']} | {effect['material_gap_threshold']} | "
                f"{effect['equivalence_margin']} |")
            pilot_arms = json.loads((pilot / "receipt.json").read_text())["arms"]
            for arm, described in receipt["arms"].items():
                before = pilot_arms.get(arm, {}).get("executable_sha256")
                after = described["executable_sha256"]
                arms.append(f"| `{fam['family_id']}` | {arm} | `{before}` | `{after}` | {'yes' if before == after else 'no'} |")
    return [
        "## Overview",
        "",
        "**Confirmation outcomes.** Per family: the ledger comparison count $m$, the per-comparison confidence, the expected "
        "draws in each bootstrap tail at that confidence (P-20 requires twenty), the evaluator's outcome counts, and the cells "
        "whose speedup estimate exceeds 1 (the external arm ahead) with their decisions.",
        "",
        "| Family | Confirmation | $m$ | Confidence | Tail draws | Outcomes | External arm ahead |",
        "|---|---|---:|---:|---:|---|---|",
        *outcomes,
        "",
        "**Resolution.** The widest relative half-width of the resolution pilot's intervals (as P-03 recomputes it) and the "
        "settings the confirmation addendum froze from it.",
        "",
        "| Family | Resolution pilot | Pilot alpha | Widest relative half-width | Frozen resolution | Material gap | Equivalence |",
        "|---|---|---:|---:|---:|---:|---:|",
        *resolutions,
        "",
        "**Arm executables.** Each arm's executable digest in the resolution pilot and in the confirmation receipt.",
        "",
        "| Family | Arm | Pilot SHA-256 | Confirmation SHA-256 | Same bytes |",
        "|---|---|---|---|---|",
        *arms,
        "",
    ]


def main() -> None:
    out = [
        "# Comparator survey evidence tables",
        "",
        "> **Diátaxis Type:** Reference",
        "",
        "Generated by `dev/active/6fb89a3c/survey/summarize.py` from the committed protocol-v3 receipts and their independently "
        "recomputed acceptance summaries. Speedup is median(gf2)/median(external): above 1 the external arm is ahead. In a "
        "comparator-gap cell `improved` means the external arm holds a material gap that needs attribution, `regressed` means "
        "gf2 is materially ahead and `not-worse` means neither. The speedup intervals are the evaluator's paired intervals at the "
        "family's per-comparison confidence. Per-arm median, probe, share and cross-cell intervals are percentile bootstraps "
        "[Efron1979] of the committed per-execution values (10000 resamples, 95% nearest-rank quantiles, SplitMix64 "
        "[Steele2014] index draws implemented in the generator, seed 0x6FB89A3C); they are descriptive.",
        "",
        "**Withdrawn.** The gf2 arms of every receipt tabulated here declared `warm` without the protocol's untimed pass "
        "over the working set before calibration, while the external arms made one, so every comparison of gf2 with an "
        "external arm in these tables is withdrawn ([findings](../../active/6fb89a3c/findings.md) §4 and §5). The "
        "external arms' own probes and shares do not depend on the gf2 arms.",
        "",
    ] + overview()
    for title, family in FAMILIES:
        out += [f"## {title}", ""]
        for mode in ("pilot", "confirmation"):
            for directory in sorted(RESULTS.glob(f"v3-*-6fb89a3c-{family}-{mode}")):
                if (directory / "acceptance-summary.json").exists():
                    out += [f"### {mode} `{directory.name}`", ""] + render(directory, family) + [""]
    (RESULTS / "tables.md").write_text("\n".join(out))
    print(f"-> {RESULTS / 'tables.md'}")


if __name__ == "__main__":
    main()
