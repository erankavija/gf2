#!/usr/bin/env python3
"""Project the consumer profile into its summary table (jit:19513245).

Usage: summarize-profile.py <profile-dir> > profile-summary.md

Reads every completed session's `cases.json`, the ladder the sessions
measured, the first session's hardware counters, and the disassembly record,
and writes one Markdown file. It derives everything it prints from those bytes
and states no value the profile did not observe. Running it twice over the same
directory writes the same bytes.

Every stochastic figure carries its sample count and interval. A per-session
value is the median of that session's five windows; the figure reported here is
the median over the sessions with the order-statistic interval [x(2), x(8)] of
nine sessions, which covers the median with probability 0.961. A ratio of two
such medians is descriptive and is labelled so: the A/B receipts, not this
file, carry the intervals that decide anything.
"""

import json
import pathlib
import sys

# Order-statistic interval of the median of nine independent sessions: the
# second and eighth order statistics, coverage 1 - 2 * P(Binomial(9, 1/2) <= 1).
LOW_RANK, HIGH_RANK = 2, 8
COVERAGE = "0.961"


def order_interval(values):
    """Median and order-statistic interval of a session series."""
    ordered = sorted(values)
    count = len(ordered)
    median = ordered[count // 2] if count % 2 else (ordered[count // 2 - 1] + ordered[count // 2]) / 2
    if count >= HIGH_RANK:
        return median, ordered[LOW_RANK - 1], ordered[HIGH_RANK - 1]
    return median, ordered[0], ordered[-1]


def number(value):
    """Fixed rendering, so a re-run writes the same bytes."""
    if value >= 1000:
        return f"{value:,.0f}".replace(",", " ")
    if value >= 10:
        return f"{value:.1f}"
    return f"{value:.3f}"


def main():
    root = pathlib.Path(sys.argv[1])
    sessions = sorted(path for path in root.glob("rep-*/cases.json"))
    if not sessions:
        raise SystemExit(f"no session records under {root}")
    records = [json.loads(path.read_text()) for path in sessions]
    ladder = [case["id"] for case in json.loads((root / "ladder.json").read_text())]
    by_case = {case_id: [] for case_id in ladder}
    for session in records:
        for record in session:
            by_case[record["id"]].append(record)
    log_lines = (root / "repetitions.log").read_text().splitlines()
    completed = [line for line in log_lines if " done " in line and line.startswith("rep-")]

    print("# Byte-field consumer profile")
    print()
    print("> **Diátaxis Type:** Reference")
    print()
    print(f"Source: `dev/active/19513245/survey/summarize-profile.py` over "
          f"{len(sessions)} session records under this directory, "
          f"{len(completed)} of them logged complete in `repetitions.log`, "
          f"and `ladder.json`, the case ladder the sessions measured.")
    print()
    print("Every wall time is nanoseconds per call. A session's value is the median of its "
          "five 40 ms windows; the value below is the median over the sessions, and the "
          f"interval is the order-statistic interval [x({LOW_RANK}), x({HIGH_RANK})] of "
          f"{len(sessions)} sessions, coverage {COVERAGE}. Allocation counts and reuse counts "
          "are exact integers observed in one call and are identical across sessions unless the "
          "table says otherwise. The counting allocator adds two relaxed atomic increments per "
          "allocation, so a route's wall time here carries the cost of counting its own "
          "allocations; the A/B receipts carry the timing that decides anything.")
    print()

    print("## Routes")
    print()
    print("| Case | Route selected at run time | ns/call | interval | sessions |")
    print("|---|---|---:|---|---:|")
    for case_id in ladder:
        rows = by_case[case_id]
        median, low, high = order_interval([row["ns_per_call"] for row in rows])
        print(f"| `{case_id}` | `{rows[0]['selected_path']}` | {number(median)} | "
              f"[{number(low)}, {number(high)}] | {len(rows)} |")
    print()

    print("## Allocation, reuse and conversion")
    print()
    print("`allocations` and `bytes` are what one call allocates. `reuse` is the number of "
          "multiplications one prepared coefficient table serves, and is zero for a route that "
          "prepares no coefficient table. `table` is the prepared table's size in bytes. The "
          "remaining columns are untimed probes, each the median of nine repetitions inside one "
          "session and then the median over the sessions: `setup` builds the field context, "
          "`table prep` builds the table, `pack` converts the operands into the route's "
          "representation and `unpack` converts the result out of it. A zero means the route "
          "does not perform that step.")
    print()
    print("| Case | allocations | bytes | reuse | table | setup ns | table prep ns | pack ns | unpack ns |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for case_id in ladder:
        rows = by_case[case_id]
        allocations = sorted({row["allocations_per_call"] for row in rows})
        allocated = sorted({row["allocated_bytes_per_call"] for row in rows})
        def probe(field):
            median, _, _ = order_interval([row[field] for row in rows])
            return number(median)
        print(f"| `{case_id}` | {render_set(allocations)} | {render_set(allocated)} | "
              f"{rows[0]['reuse_per_table']} | {rows[0]['table_bytes']} | {probe('setup_ns')} | "
              f"{probe('table_ns')} | {probe('pack_ns')} | {probe('unpack_ns')} |")
    print()

    print("## Prototype over current route")
    print()
    print("Each row divides the current route's session-median wall time by the prototype's, so "
          "a value above one favours the prototype. Both medians come from separate executions "
          "inside the same sessions and the quotient carries no interval, so every value here is "
          "descriptive; the A/B receipts of the three campaign families carry the paired "
          "estimates and their bootstrap intervals.")
    print()
    print("| Shape | current ns/call | prototype ns/call | descriptive ratio |")
    print("|---|---:|---:|---:|")
    for case_id in ladder:
        if not case_id.endswith("-current"):
            continue
        shape = case_id[: -len("-current")]
        candidate = f"{shape}-prototype"
        if candidate not in by_case:
            continue
        current, _, _ = order_interval([row["ns_per_call"] for row in by_case[case_id]])
        prototype, _, _ = order_interval([row["ns_per_call"] for row in by_case[candidate]])
        print(f"| `{shape}` | {number(current)} | {number(prototype)} | "
              f"{number(current / prototype)} |")
    print()

    counters = root / "rep-01" / "counters"
    if counters.is_dir():
        print("## Hardware counters")
        print()
        print("Counters of the first session only, over a one-second repetition of each case in "
              "its own process, user mode. Each value is divided by the calls that process "
              "reported, so a row is per call. `IPC` is instructions divided by cycles. These "
              "are single observations with no interval: they explain a limit, they do not "
              "estimate one.")
        print()
        print("| Case | calls | cycles/call | instructions/call | IPC | branch misses/call | cache misses/call |")
        print("|---|---:|---:|---:|---:|---:|---:|")
        for path in sorted(counters.glob("*.csv")):
            case_id = path.stem
            calls_file = counters / f"{case_id}.calls"
            calls = int(calls_file.read_text().split()[1])
            events = {}
            for line in path.read_text().splitlines():
                if line.startswith("#") or not line.strip():
                    continue
                fields = line.split(",")
                if len(fields) < 3 or not fields[0].strip():
                    continue
                events[fields[2].split(":")[0]] = float(fields[0])
            cycles = events.get("cycles", 0.0)
            instructions = events.get("instructions", 0.0)
            ipc = instructions / cycles if cycles else 0.0
            print(f"| `{case_id}` | {calls} | {number(cycles / calls)} | "
                  f"{number(instructions / calls)} | {ipc:.2f} | "
                  f"{number(events.get('branch-misses', 0.0) / calls)} | "
                  f"{number(events.get('cache-misses', 0.0) / calls)} |")
        print()

    print("## Generated code")
    print()
    print("`dev/active/19513245/survey/asm/` holds the annotated release disassembly of the "
          "measured routes and their instruction mix, written by `disassemble.sh` from the arm "
          "executable of the same build these sessions ran; `asm/index.txt` names each file, the "
          "symbols it covers and the pattern that selected them.")
    print()


def render_set(values):
    """One integer, or the range the sessions disagreed over."""
    if len(values) == 1:
        return str(values[0])
    return f"{values[0]}..{values[-1]}"


if __name__ == "__main__":
    main()
