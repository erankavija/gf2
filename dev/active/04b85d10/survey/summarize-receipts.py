#!/usr/bin/env python3
"""Projects the receipts of the bit-storage survey into one Markdown table set.

Reads every finalized receipt directory named on the command line together
with its acceptance summary and prints, per receipt, one row per cell: the
arms and the routes they observed themselves taking, the fixture seed and the
pair-order and resampling seed beside their generators, the per-arm median of
the per-execution medians in nanoseconds per call with its order-statistic
interval, the acceptance tool's speedup of medians with its interval at the
corrected per-comparison confidence, the decision and outcome, the
flagged-window count, the baseline arm's reported setup and conversion probes
as medians over its executions with the same order-statistic interval, and the
contradictions the evidence record carries for that cell.

Every figure comes from the receipt, its snapshots or the summary it sits
beside. Every statement about how a seed is derived, what a conversion probe
times and where a receipt's declared workload and its measurement disagree is
projected from the evidence record, which this tool names by path and digest.

Usage: summarize-receipts.py [<receipt-dir>...]

With no argument it projects every receipt the evidence record describes, so
the tables regenerate from committed inputs without a hand-typed list.
"""

import hashlib
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from intervals import median_interval, median_rank  # noqa: E402

HARNESS_LOCK = "inputs/producing/dev/active/04b85d10/survey/gf2-side/Cargo.lock"
ABTEST = "dev/tools/tuning-campaign-support/src/abtest.rs"
RECORD = "dev/bench_results/04b85d10/receipt-evidence-record.json"
GENERATOR = "dev/active/04b85d10/survey/summarize-receipts.py"
MEDIAN_LEVEL = 0.95


def ns(value):
    if value >= 1e6:
        return f"{value / 1e6:.3f} ms"
    if value >= 1e3:
        return f"{value / 1e3:.2f} us"
    return f"{value:.1f} ns"


def ns_interval(values):
    median, lower, upper, _ = median_interval(values, MEDIAN_LEVEL)
    return f"{ns(median)} [{ns(lower)}, {ns(upper)}]"


def conversion_text(conversions):
    """Median and interval of each probe an arm reported in every execution."""
    if not conversions or not all(conversions):
        return "none reported"
    parts = []
    for key in ("setup_ns", "pack_ns", "unpack_ns", "batch_fill_ns", "dispatch_ns"):
        values = [conversion.get(key, 0) for conversion in conversions]
        if max(values):
            parts.append(f"{key[:-3]} {ns_interval(values)}")
    return ", ".join(parts) if parts else "all phases 0"


def locked_versions(directory):
    """Versions of the fixture generator's crates in the receipt's own lockfile."""
    text = (directory / HARNESS_LOCK).read_text(encoding="utf-8")
    versions = {}
    for block in text.split("[[package]]"):
        lines = [line.strip() for line in block.splitlines()]
        for name in ("rand", "rand_chacha", "rand_core"):
            if f'name = "{name}"' in lines:
                version = next(line for line in lines if line.startswith("version = "))
                versions.setdefault(name, []).append(version.split('"')[1])
    for name in ("rand", "rand_chacha", "rand_core"):
        if len(versions.get(name, [])) != 1:
            sys.exit(f"{directory / HARNESS_LOCK} resolves {name} to {versions.get(name)}")
    return {name: found[0] for name, found in versions.items()}


def load_record():
    """The evidence record this projection cites, with its digest."""
    path = pathlib.Path(RECORD)
    raw = path.read_bytes()
    return json.loads(raw.decode("utf-8")), hashlib.sha256(raw).hexdigest()


def load_receipt(directory):
    receipt = json.loads((directory / "receipt.json").read_text(encoding="utf-8"))
    summary = json.loads((directory / "acceptance-summary.json").read_text(encoding="utf-8"))
    addendum = json.loads(
        (directory / "inputs" / "family-addendum.json").read_text(encoding="utf-8")
    )
    return receipt, summary, addendum


def seed_paragraph(record):
    parts = []
    for entry in record["seed_derivation"]:
        cites = ", ".join(f"`{cite}`" for cite in entry["cites"])
        sentence = (
            f"The {entry['seed']} seed is {entry['source']}: {entry['derivation']}. The "
            f"generator is {entry['algorithm']}, pinned by {entry['pinned_by']}. Its "
            f"derivation is {cites}."
        )
        if entry["unseeded_workloads"]:
            workloads = ", ".join(f"`{w}`" for w in entry["unseeded_workloads"])
            sentence += f" A seed marked unused belongs to {entry['unseeded_note']}: {workloads}."
        parts.append(sentence)
    return " ".join(parts)


def probe_paragraph(record):
    parts = []
    for entry in record["probe_semantics"]:
        scope = (
            ", ".join(f"`{w}`" for w in entry["workloads"])
            if entry["workloads"]
            else "every workload"
        )
        parts.append(f"`{entry['probe']}` of {scope} times {entry['times']}")
    return "; ".join(parts) + "."


def unseeded_workloads(record):
    for entry in record["seed_derivation"]:
        if entry["seed"] == "fixture":
            return set(entry["unseeded_workloads"])
    return set()


def contradiction_rows(record):
    for entry in record["contradictions"]:
        yield entry


def main():
    record, record_sha256 = load_record()
    described = list(record["receipts"])
    dirs = [pathlib.Path(arg) for arg in sys.argv[1:]] or [
        pathlib.Path(path) for path in described
    ]
    for directory in dirs:
        if str(directory) not in described:
            sys.exit(f"{directory} is not a receipt the evidence record {RECORD} describes")
    loaded = [(directory, *load_receipt(directory)) for directory in dirs]
    pair_counts = sorted(
        {len(cell.get("pairs") or []) for _, receipt, _, _ in loaded for cell in receipt["cells"]}
        - {0}
    )
    ranks = []
    for count in pair_counts:
        rank, coverage = median_rank(count, MEDIAN_LEVEL)
        ranks.append(f"[x({rank}), x({count + 1 - rank})] at n = {count}, coverage {coverage:.1%}")

    print("# Receipts of the bit-storage consumer survey")
    print()
    print(
        f"Generated by `{GENERATOR}` from the receipts, their input snapshots and the "
        f"acceptance summaries it names, and from the evidence record `{RECORD}` SHA-256 "
        f"`{record_sha256}`, which carries every statement below about seed derivation, "
        "probe semantics and recorded contradictions. Speedups are baseline median over "
        "candidate median; values above one favour the candidate. Speedup intervals are the "
        "acceptance tool's whole-pair percentile bootstrap at the corrected per-comparison "
        "confidence shown in each receipt heading. Per-arm medians are the median over the n "
        "pairs of each execution's median nanoseconds per call, with the distribution-free "
        f"order-statistic interval for a median ({'; '.join(ranks)}; "
        "`dev/active/04b85d10/survey/intervals.py`). Conversion probes are the baseline arm's "
        "reports, one per execution, summarized the same way."
    )
    print()
    print(f"Seeds and generators. {seed_paragraph(record)}")
    print()
    print(f"What a receipt's conversion probes time: {probe_paragraph(record)}")
    print()
    print(
        "Recorded contradictions. Each entry names a receipt cell whose measurement differs "
        "from what its addendum declares. The receipts and addenda keep their bytes; the "
        "affected rows below name the entry, and the entry states what the cell falsifies and "
        "what it still supports."
    )
    print()
    for entry in contradiction_rows(record):
        cells = ", ".join(f"`{cell}`" for cell in entry["cells"])
        declared_in = ", ".join(f"`{path}`" for path in entry["declared_in"])
        evidence = ", ".join(f"`{path}`" for path in entry["evidence"])
        replacement = f"`{entry['replacement']}`" if entry["replacement"] else "none"
        print(
            f"- **`{entry['id']}`** ({entry['class']}, {entry['status']}). Declared in "
            f"{declared_in}: {entry['declared']}. Measured: {entry['measured']}. Cells: {cells}. "
            f"Withdrawn: {entry['withdrawn']}. Stands: {entry['stands']}. Evidence: {evidence}. "
            f"Replacement: {replacement}."
        )

    unseeded = unseeded_workloads(record)
    for directory, receipt, summary, addendum in loaded:
        workloads = {cell["cell_id"]: cell["workload"] for cell in addendum["cells"]}
        versions = locked_versions(directory)
        abtest = receipt["source"]["producing"]["behavior_sha256"][ABTEST]
        family = summary["family"]
        marks = {}
        for entry in contradiction_rows(record):
            if str(directory) not in entry["receipts"]:
                continue
            for cell_id in entry["cells"]:
                marks.setdefault(cell_id, []).append(entry["id"])
        print()
        print(f"## `{directory.name}`")
        print()
        print(
            f"Campaign `{receipt['campaign_id']}`, label `{receipt['label']}`, verdict "
            f"**{summary['verdict']}**, qualifies {summary['qualifies']}, findings "
            f"{len(summary['findings'])}, receipt digest `{summary['receipt_sha256']}`. "
            f"Family `{family['family_id']}`: {family['comparisons']} comparisons, attempt "
            f"alpha {family['family_alpha']}, per-comparison confidence "
            f"{family['per_comparison_confidence']:.6f}. Protocol version "
            f"{addendum['protocol']['version']}. Toolchain `{receipt['toolchain']}`. "
            f"Campaign seed {receipt['campaign_seed']}; fixture generator rand "
            f"{versions['rand']} StdRng (rand_chacha {versions['rand_chacha']}, rand_core "
            f"{versions['rand_core']}); order and bootstrap generator xoshiro256** of "
            f"`{ABTEST}` SHA-256 `{abtest[:16]}`."
        )
        arms = receipt["arms"]
        digests = {arm["executable_sha256"] for arm in arms.values()}
        print(
            f"Arm executables: {len(arms)} arms over {len(digests)} distinct executable "
            f"digest(s) `{', '.join(sorted(d[:16] for d in digests))}`."
        )
        print()
        print("| Cell | Baseline arm (observed route) | Candidate arm (observed route) | Fixture seed | Order and bootstrap seed | Pairs | Baseline median [interval] | Candidate median [interval] | Speedup | Interval | Decision | Outcome | Flagged | Baseline conversion probes, median [interval] | Recorded contradictions |")
        print("|---|---|---|---|---:|---:|---:|---:|---:|---|---|---|---:|---|---|")
        cells_by_id = {cell["cell_id"]: cell for cell in summary["cells"]}
        for cell in receipt["cells"]:
            accepted = cells_by_id[cell["cell_id"]]
            workload = workloads[cell["cell_id"]]
            if workload["identity"] in unseeded:
                fixture = f"{workload['seed']} (unused)"
            else:
                fixture = f"{workload['seed']} (rand {versions['rand']} StdRng)"
            recorded = marks.get(cell["cell_id"])
            recorded = ", ".join(f"`{name}`" for name in recorded) if recorded else "none"
            pairs = cell.get("pairs") or []
            if not pairs:
                print(
                    f"| `{cell['cell_id']}` | {cell['baseline_arm']} | {cell['candidate_arm']} | "
                    f"{fixture} | n/a | 0 | unavailable | unavailable | n/a | n/a | n/a | "
                    f"{accepted['outcome']} | n/a | {cell.get('unavailable_reason') or ''} | "
                    f"{recorded} |"
                )
                continue
            base = [p["baseline"]["ns_per_call"] for p in pairs]
            cand = [p["candidate"]["ns_per_call"] for p in pairs]
            base_paths = sorted({p["baseline"]["selected_path"] for p in pairs})
            cand_paths = sorted({p["candidate"]["selected_path"] for p in pairs})
            interval = accepted["interval"]
            decision = accepted["decision"] or "n/a"
            print(
                f"| `{cell['cell_id']}` | {cell['baseline_arm']} ({', '.join(base_paths)}) | "
                f"{cell['candidate_arm']} ({', '.join(cand_paths)}) | {fixture} | "
                f"{interval['seed']} (xoshiro256**) | {len(pairs)} | "
                f"{ns_interval(base)} | {ns_interval(cand)} | "
                f"{interval['estimate']:.4f} | [{interval['lower']:.4f}, {interval['upper']:.4f}] | "
                f"{decision} | **{accepted['outcome']}** | "
                f"{accepted['flagged_windows']}/{accepted['total_windows']} | "
                f"{conversion_text([p['baseline'].get('conversion') for p in pairs])} | "
                f"{recorded} |"
            )


if __name__ == "__main__":
    main()
