#!/usr/bin/env python3
"""Projects the survey's receipts into `dev/bench_results/26465e6c/tables.md`.

Every number comes from a committed receipt, the frozen addendum it snapshots
and the acceptance summary the current `benchmark-acceptance` wrote beside it.
Speedups, their intervals and the cell decisions are the acceptance tool's;
the script recomputes one interval per receipt bit-for-bit from the raw pairs
before it writes anything, so its resampling port is checked against the tool.
An overview gives each campaign's family accounting, the pilot's widest
relative half-width and the resolution and margins each confirmation froze.
Each receipt section then adds, under its own heading and with the method
printed in the output, the arm medians with bootstrap intervals, the fastest
measured arm per population-count workload and each arm's threshold,
alignment and bit-pattern contrasts as bootstrapped cross-cell ratios, the
cells grouped by decision, and the conversion-cost probes of whole-consumer
cells. Whether each confirmation estimate lies inside its pilot's interval and
the protocol-v1 confirmation as history follow. Workload rows carry the
workload key and cell rows the cell ID, so prose can cite a row.

Usage: summarize-receipts.py  (from the repository root)
"""

import collections
import hashlib
import json
import math
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import families  # noqa: E402

RESULTS = pathlib.Path(families.RESULTS)
OUTPUT = RESULTS / "tables.md"
V3 = [
    ("popcount", "pilot"),
    ("popcount", "confirmation"),
    ("and-popcnt", "pilot"),
    ("and-popcnt", "pilot-r2"),
    ("and-popcnt", "confirmation"),
]
V1_CONFIRMATION = RESULTS / "2026-09-08-26465e6c-popcount"
# v1 cell -> the v3 cell measuring the same size, offset, pattern and arm pair.
# The v1 63-word cell and 32 MiB streaming cell have no exact v3 counterpart.
V1_COUNTERPARTS = {
    "popcount-small-vs-nibble-lut": "popcount-w4-vs-nibble-lut",
    "popcount-boundary-vs-scalar-popcnt": "popcount-w8-vs-scalar-popcnt",
    "popcount-alignment-vs-compiler-count-ones": "popcount-w256-off24-vs-compiler-count-ones",
    "popcount-bitpattern-allones-vs-libpopcnt": "popcount-w64-ones-vs-libpopcnt",
    "popcount-bitpattern-allzero-vs-mula": "popcount-w64-zeros-vs-mula-avx2-harley-seal",
    "popcount-csa-boundary-at-vs-mula": "popcount-w64-vs-mula-avx2-harley-seal",
    "popcount-cache-resident-vs-libpopcnt": "popcount-w16384-vs-libpopcnt",
}
# (variant workload, reference workload, what the variant changes).
CONTRASTS = [
    ("w8", "w4", "64 B instead of 32 B: gf2's SIMD threshold"),
    ("w12", "w8", "96 B instead of 64 B: libpopcnt's AVX2 threshold"),
    ("w64", "w60", "512 B instead of 480 B: Mula's carry-save loop"),
    ("w128", "w64", "1 KiB instead of 512 B: libpopcnt's Harley-Seal loop"),
    ("w256-off24", "w256", "window 24 bytes past a vector boundary"),
    ("w64-ones", "w64", "every bit set"),
    ("w64-zeros", "w64", "every bit clear"),
]
RESAMPLES = 10000
DESCRIPTIVE_ALPHA = 0.05
PROBES = ("setup_ns", "pack_ns", "dispatch_ns")
DECISIONS = ("improved", "not-worse", "inconclusive", "regressed", None)
MASK = (1 << 64) - 1


class SplitMix64:
    """SplitMix64 [Steele2014], as tuning-campaign-support's abtest.rs implements it."""

    def __init__(self, seed):
        self.state = seed & MASK

    def next_u64(self):
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        return z ^ (z >> 31)


def rotate_left(value, bits):
    return ((value << bits) | (value >> (64 - bits))) & MASK


class Xoshiro256StarStar:
    """xoshiro256** [BlackmanVigna2021] seeded through SplitMix64, as abtest.rs."""

    def __init__(self, seed):
        mixer = SplitMix64(seed)
        self.state = [mixer.next_u64() for _ in range(4)]

    def next_u64(self):
        s = self.state
        result = (rotate_left((s[1] * 5) & MASK, 7) * 9) & MASK
        t = (s[1] << 17) & MASK
        s[2] ^= s[0]
        s[3] ^= s[1]
        s[1] ^= s[2]
        s[0] ^= s[3]
        s[2] ^= t
        s[3] = rotate_left(s[3], 45)
        return result

    def below(self, n):
        unit = (self.next_u64() >> 11) / float(1 << 53)
        return min(int(unit * n), n - 1)


def median(values):
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2 == 0:
        return (ordered[middle - 1] + ordered[middle]) / 2.0
    return ordered[middle]


def rank(quantile, count):
    """abtest.rs's zero-based nearest-rank index."""
    return min(max(math.ceil(quantile * count), 1), count) - 1


def percentile_interval(replicates, alpha):
    replicates.sort()
    count = len(replicates)
    return replicates[rank(alpha / 2, count)], replicates[rank(1 - alpha / 2, count)]


def label_seed(label):
    return int.from_bytes(hashlib.sha256(label.encode()).digest()[:8], "little")


def paired_speedup_interval(pairs, alpha, seed):
    """The acceptance tool's whole-pair bootstrap of the speedup of medians."""
    generator = Xoshiro256StarStar(seed)
    replicates = []
    for _ in range(RESAMPLES):
        draw = [pairs[generator.below(len(pairs))] for _ in pairs]
        replicates.append(median([b for b, _ in draw]) / median([c for _, c in draw]))
    estimate = median([b for b, _ in pairs]) / median([c for _, c in pairs])
    return (estimate, *percentile_interval(replicates, alpha))


def median_interval(values, label):
    """Median of independent executions with a 95% percentile bootstrap interval."""
    generator = Xoshiro256StarStar(label_seed(label))
    replicates = [median([values[generator.below(len(values))] for _ in values])
                  for _ in range(RESAMPLES)]
    return (median(values), *percentile_interval(replicates, DESCRIPTIVE_ALPHA))


def ratio_interval(numerator, denominator, label):
    """Ratio of two independent samples' medians, each resampled independently."""
    generator = Xoshiro256StarStar(label_seed(label))
    replicates = []
    for _ in range(RESAMPLES):
        top = median([numerator[generator.below(len(numerator))] for _ in numerator])
        bottom = median([denominator[generator.below(len(denominator))] for _ in denominator])
        replicates.append(top / bottom)
    return (median(numerator) / median(denominator),
            *percentile_interval(replicates, DESCRIPTIVE_ALPHA))


def load(directory):
    receipt_bytes = (directory / "receipt.json").read_bytes()
    receipt = json.loads(receipt_bytes)
    summary = json.loads((directory / "acceptance-summary.json").read_text(encoding="utf-8"))
    digest = hashlib.sha256(receipt_bytes).hexdigest()
    if summary["receipt_sha256"] != digest:
        sys.exit(f"{directory}: acceptance summary describes another receipt")
    return receipt, summary, digest


def self_check(directory, receipt, summary):
    """Reproduces the first measured cell's acceptance interval exactly."""
    verdicts = {entry["cell_id"]: entry for entry in summary["cells"]}
    for cell in receipt["cells"]:
        found = verdicts[cell["cell_id"]].get("interval")
        if not cell["pairs"] or not found:
            continue
        pairs = [(pair["baseline"]["ns_per_call"], pair["candidate"]["ns_per_call"])
                 for pair in cell["pairs"]]
        recomputed = paired_speedup_interval(pairs, found["alpha"], found["seed"])
        if recomputed != (found["estimate"], found["lower"], found["upper"]):
            sys.exit(f"{directory} {cell['cell_id']}: port gives {recomputed}, tool gives {found}")
        return cell["cell_id"]
    sys.exit(f"{directory}: no measured cell to check")


def ns(value):
    if value >= 1e6:
        return f"{value / 1e6:.3f} ms"
    if value >= 1e3:
        return f"{value / 1e3:.3f} us"
    return f"{value:.3f} ns"


def ns_interval(found):
    estimate, lower, upper = found
    return f"{ns(estimate)} [{ns(lower)}, {ns(upper)}]"


def ratio_text(found):
    estimate, lower, upper = found
    return f"{estimate:.4f} [{lower:.4f}, {upper:.4f}]"


def speedup_text(entry):
    found = entry.get("interval")
    if not found:
        return "—"
    return f"{found['estimate']:.4f} [{found['lower']:.4f}, {found['upper']:.4f}]"


def values(cell, side):
    return [pair[side]["ns_per_call"] for pair in cell["pairs"]]


def paths(cell, side):
    return sorted({pair[side]["selected_path"] for pair in cell["pairs"]})


def workload_key(family, cell_id):
    for design_id, key, _ in families.FAMILIES[family]["cells"]():
        if design_id == cell_id:
            return key
    sys.exit(f"{cell_id} is not in the {family} design")


def workload_text(family, key):
    if family == "popcount":
        words, offset, pattern, cache, seed, _ = families.POPCOUNT_WORKLOADS[key]
        where = f" +{offset * 8} B" if offset else ""
        data = (f"SplitMix64 seed {seed}" if pattern == "random"
                else pattern.replace("_", "-"))
        return f"{words} w = {words * 8} B{where}, {data}, {cache}"
    words, cache, seed, _ = families.AND_WORKLOADS[key]
    return (f"2 x {words} w = 2 x {words * 8} B, SplitMix64 seeds {seed}/"
            f"{seed + families.AND_RHS_SEED_OFFSET}, {cache}")


def widest_half_width(summary):
    """P-03's quantity, as make-addenda.py derives it from a pilot summary."""
    return max(max(entry["interval"]["estimate"] - entry["interval"]["lower"],
                   entry["interval"]["upper"] - entry["interval"]["estimate"])
               / entry["interval"]["estimate"]
               for entry in summary["cells"] if entry.get("interval"))


def tally(counts):
    return ", ".join(f"{count} {name}" for name, count in sorted(counts.items())) or "none"


def overview(out, loaded):
    """One row per v3 campaign: family accounting, resolution and margins, outcomes."""
    out.append("## Campaigns")
    out.append("")
    out.append(
        "One row per v3 receipt. m and the attempt alpha are the acceptance tool's: m sums the "
        "comparisons the family ledger reserves up to and including the campaign's own line, "
        "imported protocol-v1 reservations included (at least 1), and a confirmation's attempt t "
        "counts the ledger lines that reserve comparisons, giving the attempt alpha "
        "0.05/(t(t+1)). A pilot reserves nothing and is no attempt, so its correction is "
        "informational. Expected draws per tail are the resamples times the per-comparison alpha "
        "over two, which P-20 requires to reach twenty. A pilot's widest relative half-width is "
        "P-03's quantity over its intervals; a confirmation's frozen resolution comes from the "
        "addendum snapshot in its receipt and names the pilot receipt it was derived from; "
        "margins are the summary's."
    )
    out.append("")
    out.append("| Receipt | Family | Label | Verdict | Cells x pairs | Sessions | Attempt | m "
               "| Per-comparison alpha | Draws per tail | Widest relative half-width "
               "| Frozen resolution | Material gap / equivalence | Outcomes | Findings "
               "| Arm executable |")
    out.append("|---|---|---|---|---|---:|---|---:|---|---:|---|---|---|---|---|---|")
    for directory, receipt, summary, _ in loaded:
        family = summary["family"]
        alpha = 1 - family["per_comparison_confidence"]
        pilot = summary["label"] == "pilot"
        attempt = round((math.sqrt(1 + 4 * 0.05 / family["family_alpha"]) - 1) / 2)
        addendum = json.loads((directory / "inputs" / "family-addendum.json")
                              .read_text(encoding="utf-8"))["effect"]
        resolution = ("—" if addendum["measurement_resolution"] is None else
                      f"{addendum['measurement_resolution']} from "
                      f"`{pathlib.Path(addendum['resolution_evidence']['receipt']).parent.name}`")
        pairs = sorted({len(cell["pairs"]) for cell in receipt["cells"]})
        margins = summary["cells"][0]["margins"]
        findings = collections.Counter(f"{finding['rule']} {finding['severity']}"
                                       for finding in summary["findings"])
        digests = sorted({arm["executable_sha256"][:8] for arm in receipt["arms"].values()})
        out.append(
            f"| `{directory.name}` | `{family['family_id']}` | {summary['label']} "
            f"| **{summary['verdict']}** | {len(receipt['cells'])} x "
            f"{', '.join(str(count) for count in pairs)} | {summary['sessions']} "
            f"| {'—' if pilot else attempt} | {family['comparisons']} | {alpha:.4g} "
            f"| {family['bootstrap_resamples'] * alpha / 2:.2f} "
            f"| {f'{widest_half_width(summary):.4f}' if pilot else '—'} | {resolution} "
            f"| {margins['improvement']:.2f} / {margins['equivalence']:.2f} "
            f"| {tally(collections.Counter(entry['outcome'] for entry in summary['cells']))} "
            f"| {tally(findings)} | {', '.join(f'`{digest}…`' for digest in digests)} |"
        )
    out.append("")


def header(out, directory, receipt, summary, digest, checked):
    family = summary["family"]
    alpha = 1 - family["per_comparison_confidence"]
    host = receipt["host"]
    governors = ", ".join(sorted(set(host["governors"].values())))
    digests = sorted({arm["executable_sha256"] for arm in receipt["arms"].values()})
    out.append(f"## `{directory.name}`")
    out.append("")
    out.append(
        f"Campaign `{summary['campaign_id']}`, label `{summary['label']}`, verdict "
        f"**{summary['verdict']}**, qualifies {summary['qualifies']}, findings "
        f"{len(summary['findings'])}, {summary['sessions']} session(s). Receipt sha256 `{digest}`."
    )
    out.append(
        f"Family `{family['family_id']}`: m = {family['comparisons']}, attempt alpha "
        f"{family['family_alpha']:.6g}, corrected per-comparison alpha {alpha:.4g} (confidence "
        f"{family['per_comparison_confidence']:.6f}), {family['bootstrap_resamples']} resamples, "
        f"{family['bootstrap_resamples'] * alpha / 2:.2f} expected draws per tail."
    )
    out.append(
        f"Host `{host['cpu_model']}` (`{host['hostname']}`), kernel `{host['os_kernel']}`, governor "
        f"{governors}, SMT active {host['smt_active']}; toolchain `{receipt['toolchain']}`; every "
        f"arm runs executable sha256 {', '.join(f'`{d}`' for d in digests)}. The port reproduces "
        f"the tool's interval of `{checked}` exactly."
    )
    groups = {}
    for finding in summary["findings"]:
        groups.setdefault((finding["rule"], finding["severity"]), []).append(finding)
    for (rule, severity), found in sorted(groups.items()):
        cells = sorted({finding["cell"] for finding in found if finding["cell"]})
        messages = sorted({finding["message"] for finding in found})
        out.append(
            f"Findings {rule} {severity} x{len(found)} on {len(cells)} cell(s)"
            + (f" (`{'`, `'.join(cells)}`)" if len(cells) <= 2 else "")
            + ": " + "; ".join(messages) + "."
        )
    out.append("")


def cell_table(out, directory, receipt, summary, family, digest):
    verdicts = {entry["cell_id"]: entry for entry in summary["cells"]}
    out.append(f"### Cells: `{directory.name}`")
    out.append("")
    out.append(
        "Per cell: paired executions, each arm's median with its 95% descriptive interval, the "
        "tool's speedup with the corrected interval, its decision against the margins in "
        "§ Campaigns, the cell outcome and the flagged windows."
    )
    out.append("")
    out.append(
        "| Cell | Workload | Pairs | Baseline ns/call | Candidate ns/call | Speedup [corrected interval] "
        "| Decision | Outcome | Flagged |"
    )
    out.append("|---|---|---:|---|---|---|---|---|---:|")
    for cell in receipt["cells"]:
        entry = verdicts[cell["cell_id"]]
        key = workload_key(family, cell["cell_id"])
        decision = entry.get("decision") or "—"
        if not cell["pairs"]:
            out.append(f"| `{cell['cell_id']}` | {workload_text(family, key)} | 0 | — | — | — "
                       f"| {decision} | **{entry['outcome']}** | — |")
            continue
        base = median_interval(values(cell, "baseline"), f"{digest}:{cell['cell_id']}:baseline")
        cand = median_interval(values(cell, "candidate"), f"{digest}:{cell['cell_id']}:candidate")
        out.append(
            f"| `{cell['cell_id']}` | {workload_text(family, key)} | {len(cell['pairs'])} "
            f"| {ns_interval(base)} | {ns_interval(cand)} | {speedup_text(entry)} "
            f"| {decision} | **{entry['outcome']}** "
            f"| {entry['flagged_windows']}/{entry['total_windows']} |"
        )
    out.append("")
    grouped = {}
    for entry in summary["cells"]:
        grouped.setdefault(entry.get("decision"), []).append(f"`{entry['cell_id']}`")
    for decision in DECISIONS:
        if decision in grouped:
            out.append(f"- Decision {decision or 'none'} ({len(grouped[decision])}): "
                       + ", ".join(grouped[decision]) + ".")
    out.append("")


def popcount_matrix(out, directory, receipt, summary):
    verdicts = {entry["cell_id"]: entry for entry in summary["cells"]}
    candidates = families.POPCOUNT_CANDIDATES
    out.append(f"### Speedups by workload: `{directory.name}`")
    out.append("")
    out.append(
        "Speedup of each alternative over gf2's dispatcher with the corrected interval (above 1: "
        "the alternative is faster), and the dispatcher's observed route."
    )
    out.append("")
    out.append("| Row | Workload | Dispatcher route | " + " | ".join(candidates) + " |")
    out.append("|---|---|---|" + "---|" * len(candidates))
    by_cell = {cell["cell_id"]: cell for cell in receipt["cells"]}
    for key in families.POPCOUNT_WORKLOADS:
        routes, row = set(), []
        for candidate in candidates:
            cell = by_cell.get(f"popcount-{key}-vs-{candidate}")
            if cell is None:
                row.append("not declared: aligned loads")
                continue
            row.append(speedup_text(verdicts[cell["cell_id"]]))
            routes.update(path.split(":", 1)[1] for path in paths(cell, "baseline"))
        out.append(f"| `{key}` | {workload_text('popcount', key)} | {', '.join(sorted(routes))} | "
                   + " | ".join(row) + " |")
    out.append("")


def arm_samples(by_cell, key):
    """Each arm's executions on one workload; the dispatcher pools its cells."""
    samples = {families.POPCOUNT_BASELINE: []}
    for candidate in families.POPCOUNT_CANDIDATES:
        cell = by_cell.get(f"popcount-{key}-vs-{candidate}")
        if cell is None:
            continue
        samples[families.POPCOUNT_BASELINE].extend(values(cell, "baseline"))
        samples[candidate] = values(cell, "candidate")
    return samples


def workload_contrasts(out, directory, receipt, digest):
    by_cell = {cell["cell_id"]: cell for cell in receipt["cells"]}
    out.append(f"### Workload contrasts: `{directory.name}`")
    out.append("")
    out.append(
        "Each arm's median on the variant workload over its median on the reference workload "
        "(above 1: the variant takes longer), resampled independently; 95% descriptive "
        "intervals. Each arm's sample is as in § Fastest arm."
    )
    out.append("")
    arms = [families.POPCOUNT_BASELINE] + families.POPCOUNT_CANDIDATES
    out.append("| Variant / reference | Change | " + " | ".join(arms) + " |")
    out.append("|---|---|" + "---|" * len(arms))
    for variant, reference, change in CONTRASTS:
        top, bottom = arm_samples(by_cell, variant), arm_samples(by_cell, reference)
        row = [ratio_text(ratio_interval(top[arm], bottom[arm],
                                         f"{digest}:{variant}/{reference}:{arm}"))
               if arm in top and arm in bottom else "not declared" for arm in arms]
        out.append(f"| `{variant} / {reference}` | {change} | " + " | ".join(row) + " |")
    out.append("")


def fastest_arms(out, directory, receipt, digest):
    by_cell = {cell["cell_id"]: cell for cell in receipt["cells"]}
    out.append(f"### Fastest arm: `{directory.name}`")
    out.append("")
    out.append(
        "Fastest measured arm per workload. Each arm's sample is its executions in the workload's "
        "cells: an alternative's are the pairs of its one cell, the dispatcher's the pairs of "
        "every cell of the workload (§ Cells, column *Pairs*); n is the fastest arm's. Ratios "
        "divide the slower arm's median by the fastest arm's, resampled independently; GB/s is "
        "buffer bytes over the median time. Intervals are 95% and descriptive."
    )
    out.append("")
    out.append("| Row | Workload | Fastest arm | n | ns/call | GB/s | Runner-up "
               "| Runner-up / fastest | Dispatcher / fastest |")
    out.append("|---|---|---|---:|---|---|---|---|---|")
    for key in families.POPCOUNT_WORKLOADS:
        words = families.POPCOUNT_WORKLOADS[key][0]
        samples = arm_samples(by_cell, key)
        ranked = sorted(samples, key=lambda arm: median(samples[arm]))
        fastest, runner_up = ranked[0], ranked[1]
        found = median_interval(samples[fastest], f"{digest}:{key}:{fastest}:fastest")
        rate = (8 * words / found[0], 8 * words / found[2], 8 * words / found[1])
        second = ratio_interval(samples[runner_up], samples[fastest],
                                f"{digest}:{key}:{runner_up}/{fastest}")
        dispatcher = families.POPCOUNT_BASELINE
        versus = ("it is the dispatcher" if fastest == dispatcher else ratio_text(ratio_interval(
            samples[dispatcher], samples[fastest], f"{digest}:{key}:{dispatcher}/{fastest}")))
        out.append(
            f"| `{key}` | {workload_text('popcount', key)} | {fastest} | {len(samples[fastest])} "
            f"| {ns_interval(found)} | {rate[0]:.2f} [{rate[1]:.2f}, {rate[2]:.2f}] "
            f"| {runner_up} | {ratio_text(second)} | {versus} |"
        )
    out.append("")


def probe_table(out, directory, receipt, digest):
    rows = []
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            reported = [pair[side]["conversion"] for pair in cell["pairs"]]
            if not reported or any(entry is None for entry in reported):
                continue
            arm = cell[f"{side}_arm"]
            found = [median_interval([float(entry[probe]) for entry in reported],
                                     f"{digest}:{cell['cell_id']}:{side}:{probe}")
                     for probe in PROBES]
            rows.append(f"| `{cell['cell_id']}` | {arm} | {len(reported)} | "
                        + " | ".join(ns_interval(entry) for entry in found) + " |")
    if not rows:
        return
    out.append(f"### Conversion-cost probes: `{directory.name}`")
    out.append("")
    out.append(
        "Conversion-cost probes of whole-consumer cells: one value per execution, measured "
        "after its timed windows, which already contain these costs; medians over executions "
        "with 95% descriptive intervals, rounded to integer nanoseconds. `setup_ns` times the "
        "arm's one-time route resolution (single shot); `pack_ns` the mean of repeated temporary "
        "copies of bank 0's left operand, so cache-warm even in a streaming cell; `dispatch_ns` "
        "the mean of repeated two-selection calls of the two-pass route; 0 where the arm has "
        "no such step."
    )
    out.append("")
    out.append("| Cell | Arm | Executions | setup_ns | pack_ns | dispatch_ns |")
    out.append("|---|---|---:|---|---|---|")
    out.extend(rows)
    out.append("")


def selected_paths(out, directory, receipt):
    seen = {}
    for cell in receipt["cells"]:
        for side in ("baseline", "candidate"):
            seen.setdefault(cell[f"{side}_arm"], set()).update(paths(cell, side))
    out.append(f"### Selected paths: `{directory.name}`")
    out.append("")
    out.append("Observed selected paths per arm: " + "; ".join(
        f"{arm}: {', '.join(sorted(found))}" for arm, found in sorted(seen.items())) + ".")
    out.append("")


def pilot_agreement(out):
    """Whether each confirmation estimate lies inside its accepted pilot's interval."""
    out.append("## Pilot and confirmation agreement")
    out.append("")
    out.append("Each confirmation point estimate against the interval of the accepted pilot that "
               "froze its resolution, an independent earlier sample of the same cell.")
    out.append("")
    for family, pilot_mode in (("popcount", "pilot"), ("and-popcnt", "pilot-r2")):
        pilot_dir = RESULTS / f"v3-{family}-{pilot_mode}"
        confirmation_dir = RESULTS / f"v3-{family}-confirmation"
        if not (confirmation_dir / "acceptance-summary.json").exists():
            continue
        pilot = {entry["cell_id"]: entry["interval"] for entry in load(pilot_dir)[1]["cells"]}
        outside = []
        cells = load(confirmation_dir)[1]["cells"]
        for entry in cells:
            estimate = entry["interval"]["estimate"]
            bounds = pilot[entry["cell_id"]]
            if not bounds["lower"] <= estimate <= bounds["upper"]:
                outside.append(f"`{entry['cell_id']}` {estimate:.4f} outside "
                               f"[{bounds['lower']:.4f}, {bounds['upper']:.4f}]")
        out.append(f"- `{confirmation_dir.name}` against `{pilot_dir.name}`: "
                   f"{len(cells) - len(outside)} of {len(cells)} inside"
                   + (f"; {'; '.join(outside)}." if outside else "."))
    out.append("")


def v1_history(out, current):
    receipt, summary, digest = load(V1_CONFIRMATION)
    family = summary["family"]
    out.append(f"## History: protocol-v1 confirmation `{V1_CONFIRMATION.name}`")
    out.append("")
    out.append(
        f"Campaign `{summary['campaign_id']}`, verdict **{summary['verdict']}**, receipt sha256 "
        f"`{digest}`, m = {family['comparisons']}, per-comparison confidence "
        f"{family['per_comparison_confidence']:.6f}, measured through the superseded two-harness "
        f"arms and kept byte-for-byte; the v3 ledger imports its nine comparisons. The last "
        f"columns give the v3 cell with the same size, offset, pattern and arm pair from "
        f"`{current[0] if current else 'no v3 confirmation yet'}`."
    )
    out.append("")
    out.append("| v1 cell | Candidate | Pairs | v1 speedup [interval] | v1 outcome | v3 counterpart "
               "| v3 speedup [corrected interval] | v3 decision | v3 outcome |")
    out.append("|---|---|---:|---|---|---|---|---|---|")
    by_cell = {cell["cell_id"]: cell for cell in receipt["cells"]}
    later = {entry["cell_id"]: entry for entry in current[1]["cells"]} if current else {}
    for entry in summary["cells"]:
        counterpart = V1_COUNTERPARTS.get(entry["cell_id"])
        match = later.get(counterpart)
        cell = by_cell[entry["cell_id"]]
        out.append(
            f"| `{entry['cell_id']}` | {cell['candidate_arm']} | {len(cell['pairs'])} "
            f"| {speedup_text(entry)} | **{entry['outcome']}** "
            f"| {f'`{counterpart}`' if counterpart else 'none'} "
            f"| {speedup_text(match) if match else '—'} | {match['decision'] if match else '—'} "
            f"| {match['outcome'] if match else '—'} |"
        )
    out.append("")


def main():
    out = [
        "# Population-count survey receipts",
        "",
        "Generated by `dev/active/26465e6c/survey/summarize-receipts.py` from the receipts and "
        "acceptance summaries it names; regenerate rather than edit.",
        "",
        "## Method",
        "",
        "- A cell's speedup is the baseline median over the candidate median of the paired "
        "per-execution values; a per-execution value is the median nanoseconds per call over its "
        "five timed windows. Above 1 favours the candidate. The population-count baseline is gf2's "
        "dispatcher and the AND family's is gf2's fused kernel.",
        "- Speedup intervals are the acceptance tool's whole-pair percentile bootstrap (10000 "
        "resamples, nearest-rank quantiles) at the corrected per-comparison alpha in each "
        "heading [Efron1979] [EfronTibshirani1993].",
        "- Arm medians carry a 95% percentile bootstrap interval over the executions (10000 "
        "resamples). Cross-cell ratios resample each arm's executions independently and take the "
        "ratio of medians. Both are descriptive and not family-corrected.",
        "- Resampling uses xoshiro256** seeded through SplitMix64 [BlackmanVigna2021] "
        "[Steele2014], ported from tuning-campaign-support 0.1.0 `src/abtest.rs`; each descriptive "
        "seed is the first eight bytes, little-endian, of SHA-256 over `<receipt sha256>:<label>`.",
        "- Random workload words come from SplitMix64 [Steele2014] in the same `abtest.rs`, seeded "
        "per cell as listed (`Fixture::new` in the survey crate's `src/fixture.rs`); the AND "
        "family's right operand uses the left seed plus "
        f"{families.AND_RHS_SEED_OFFSET}. Constant patterns ignore the seed.",
        "- Each table has its own heading; a workload row is named by its workload key, a cell "
        "row by its cell ID and a contrast by `variant / reference`.",
        "",
    ]
    measured = {}
    for family, mode in V3:
        directory = RESULTS / f"v3-{family}-{mode}"
        if (directory / "acceptance-summary.json").exists():
            measured[directory] = load(directory)
    overview(out, [(directory, *found) for directory, found in measured.items()])
    current = None
    for family, mode in V3:
        directory = RESULTS / f"v3-{family}-{mode}"
        if directory not in measured:
            out.append(f"## `{directory.name}`\n\nNot yet measured.\n")
            continue
        receipt, summary, digest = measured[directory]
        header(out, directory, receipt, summary, digest, self_check(directory, receipt, summary))
        if family == "popcount":
            popcount_matrix(out, directory, receipt, summary)
            fastest_arms(out, directory, receipt, digest)
            workload_contrasts(out, directory, receipt, digest)
            if mode == "confirmation":
                current = (directory.name, summary)
        cell_table(out, directory, receipt, summary, family, digest)
        probe_table(out, directory, receipt, digest)
        selected_paths(out, directory, receipt)
    pilot_agreement(out)
    v1_history(out, current)
    OUTPUT.write_text("\n".join(out).rstrip() + "\n", encoding="utf-8")
    print(f"{OUTPUT}: written")


if __name__ == "__main__":
    main()
