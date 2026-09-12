#!/usr/bin/env python3
"""Projects the repeated profile's counters, call graphs and derived figures.

Reads every `rep-*` session directory of a profile run and prints three
tables. Counter ratios: per counter case, the median over the sessions of each
derived ratio (instructions per cycle, front-end stalled cycles per cycle,
branch misses per branch, L1 load misses per load, last-level misses per
reference, instructions per timed call) with its order-statistic interval.
Sampled shares: per call-graph case, each symbol's self samples pooled over
the sessions, the share they make of the pooled total, and its Wilson
interval. Derived figures: the bandwidths, per-batch costs and cross-cell
ratios the findings cite, each computed per session from the session's own
records, then summarized by the same order-statistic interval or, for a ratio
of two cells, by a percentile bootstrap over both cells' session values.
Every figure is computed from the files beside it; nothing is typed in.

Usage: summarize-attribution.py <profile-dir>
"""

import hashlib
import json
import pathlib
import re
import sys
from collections import OrderedDict, defaultdict

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from intervals import (  # noqa: E402
    BOOTSTRAP_GENERATOR,
    BOOTSTRAP_RESAMPLES,
    bootstrap_ratio,
    median_interval,
    wilson_interval,
)

EVENT = re.compile(r"^\s*([\d,]+)\s+(\S+?)(?::u)?\s")
SAMPLE_LINE = re.compile(r"^\s*(\d+\.\d+)%\s+(\d+)\s+\S+\s+(\S+)\s+\[(.)\]\s+(.*?)\s*$")
SAMPLE_TOTAL = re.compile(r"SAMPLE events:\s+(\d+)")
UNRESOLVED = re.compile(r"^0x[0-9a-f]+$")
# The fixed period `sweep.sh` samples at, in user-space cycles.
SAMPLE_PERIOD = 4000037

# Symbols printed even below the share threshold, as an upper bound on their
# share: (call-graph case, substring of the symbol).
BOUNDED = [
    ("ldpc-codeword-check-64800", "popcnt"),
    ("ldpc-codeword-check-64800", "count_ones"),
    ("ldpc-codeword-check-64800-find", "find_first_one"),
]

# Declared seed of the bootstrap resamples; each ratio's own seed is derived
# from it and its identifier and printed beside the interval.
BOOTSTRAP_SEED = 20260911


def sessions_of(root):
    sessions = sorted(path for path in root.glob("rep-*") if path.is_dir())
    if not sessions:
        sys.exit(f"no rep-* session under {root}")
    return sessions


def record_in(path):
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("{"):
            return json.loads(line)
    raise SystemExit(f"{path} holds no consumer-profile record")


def events(path):
    values = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = EVENT.match(line)
        if match:
            values[match.group(2)] = int(match.group(1).replace(",", ""))
    return values


def samples(path):
    """Returns (total, {(object, symbol): self samples}) of one samples file."""
    text = path.read_text(encoding="utf-8", errors="replace")
    total = SAMPLE_TOTAL.search(text)
    counts = defaultdict(int)
    for line in text.splitlines():
        match = SAMPLE_LINE.match(line)
        if match:
            symbol = match.group(5)
            if UNRESOLVED.match(symbol):
                # Addresses with no symbol are pooled per object and mode.
                if match.group(4) == "k":
                    symbol = "(kernel addresses)"
                else:
                    symbol = "(unresolved addresses)"
            counts[(match.group(3), symbol)] += int(match.group(2))
    listed = sum(counts.values())
    if total is None:
        raise SystemExit(f"{path} records no sample total")
    if int(total.group(1)) != listed:
        raise SystemExit(f"{path}: {listed} listed self samples against {total.group(1)} recorded")
    return listed, counts


def seed_text(record):
    """The case seed beside the generator its record names."""
    rng = record["fixture_rng"]
    if rng is None:
        return f"seed {record['seed']} (unused)"
    return f"seed {record['seed']}, rand {rng['rand']} StdRng"


def fmt(value, digits):
    return f"{value:.{digits}f}"


def interval(values, digits):
    median, lower, upper, _ = median_interval(values)
    return f"{fmt(median, digits)} [{fmt(lower, digits)}, {fmt(upper, digits)}]"


def sweep_rows(sessions):
    """Returns {(workload, size json, path): [record per session]}."""
    rows = defaultdict(list)
    for session in sessions:
        for line in (session / "profile.jsonl").read_text(encoding="utf-8").splitlines():
            if line.strip():
                record = json.loads(line)
                key = (record["workload"], json.dumps(record["size"], sort_keys=True), record["path"])
                rows[key].append(record)
    return rows


def row(rows, workload, size, path):
    key = (workload, json.dumps(size, sort_keys=True), path)
    if key not in rows:
        raise SystemExit(f"no sweep row {key}")
    return rows[key]


def counter_section(sessions):
    print("## Counter ratios")
    print()
    print(
        "Each ratio is computed per session from that session's `perf stat` files, then "
        f"summarized as the median over the n = {len(sessions)} sessions with its "
        "order-statistic interval. Instructions per call divide the run's instructions by "
        "the calls its record reports, so preparation is amortized into them."
    )
    print()
    print(
        "| Case | Fixture | IPC | Front-end stalled cycles / cycle (%) | Branch misses / branch (%) | "
        "L1d load misses / load (%) | LLC misses / reference (%) | Instructions per call |"
    )
    print("|---|---|---:|---:|---:|---:|---:|---:|")
    cases = sorted(
        {path.name.rsplit(".", 2)[0] for path in (sessions[0] / "perf-stat").glob("*.core.txt")}
    )
    for case in cases:
        columns = defaultdict(list)
        fixtures = set()
        for session in sessions:
            core_path = session / "perf-stat" / f"{case}.core.txt"
            core = events(core_path)
            memory = events(session / "perf-stat" / f"{case}.memory.txt")
            record = record_in(core_path)
            fixtures.add(seed_text(record))
            calls = record["calls"]
            columns["ipc"].append(core["instructions"] / core["cycles"])
            columns["frontend"].append(100 * core["stalled-cycles-frontend"] / core["cycles"])
            columns["branch"].append(100 * core["branch-misses"] / core["branches"])
            columns["l1"].append(100 * memory["L1-dcache-load-misses"] / memory["L1-dcache-loads"])
            columns["llc"].append(100 * memory["cache-misses"] / memory["cache-references"])
            columns["per_call"].append(core["instructions"] / calls)
        print(
            f"| `{case}` | {'; '.join(sorted(fixtures))} | "
            f"{interval(columns['ipc'], 2)} | {interval(columns['frontend'], 2)} | "
            f"{interval(columns['branch'], 3)} | {interval(columns['l1'], 2)} | "
            f"{interval(columns['llc'], 2)} | {interval(columns['per_call'], 0)} |"
        )


def share_section(sessions):
    print("## Sampled shares")
    print()
    print(
        f"Each call-graph case is sampled once every {SAMPLE_PERIOD} user-space cycles "
        "(about 1 kHz) in every session, so each sample stands for the same number of "
        "cycles. A symbol's samples are its self samples (leaf frame) pooled over the "
        "sessions, and its share is those samples over the pooled total of the case, "
        "preparation included. The interval is the Wilson score interval at 95% "
        "[Wilson1927], which treats the pooled samples as independent draws; the "
        "per-session range shows the variation between sessions and is descriptive. "
        "Symbols at or above 0.5% of the pooled samples are listed, plus named bounds; "
        "kernel addresses are samples recorded at a kernel address although the event "
        "counts user-space cycles only."
    )
    cases = sorted(
        path.name[: -len(".samples.txt")]
        for path in (sessions[0] / "perf-report").glob("*.samples.txt")
    )
    for case in cases:
        pooled = defaultdict(int)
        per_session = []
        fixtures = set()
        total = 0
        for session in sessions:
            session_total, counts = samples(session / "perf-report" / f"{case}.samples.txt")
            fixtures.add(seed_text(record_in(session / "perf-report" / f"{case}.run.txt")))
            total += session_total
            per_session.append((session_total, counts))
            for key, value in counts.items():
                pooled[key] += value
        print()
        print(
            f"### `{case}` (N = {total} samples over {len(sessions)} sessions; "
            f"{'; '.join(sorted(fixtures))})"
        )
        print()
        print("| Symbol | Object | Samples | Share | Wilson 95% | Per-session share (range) |")
        print("|---|---|---:|---:|---|---|")
        shown = [key for key, value in pooled.items() if value / total >= 0.005]
        for bounded_case, fragment in BOUNDED:
            if bounded_case == case:
                matches = [key for key in pooled if fragment in key[1]]
                shown.extend(key for key in matches if key not in shown)
                if not matches:
                    shown.append(("(any)", f"(no symbol containing `{fragment}`)"))
        for key in sorted(shown, key=lambda item: -pooled.get(item, 0)):
            count = pooled.get(key, 0)
            lower, upper = wilson_interval(count, total)
            shares = [counts.get(key, 0) / session_total for session_total, counts in per_session]
            symbol = key[1].replace("|", "\\|")
            print(
                f"| `{symbol}` | {key[0]} | {count} of {total} | {100 * count / total:.2f}% | "
                f"[{100 * lower:.2f}%, {100 * upper:.2f}%] | "
                f"{100 * min(shares):.1f}%-{100 * max(shares):.1f}% |"
            )


def share_of(session, case, fragment):
    """Self-sample share of the symbols of `case` whose name contains `fragment`."""
    total, counts = samples(session / "perf-report" / f"{case}.samples.txt")
    return sum(value for key, value in counts.items() if fragment in key[1]) / total


# Bytes a call must move, per sweep route: (label, workload, size, path, bytes).
BANDWIDTHS = [
    ("64-row x 64-word row XOR, dispatched: 32 row pairs x 64 words x 8 B x (2 loads + 1 store)",
     "row-xor", {"rows": 64, "words": 64}, "ops-dispatched", 32 * 64 * 8 * 3),
    ("64-row x 8192-word row XOR, dispatched: 32 x 8192 x 8 B x 3",
     "row-xor", {"rows": 64, "words": 8192}, "ops-dispatched", 32 * 8192 * 8 * 3),
    ("popcount, 4096 words (32 KiB), SIMD backend", "popcount", {"words": 4096}, "simd-backend", 4096 * 8),
    ("popcount, 65536 words (512 KiB), SIMD backend", "popcount", {"words": 65536}, "simd-backend", 65536 * 8),
    ("popcount, 65536 words (512 KiB), scalar backend", "popcount", {"words": 65536}, "scalar-backend", 65536 * 8),
    ("dense matvec 1024x4096: matrix read, 512 KiB", "dense-matvec", {"rows": 1024, "cols": 4096}, "current", 1024 * 4096 // 8),
    ("dense transpose 4096-square: input read plus output write, 2 x 2 MiB",
     "dense-transpose", {"rows": 4096, "cols": 4096}, "current", 2 * 4096 * 4096 // 8),
]

# Per-call cost of one symbol: share of the call-graph run times that run's
# own per-call time. (label, call-graph case, symbol substring).
SYMBOL_COSTS = [
    ("`packed_write_codeword` per 256-message short-frame mother-code batch, current family",
     "bch-m14-b256-current", "packed_write_codeword"),
    ("`packed_write_codeword` per 256-message short-frame mother-code batch, reference family pinned",
     "bch-m14-b256-reference", "packed_write_codeword"),
    ("`packed_write_codeword` per 256-message short-frame mother-code batch, fold family pinned",
     "bch-m14-b256-clmul", "packed_write_codeword"),
]

# Instructions per unit of work from the counter runs: (label, case, units per call).
INSTRUCTION_RATES = [
    ("instructions per 64x64 block, portable transpose", "transpose64-256-scalar", 256),
    ("instructions per 64x64 block, detected AVX2 transpose", "transpose64-256-detected", 256),
    ("instructions per call, 507-word all-zero test as a count", "zero-test-507w-allzero-count", 1),
    ("instructions per call, 507-word all-zero test as a search", "zero-test-507w-allzero-find", 1),
]

# Ratios of two sweep rows: (identifier, label, numerator row, denominator row).
RATIOS = [
    ("zero-first-bit-507w", "507-word zero test, first bit set: count / find-first-one",
     ("zero-test", {"words": 507, "set_bit": 0}, "count-ones"),
     ("zero-test", {"words": 507, "set_bit": 0}, "find-first-one")),
    ("bch-m14-b16-fold", "BCH short-frame mother field (m = 14), B = 16: current / fold family",
     ("bch-encode-batch", {"degree": 14, "batch": 16}, "current"),
     ("bch-encode-batch", {"degree": 14, "batch": 16}, "family-clmul-fold")),
    ("bch-m14-b16-bitslice", "BCH short-frame mother field (m = 14), B = 16: current / bitslice family",
     ("bch-encode-batch", {"degree": 14, "batch": 16}, "current"),
     ("bch-encode-batch", {"degree": 14, "batch": 16}, "family-bitslice-interleaved")),
    ("bch-m16-b256-fold", "BCH normal-frame mother field (m = 16), B = 256: current / fold family",
     ("bch-encode-batch", {"degree": 16, "batch": 256}, "current"),
     ("bch-encode-batch", {"degree": 16, "batch": 256}, "family-clmul-fold")),
    ("bch-m16-b256-bitslice", "BCH normal-frame mother field (m = 16), B = 256: current / bitslice family",
     ("bch-encode-batch", {"degree": 16, "batch": 256}, "current"),
     ("bch-encode-batch", {"degree": 16, "batch": 256}, "family-bitslice-interleaved")),
]


def derived_section(sessions):
    rows = sweep_rows(sessions)
    n = len(sessions)
    print("## Derived figures")
    print()
    print(
        f"Every figure below is computed per session from that session's own records and "
        f"summarized over the n = {n} sessions: single-route figures as the median with its "
        "order-statistic interval, ratios of two routes by a percentile bootstrap of the "
        f"ratio of medians over both routes' session values ({BOOTSTRAP_RESAMPLES} "
        f"resamples, 95% nearest-rank interval [EfronTibshirani1993], generator "
        f"{BOOTSTRAP_GENERATOR} with the seed printed beside each interval)."
    )
    print()
    print("### Useful bandwidth")
    print()
    print("Bytes the operation must move per call over the session's wall-clock mean per call.")
    print()
    print("| Route and bytes per call | GB/s, median [interval] |")
    print("|---|---:|")
    for label, workload, size, path, moved in BANDWIDTHS:
        values = [moved / record["ns_per_call"] for record in row(rows, workload, size, path)]
        print(f"| {label} | {interval(values, 1)} |")
    print()
    print("### Per-call cost of one symbol")
    print()
    print(
        "The symbol's self-sample share in a session's call-graph run times the per-call time "
        "that same run's record reports (`perf-report/<case>.run.txt`)."
    )
    print()
    print("| Symbol and route | ms per call, median [interval] |")
    print("|---|---:|")
    for label, case, fragment in SYMBOL_COSTS:
        values = [
            share_of(session, case, fragment)
            * record_in(session / "perf-report" / f"{case}.run.txt")["ns_per_call"]
            / 1e6
            for session in sessions
        ]
        print(f"| {label} | {interval(values, 2)} |")
    print()
    print("### Instructions per unit of work")
    print()
    print("| Figure | Instructions, median [interval] |")
    print("|---|---:|")
    for label, case, units in INSTRUCTION_RATES:
        values = []
        for session in sessions:
            path = session / "perf-stat" / f"{case}.core.txt"
            values.append(events(path)["instructions"] / (record_in(path)["calls"] * units))
        print(f"| {label} | {interval(values, 0)} |")
    print()
    print("### Ratios of two routes")
    print()
    print("| Ratio | Estimate [95% interval] | Bootstrap seed |")
    print("|---|---:|---:|")
    for identifier, label, top, bottom in RATIOS:
        numerator = [record["ns_per_call"] for record in row(rows, *top)]
        denominator = [record["ns_per_call"] for record in row(rows, *bottom)]
        seed = int.from_bytes(
            hashlib.sha256(f"{BOOTSTRAP_SEED}:{identifier}".encode()).digest()[:8], "little"
        )
        estimate, lower, upper = bootstrap_ratio(numerator, denominator, seed)
        print(f"| {label} | {estimate:.3f} [{lower:.3f}, {upper:.3f}] | {seed} |")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = pathlib.Path(sys.argv[1])
    sessions = sessions_of(root)
    print("# Counter ratios, sampled shares and derived figures of the consumer profile")
    print()
    print(
        "Generated by `dev/active/04b85d10/survey/summarize-attribution.py` from the "
        f"`rep-*/perf-stat/`, `rep-*/perf-report/` and `rep-*/profile.jsonl` files of the "
        f"{len(sessions)} profile sessions beside it. Counters are user-space events over one "
        "whole `consumer-profile` run of the named case per session (preparation included); "
        "the generic backend-stall event is unsupported on this host, so no backend-stall "
        "ratio is derived. Each case names its fixture seed beside the generator its "
        "records report; `profile-summary.md` states that generator's algorithm."
    )
    print()
    counter_section(sessions)
    print()
    share_section(sessions)
    print()
    derived_section(sessions)


if __name__ == "__main__":
    main()
