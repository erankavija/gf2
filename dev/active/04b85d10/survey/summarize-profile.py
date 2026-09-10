#!/usr/bin/env python3
"""Renders the repeated sweep of the bit-storage consumer profile as Markdown.

Reads `rep-*/profile.jsonl` under a profile directory, one file per profile
session, and projects every measured route into one table per consumer
family: the median over the sessions of the route's wall-clock mean per call
with the order-statistic interval of `intervals.median_interval`, the
allocation counters, and the fixture seed beside the generator its records
name. A second table does the same for the setup and conversion probes. Every
figure comes from the records; nothing is carried in from another run.

Usage: summarize-profile.py <profile-dir>
"""

import json
import pathlib
import statistics
import sys
from collections import OrderedDict

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from intervals import median_interval  # noqa: E402

FAMILY_OF = {
    "row-xor": "logical",
    "dense-rref": "logical",
    "dense-matvec": "logical",
    "ldpc-syndrome": "logical",
    "popcount": "count",
    "zero-test": "count",
    "ldpc-codeword-check": "count",
    "transpose-64x64": "layout",
    "dense-transpose": "layout",
    "bch-encode-batch": "layout",
    "bch-encode-batch-alloc": "layout",
    "bch-encode-batch-parallel": "layout",
    "dvb-bch-encode": "layout",
    "field-id-hint": "layout",
}

FAMILY_TITLE = OrderedDict(
    [
        ("logical", "Logical row, parity and dense-matrix consumers"),
        ("count", "Count and fused-reduction consumers"),
        ("layout", "Transpose, bitslice and BCH encoding consumers"),
    ]
)

# What each setup and conversion probe of `src/lib.rs` `prepare` times.
PROBES = [
    ("dense-rref", "setup: one fixture matrix materialized; dispatch: `resolve_xor_inplace(stride)`, the kernel resolution `rref` performs per call."),
    ("dense-matvec", "setup: one fixture matrix materialized; unpack: allocating a `BitVec` with `rows` bits of capacity and appending `rows` bits, the output construction `matvec` performs; dispatch: `matvec_route(stride)` only, because the kernel-table read that follows it is crate-private."),
    ("dense-transpose", "setup: one fixture matrix materialized; dispatch: `transpose_route` only, because the block-kernel lookup is crate-private."),
    ("ldpc-syndrome, ldpc-codeword-check", "setup: construction of the declared code; unpack: allocating a `BitVec` with one bit of capacity per check and appending one bit per check, the output construction of the CSR matvec."),
    ("bch-encode-batch*", "setup: `BinaryBchCode` construction; pack: one `encode_workspace()` construction, the workspace `encode_batch_into` borrows, because the families pack messages inside the timed call; batch fill: regenerating the batch (allocation plus the fixture generator's fill), a harness cost a consumer replaces with a copy of its own data; dispatch: `selected_encode_family`, the plan construction and family selection the entry point performs per call."),
    ("dvb-bch-encode", "setup: `BchCode::dvb_t2` construction; batch fill: regenerating the batch as above."),
]


def size_text(size):
    return " ".join(f"{key}={value}" for key, value in sorted(size.items()))


def key_of(record):
    return (record["workload"], json.dumps(record["size"], sort_keys=True), record["path"])


def number(value):
    if value >= 100:
        return f"{value:.0f}"
    if value >= 10:
        return f"{value:.1f}"
    return f"{value:.2f}"


def interval_text(values):
    median, lower, upper, _ = median_interval(values)
    return f"{number(median)} [{number(lower)}, {number(upper)}]"


def exact_text(values, digits):
    lowest, highest = min(values), max(values)
    if lowest == highest:
        return f"{lowest:.{digits}f}"
    return f"{statistics.median(values):.{digits}f} (range {lowest:.{digits}f}-{highest:.{digits}f})"


def load(root):
    sessions = []
    for path in sorted(root.glob("rep-*/profile.jsonl")):
        records = [
            json.loads(line)
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        sessions.append((path.parent.name, records))
    if not sessions:
        sys.exit(f"no rep-*/profile.jsonl under {root}")
    return sessions


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    root = pathlib.Path(sys.argv[1])
    sessions = load(root)
    rows = OrderedDict()
    for _, records in sessions:
        for record in records:
            rows.setdefault(key_of(record), []).append(record)
    sizes = {len(group) for group in rows.values()}
    n = len(sessions)
    _, _, _, coverage = median_interval(list(range(n)))

    generators = {
        json.dumps(record["fixture_rng"], sort_keys=True)
        for group in rows.values()
        for record in group
        if record["fixture_rng"]
    }
    if len(generators) != 1:
        sys.exit(f"expected one fixture generator across records, found {len(generators)}")
    rng = json.loads(generators.pop())
    rng_short = f"rand {rng['rand']} StdRng"

    print("# Current-code consumer profile sweep")
    print()
    print(
        f"Projection of `rep-*/profile.jsonl`: n = {n} profile sessions "
        f"({', '.join(name for name, _ in sessions)}), each one `sweep.sh` run under its own "
        f"`dev/scripts/ccx1-bench-flock.sh --full-host` invocation, over {len(rows)} routes. "
        "Within a session each route is one process, and its `ns/call` is the wall-clock "
        "mean over the calls that process made."
    )
    if sizes != {n}:
        print()
        print(f"Some routes are missing from some sessions: rows carry {sorted(sizes)} sessions.")
    print()
    print(
        f"Each `ns/call` below is the median over the n = {n} sessions with the "
        "distribution-free order-statistic interval for the median: for n independent "
        "sessions the count below the population median is Binomial(n, 1/2), so "
        "[x(j), x(n+1-j)] covers it with probability 1 - 2 P(Binomial(n, 1/2) <= j - 1), "
        f"here {coverage:.1%} (`dev/active/04b85d10/survey/intervals.py`). Calls per "
        "session are descriptive. Allocation counters are exact counts from the counting "
        "allocator over the timed calls; one value means every session reported it."
    )
    print()
    print(
        f"Fixture generator, named by every seeded record's `fixture_rng` beside its "
        f"`seed`: {rng['generator']}. Algorithm: {rng['algorithm']}. Resolved versions: "
        f"rand {rng['rand']}, rand_chacha {rng['rand_chacha']}, rand_core {rng['rand_core']}. "
        "A seed marked unused belongs to a deterministic fixture that draws nothing."
    )
    print()
    print(
        "BCH rows: degree 14 is the mother field of the DVB-T2 short-frame BCH code and "
        "degree 16 that of the normal-frame code [Etsi2015] (rows T2S and T2N of "
        "`dev/active/4e732b56/workload-selection.md`); degree 8 is its row B3. "
        "`dvb-bch-encode` measures the shortened rate-1/2 frames themselves, n = 7200 short "
        "and n = 32400 normal."
    )

    for family, title in FAMILY_TITLE.items():
        members = [
            (key, group) for key, group in rows.items() if FAMILY_OF.get(key[0]) == family
        ]
        if not members:
            continue
        print()
        print(f"## {title}")
        print()
        print(
            f"| Workload | Size | Route | Observed path | Seed (generator) | Calls/session "
            f"(median) | ns/call, median [interval], n = {n} | Allocations/call | Bytes/call | "
            "Peak live bytes |"
        )
        print("|---|---|---|---|---|---:|---:|---:|---:|---:|")
        for (workload, _, path), group in members:
            first = group[0]
            observed = sorted({record["selected_path"] for record in group})
            seeds = {record["seed"] for record in group}
            seed = ", ".join(str(value) for value in sorted(seeds))
            if first["fixture_rng"]:
                seed_text = f"{seed} ({rng_short})"
            else:
                seed_text = f"{seed} (unused)"
            timed = [record["timed"] for record in group]
            print(
                f"| {workload} | {size_text(first['size'])} | {path} | {', '.join(observed)} | "
                f"{seed_text} | {statistics.median(r['calls'] for r in group):.0f} | "
                f"{interval_text([r['ns_per_call'] for r in group])} | "
                f"{exact_text([t['allocations_per_call'] for t in timed], 2)} | "
                f"{exact_text([t['allocated_bytes_per_call'] for t in timed], 0)} | "
                f"{exact_text([float(t['peak_live_bytes']) for t in timed], 0)} |"
            )

    conversions = [(key, group) for key, group in rows.items() if group[0].get("conversion")]
    if conversions:
        print()
        print("## Reported setup and conversion probes")
        print()
        print(
            "Whole-consumer routes report the probes the receipt schema carries. `setup` "
            "is one-shot preparation the timed body does not repeat; the remaining columns "
            "are per timed call. Each cell is the median over the sessions with the same "
            "order-statistic interval, in nanoseconds; a probe times many repetitions and "
            "reports the integer mean, so a zero is either a phase the route does not have "
            "or a per-call cost below 1 ns. What each probe times:"
        )
        print()
        for workload, text in PROBES:
            print(f"- `{workload}`: {text}")
        print()
        print(
            f"| Workload | Size | Route | setup ns | pack ns | unpack ns | batch fill ns | "
            f"dispatch ns |"
        )
        print("|---|---|---|---:|---:|---:|---:|---:|")
        for (workload, _, path), group in conversions:
            cells = []
            for field in ("setup_ns", "pack_ns", "unpack_ns", "batch_fill_ns", "dispatch_ns"):
                values = [record["conversion"][field] for record in group]
                cells.append("0" if max(values) == 0 else interval_text(values))
            print(
                f"| {workload} | {size_text(group[0]['size'])} | {path} | " + " | ".join(cells) + " |"
            )


if __name__ == "__main__":
    main()
