#!/usr/bin/env python3
"""Project the repeated steady-state LDPC profile (jit:3be770d5).

Reads every completed `rep-*` session of a run-profile.sh output directory and
writes one JSON summary and its Markdown projection. A session is completed
when the series' execution log records it done after a start under the first
session's identity; a done session under another identity stops the summary.
Every figure is computed from the files beside it; nothing is typed in.

Sampled shares. Each record case is sampled once every fixed number of
user-space cycles, so each sample stands for the same number of cycles.
Samples are pooled over the sessions per code address. Addresses in the
harness executable are resolved to their inline chains with `addr2line -a -f
-i -C`: source locations from the innermost code outward, each the call site
of the level inside it. The first location a rule covers names the address's
category; the rules are source spans of the files the harness executable was
built from, selected by the executable's recorded digest
(RULES_BY_EXECUTABLE). Addresses in other objects (libc) are
categorized by symbol. A category's share is its pooled samples
over the pooled total of the case, with a Wilson 95% interval [Wilson1927]
that treats pooled samples as independent draws; the per-session range shows
the variation between sessions and is descriptive. `attribution.tsv` lists
every resolved address with its chain and category, so each rule can be
checked.

Every sample the case recorded reaches a category: the pooled total of a case
equals the SAMPLE count `perf report --stats` reports for it, and a case whose
listing and total disagree stops the summary rather than reporting a share
over an incomplete denominator. Samples whose instruction pointer perf
resolved to no object carry the `unmapped-ip` category; on this host they are
the kernel-space addresses its own report lists as `[k]`, which `cycles:u`
still samples through interrupt skid and an unprivileged session cannot map.

Usage: summarize-profile.py --self-test checks the parsing and the total rule
against synthesized listings and writes nothing.

Counters and throughput. Each stat case records two counter groups in two
runs per session. A session's per-worker time per frame is the mean of its
two runs; counter ratios come from the run that counted them. Per-session
figures are summarized by the median over the sessions with the
order-statistic interval of `intervals.median_interval`. Scaling and
route-pair (ROUTE_PAIRS) ratios are ratios of medians over sessions with the
percentile bootstrap of `intervals.bootstrap_ratio`, seeded per ratio from its
identifier.

Allocation census. The first session's counting-allocator records are
checked against the structural prediction of `edge-costs.py`: one
allocation per edge per iteration plus two syndrome vectors per check.

Usage: summarize-profile.py <profile-dir> --json FILE --markdown FILE
[--attribution FILE]; the address listing defaults to the profile directory.
"""

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile
from collections import defaultdict

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from intervals import (  # noqa: E402
    BOOTSTRAP_GENERATOR,
    BOOTSTRAP_RESAMPLES,
    bootstrap_ratio,
    median_interval,
    wilson_interval,
)

REPO = pathlib.Path(subprocess.check_output(
    ["git", "-C", str(pathlib.Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"], text=True
).strip())
BINARY = REPO / "target/ldpc-throughput/release/ldpc-profile"
IDENTITY = REPO / "dev/bench_results/3be770d5/preparation/build-identity.json"
EXPECTED_BINARY_SHA256 = None
STRUCTURE = REPO / "dev/bench_results/3be770d5/preparation/structural-costs.json"
CODE_KEYS = {"dvb-t2-r12": "dvb-t2-r12", "nr-bg1-r12": "nr-bg1-z384"}
# perf prints `symbol+0xoff (object+0xoff)` per sample, and `[unknown]
# ([unknown])` for an instruction pointer it has no map for, so both
# offsets are optional. UNMAPPED names the second shape's category.
ADDRESS_LINE = re.compile(r"^(\d+)\t(.*?)(?:\+0x([0-9a-f]+))? \((.*?)(?:\+0x([0-9a-f]+))?\)$")
UNMAPPED = "unmapped-ip"
TOTAL_LINE = re.compile(r"SAMPLE events:\s+(\d+)")
RUST_HASH = re.compile(r"::h[0-9a-f]{16}$")

ALLOCATOR_SYMBOLS = ("malloc", "free", "_int_malloc", "_int_free", "cfree", "tcache",
                     "unlink_chunk", "malloc_consolidate", "__libc_malloc", "__libc_free")
COPY_SYMBOLS = ("memcpy", "memmove", "memset")
# Arm pairs whose time per frame is compared at each code and worker count.
ROUTE_PAIRS = [("gf2", "aff3ct", "gf2 over AFF3CT flooding time per frame"),
               ("lane", "single-frame", "lane route over single-frame route time per frame")]

# Location rules: (category, source path suffix, first line, last line). An
# address's inline chain is a list of source locations from the innermost code
# outward, each the call site of the level inside it; the first location a
# rule covers names the category, and locations no rule covers (library
# iterators, bit-vector helpers) pass the decision outward. The spans are the
# functions of the sources a harness executable was built from: gf2-coding,
# gf2-core and gf2-kernels-simd at the revision of its profile, AFF3CT at the
# recorded commit.
CORE = "crates/gf2-coding/src/ldpc/core.rs"
LLR = "crates/gf2-coding/src/llr.rs"
SIMD_LLR = "crates/gf2-kernels-simd/src/llr.rs"
SPARSE = "crates/gf2-core/src/sparse.rs"
LANE_PORTABLE = "crates/gf2-kernels-simd/src/ldpc_lanes.rs"
LANE_AVX2 = "crates/gf2-kernels-simd/src/x86/ldpc_lanes.rs"
BATCH = "crates/gf2-coding/src/ldpc/batch.rs"
MIN_SUM = "crates/gf2-coding/src/ldpc/min_sum.rs"
FLOODING = "Module/Decoder/LDPC/BP/Flooding/Decoder_LDPC_BP_flooding.hxx"
ALLOCATOR_RULES = [
    ("allocator", "/library/alloc/src/alloc.rs", 0, 10**9),
    ("allocator", "/library/alloc/src/raw_vec/mod.rs", 0, 10**9),
    ("allocator", "/library/alloc/src/raw_vec.rs", 0, 10**9),
]

# Harness and comparator rules. A harness path is its suffix from the issue
# directory on, which every location of the harness sources shares.
HARNESS_RULES = [
    ("conversion-output", "/3be770d5/survey/harness/src/gf2.rs", 0, 10**9),
    ("dispatch", "/3be770d5/survey/harness/src/pool.rs", 0, 10**9),
    ("aff3ct-min-sum-update", "Tools/Code/LDPC/Update_rule/MS/Update_rule_MS.hxx", 0, 10**9),
    ("aff3ct-min-sum-update", "Tools/Code/LDPC/Update_rule/NMS/Update_rule_NMS.hxx", 0, 10**9),
    ("aff3ct-syndrome", "Tools/Code/LDPC/Syndrome/LDPC_syndrome.hxx", 0, 10**9),
    ("aff3ct-syndrome", "Module/Decoder/LDPC/BP/Decoder_LDPC_BP.hxx", 0, 10**9),
    ("aff3ct-check-node", FLOODING, 234, 257),
    ("aff3ct-variable-node", FLOODING, 205, 230),
    ("aff3ct-posterior", FLOODING, 261, 282),
    ("aff3ct-decode-loop", FLOODING, 0, 10**9),
]
# The comparator shim, reached through a relative include. STEADY_RULES omits
# it: the steady-state summaries record its samples under `other:`.
SHIM_RULES = [("conversion-output", "/c077a88b/survey/harness/cpp/aff3ct_shim.cpp", 0, 10**9)]
# The steady-state profile's executable (3be770d5).
STEADY_RULES = ALLOCATOR_RULES + [
    ("edge-position-search", CORE, 1290, 1295),
    ("edge-position-search", CORE, 1298, 1305),
    ("message-reset", CORE, 1331, 1341),
    ("check-node-loop", CORE, 1143, 1263),
    ("variable-node-update", CORE, 1268, 1287),
    ("syndrome-termination", CORE, 1372, 1378),
    ("syndrome-termination", CORE, 134, 143),
    ("syndrome-termination", SPARSE, 1367, 1380),
    ("syndrome-termination", SPARSE, 1721, 1723),
    ("decode-loop-other", CORE, 1328, 1369),
    ("min-sum-input-vec", LLR, 392, 392),
    ("min-sum-dispatch", LLR, 377, 400),
    ("min-sum-reduction", LLR, 403, 415),
    ("min-sum-reduction", LLR, 491, 494),
    ("min-sum-dispatch", SIMD_LLR, 89, 91),
    ("min-sum-reduction", SIMD_LLR, 119, 181),
] + HARNESS_RULES
# The allocation-free update's re-sampled profile executable (07ca8585),
# built from 20a3c7b195e25b70a18ba7a8d9d571a1d9a30335: the layout of
# BATCH_RULES at that revision's lines.
UPDATE_RULES = ALLOCATOR_RULES + [
    ("min-sum-reduction", MIN_SUM, 65, 71),
    ("min-sum-reduction", MIN_SUM, 109, 158),
    ("check-node-loop", CORE, 1170, 1193),
    ("variable-node-update", CORE, 1200, 1221),
    ("syndrome-termination", CORE, 1230, 1245),
    ("message-reset", CORE, 1331, 1337),
    # Call sites of the decode loop: their inlined callees carry no line.
    ("check-node-loop", CORE, 1345, 1345),
    ("variable-node-update", CORE, 1346, 1346),
    ("syndrome-termination", CORE, 1348, 1348),
    ("syndrome-termination", CORE, 1355, 1355),
    ("decode-loop-other", CORE, 1323, 1373),
] + SHIM_RULES + HARNESS_RULES
# The batch decoder profile's executable (f434ddd3).
BATCH_RULES = ALLOCATOR_RULES + [
    ("lane-check-step", LANE_PORTABLE, 246, 316),
    ("lane-check-step", LANE_AVX2, 29, 45),
    ("lane-check-step", LANE_AVX2, 123, 163),
    ("lane-check-step", LANE_AVX2, 170, 203),
    ("lane-variable-step", LANE_PORTABLE, 324, 375),
    ("lane-variable-step", LANE_AVX2, 49, 88),
    ("lane-variable-step", LANE_AVX2, 218, 266),
    ("lane-transpose-in", BATCH, 376, 385),
    ("lane-transpose-out", BATCH, 430, 457),
    ("lane-syndrome", BATCH, 462, 475),
    ("lane-decode-other", BATCH, 365, 458),
    ("batch-wave-dispatch", BATCH, 285, 363),
    ("batch-wave-dispatch", BATCH, 493, 501),
    ("min-sum-reduction", MIN_SUM, 26, 32),
    ("min-sum-reduction", MIN_SUM, 96, 145),
    ("check-node-loop", CORE, 605, 621),
    ("variable-node-update", CORE, 628, 647),
    ("syndrome-termination", CORE, 656, 671),
    ("message-reset", CORE, 734, 742),
    # Call sites of the decode loop: their inlined callees carry no line.
    ("check-node-loop", CORE, 751, 751),
    ("variable-node-update", CORE, 752, 752),
    ("syndrome-termination", CORE, 754, 754),
    ("syndrome-termination", CORE, 761, 761),
    ("decode-loop-other", CORE, 725, 782),
] + SHIM_RULES + HARNESS_RULES
# Production profile: the integrated batch-wave interface carries worker counts.
INTEGRATED_RULES = [
    (category, path, first, last) for category, path, first, last in BATCH_RULES
    if path != BATCH
] + [
    ("conversion-input", "/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs", 208, 212),
    ("conversion-output", "/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs", 228, 238),
    ("batch-wave-dispatch", "/ed3d490e-zen3-ldpc-frame-simd/survey/arms/src/lib.rs", 204, 227),
    ("lane-transpose-in", BATCH, 408, 427),
    ("lane-transpose-out", BATCH, 462, 489),
    ("lane-syndrome", BATCH, 494, 507),
    ("lane-decode-other", BATCH, 397, 490),
    ("batch-wave-dispatch", BATCH, 237, 394),
    ("batch-wave-dispatch", BATCH, 525, 542),
]
# Keyed by the executable digest a series' resolution records, or for a
# resolution without one, the build identity's digest `resolve` verified.
RULES_BY_EXECUTABLE = {
    "805bdcb457c22d3f713763a29f909bad152fd28bf75eca6d0f1532bbde2eacfa": INTEGRATED_RULES,
    "2ba24e5a81aa4193ede75e51270b2c49968d8af43f942014ef323254a489f017": STEADY_RULES,
    "4349a556571af45fc6af56b4b15effde9223fee4934f6c7f264304f0fd291aa9": UPDATE_RULES,
    "647f1526ee1851bf9e0bd8b4ed94008e4b01079f6da723f74c95b27b7eb6c483": BATCH_RULES,
}
HARNESS_DSOS = ("/ldpc-profile", "/ldpc-batch-profile")


def short(function):
    """A function name without its generic or template arguments."""
    return function if function.startswith("<") else function.split("<")[0]


def categorize(chain, rules):
    """Category of an inline chain of (function, path, line), innermost first."""
    for _, path, line in chain:
        for category, suffix, first, last in rules:
            if path.endswith(suffix) and first <= line <= last:
                return category
    return "other"


def categorize_symbol(dso, symbol):
    name = symbol or ""
    if any(name.startswith(s) or name == s for s in ALLOCATOR_SYMBOLS):
        return "allocator"
    if any(s in name for s in COPY_SYMBOLS):
        return "memory-copy"
    return "other:" + pathlib.Path(dso).name


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def seed_of(identifier):
    return int(hashlib.sha256(f"3be770d5:{identifier}".encode()).hexdigest()[:8], 16)


def completed_sessions(root):
    identities, done, first = {}, set(), None
    for line in (root / "repetitions.log").read_text(encoding="utf-8").splitlines():
        fields = line.split(" ", 3)
        if len(fields) < 2 or not fields[0].startswith("rep-"):
            continue
        if fields[1] == "start":
            identity = fields[3].rsplit(" load=", 1)[0] if len(fields) > 3 else ""
            first = identity if first is None else first
            identities[fields[0]] = identity
        elif fields[1] == "done":
            if fields[0] not in identities or identities[fields[0]] != first:
                sys.exit(f"{root}: {fields[0]} is done without a start under the first session's identity")
            done.add(fields[0])
    sessions = sorted(path for path in root.glob("rep-*") if path.is_dir() and path.name in done)
    if not sessions:
        sys.exit(f"no completed session under {root}")
    return sessions


SERIES_DONE = re.compile(r"^series done \S+ sessions=(\d+)$")


def verified_sessions(root):
    """The completed sessions of a finished series: its log ends in the terminal
    `series done` record of exactly that many sessions."""
    sessions = completed_sessions(root)
    lines = (root / "repetitions.log").read_text(encoding="utf-8").splitlines()
    terminal = SERIES_DONE.match(lines[-1]) if lines else None
    if terminal is None or int(terminal.group(1)) != len(sessions):
        sys.exit(f"{root}: the log does not end in a series done record of its "
                 f"{len(sessions)} completed sessions")
    return sessions


def read_cases(root):
    cases = {}
    for line in (root / "cases.expanded.tsv").read_text().splitlines():
        kind, label, arm, bundle, code, core_arm, batch, passes, quality, *_ = line.split("\t")
        cases[(kind, label)] = {"kind": kind, "label": label, "arm": arm, "code": code,
                                "core_arm": core_arm, "batch": int(batch), "passes": int(passes)}
    return cases


def record_of(path):
    text = path.read_text(encoding="utf-8").strip()
    return json.loads(text.splitlines()[-1]) if text else None


def counters(path):
    values = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split(",")
        if len(fields) < 3 or line.startswith("#"):
            continue
        try:
            values[fields[2].removesuffix(":u")] = float(fields[0])
        except ValueError:
            values[fields[2].removesuffix(":u")] = None
    return values


def interval(values):
    median, lower, upper, coverage = median_interval(values)
    return {"median": median, "lower": lower, "upper": upper, "coverage": coverage, "n": len(values)}


# ------------------------------------------------------------------ counters

def stat_summary(root, sessions, cases):
    summary, per_session = {}, {}
    for (kind, label), case in cases.items():
        if kind != "stat":
            continue
        columns = defaultdict(list)
        for session in sessions:
            stem = session / "stat" / label
            core, memory = counters(stem.with_suffix(".core.csv")), counters(stem.with_suffix(".memory.csv"))
            statuses = [stem.with_suffix(f".{g}.status") for g in ("core", "memory")]
            if any(not path.exists() or path.read_text().strip() != "0" for path in statuses):
                continue
            runs = [record_of(stem.with_suffix(f".{g}.json")) for g in ("core", "memory")]
            if any(run is None for run in runs):
                continue
            frames = runs[0]["frames"]
            columns["ns_per_frame_per_worker"].append(sum(r["ns_per_frame_per_worker"] for r in runs) / 2)
            ratios = {
                "ipc": ("instructions", "cycles", core),
                "frontend_stall_fraction": ("stalled-cycles-frontend", "cycles", core),
                "branch_miss_fraction": ("branch-misses", "branches", core),
                "l1d_miss_fraction": ("L1-dcache-load-misses", "L1-dcache-loads", memory),
                "cache_miss_fraction": ("cache-misses", "cache-references", memory),
            }
            for name, (top, bottom, group) in ratios.items():
                if group.get(top) is not None and group.get(bottom):
                    columns[name].append(group[top] / group[bottom])
            if core.get("instructions") is not None:
                columns["instructions_per_frame"].append(core["instructions"] / frames)
            if core.get("cycles") is not None:
                columns["cycles_per_frame"].append(core["cycles"] / frames)
            columns["workers"].append(runs[0]["workers"])
        per_session[label] = columns
        summary[label] = dict(case, metrics={name: interval(values) for name, values in columns.items()
                                             if name != "workers" and len(values) >= 6},
                              workers_observed=sorted(set(columns["workers"])))
    return summary, per_session


def scaling_summary(cases, per_session):
    ratios = []
    # A baseline is the single-worker case of the same arm, code, batch and passes;
    # route pairs differ in arm and pass count, so they match on batch alone.
    stat = {label: c for (kind, label), c in cases.items() if kind == "stat"}
    labels = {(c["arm"], c["code"], c["core_arm"], c["batch"], c["passes"]): label for label, c in stat.items()}
    routes = {(c["arm"], c["code"], c["core_arm"], c["batch"]): label for label, c in stat.items()}
    for (arm, code, core_arm, batch, passes), label in sorted(labels.items()):
        base = labels.get((arm, code, "single-core", batch, passes))
        values = per_session[label].get("ns_per_frame_per_worker", [])
        base_values = per_session[base].get("ns_per_frame_per_worker", []) if base else []
        if base and base != label and len(values) >= 2 and len(base_values) >= 2:
            identifier = f"slowdown:{label}"
            estimate, lower, upper = bootstrap_ratio(values, base_values, seed_of(identifier))
            ratios.append({"id": identifier, "kind": "per-worker slowdown against one worker",
                           "numerator": label, "denominator": base, "estimate": estimate,
                           "lower": lower, "upper": upper, "seed": seed_of(identifier)})
        for numerator, denominator, kind in ROUTE_PAIRS:
            if arm != numerator:
                continue
            other = routes.get((denominator, code, core_arm, batch))
            other_values = per_session[other].get("ns_per_frame_per_worker", []) if other else []
            if other and len(values) >= 2 and len(other_values) >= 2:
                identifier = f"gap:{label}"
                estimate, lower, upper = bootstrap_ratio(values, other_values, seed_of(identifier))
                ratios.append({"id": identifier, "kind": kind,
                               "numerator": label, "denominator": other, "estimate": estimate,
                               "lower": lower, "upper": upper, "seed": seed_of(identifier)})
    return ratios


# ------------------------------------------------------------------- samples

def load_segments(binary):
    segments = []
    for line in subprocess.run(["readelf", "-lW", str(binary)], check=True, capture_output=True,
                               text=True).stdout.splitlines():
        fields = line.split()
        if fields and fields[0] == "LOAD":
            segments.append((int(fields[1], 16), int(fields[2], 16), int(fields[4], 16)))
    return segments


def symbol_table(binary):
    """Function symbols as (address, mangled name, demangled name), sorted."""
    names = {}
    for demangle in (False, True):
        command = ["nm", "--defined-only", str(binary)] + (["-C"] if demangle else [])
        for line in subprocess.run(command, check=True, capture_output=True, text=True).stdout.splitlines():
            fields = line.split(" ", 2)
            if len(fields) == 3 and fields[1].lower() in "tw":
                names.setdefault(int(fields[0], 16), {})[demangle] = fields[2]
    return sorted((address, pair.get(False), pair.get(True)) for address, pair in names.items())


def containing(table, address):
    low, high = 0, len(table)
    while low < high:
        middle = (low + high) // 2
        if table[middle][0] <= address:
            low = middle + 1
        else:
            high = middle
    return table[low - 1][1:] if low else None


def same_symbol(perf_name, nm_names):
    if not perf_name or not nm_names:
        return False
    left = RUST_HASH.sub("", perf_name)
    for name in nm_names:
        if name:
            right = RUST_HASH.sub("", name)
            if left == right or left.endswith(right) or right.endswith(left):
                return True
    return False


def to_address(offset, segments, mode):
    if mode == "offset-is-address":
        return offset
    for file_offset, virtual, size in segments:
        if file_offset <= offset < file_offset + size:
            return offset - file_offset + virtual
    return offset


def expected_digest():
    if EXPECTED_BINARY_SHA256 is not None:
        return EXPECTED_BINARY_SHA256
    return json.loads(IDENTITY.read_text())["executables"]["ldpc-profile"]


def recorded_digest(cache):
    """The executable digest a series' cached chains record, if any."""
    if not cache.exists():
        return None
    return json.loads(cache.read_text())["resolution"].get("executable_sha256")


def rules_of(resolution):
    """Location rules of the executable a resolution's chains came from."""
    digest = resolution.get("executable_sha256") or expected_digest()
    if digest not in RULES_BY_EXECUTABLE:
        sys.exit(f"no source spans for the profiling executable {digest}")
    return RULES_BY_EXECUTABLE[digest]


def resolve(offsets, recorded=None):
    """Inline chains of harness offsets, with the offset convention chosen by
    agreement between perf's symbols and the executable's symbol table. A
    series whose chains record an executable digest resolves only under it."""
    expected = recorded or expected_digest()
    if not BINARY.exists() or sha(BINARY) != expected:
        return None, {"resolved": False, "reason": "the profiled executable is not available unchanged"}
    segments, table = load_segments(BINARY), symbol_table(BINARY)
    votes = {}
    for mode in ("offset-is-address", "file-offset"):
        votes[mode] = sum(same_symbol(symbol, containing(table, to_address(offset, segments, mode)))
                          for offset, symbol in offsets.items())
    # perf reports DSO offsets as file offsets (map_ip); symbol agreement
    # confirms the convention and the file-offset reading wins a tie.
    mode = "offset-is-address" if votes["offset-is-address"] > votes["file-offset"] else "file-offset"
    addresses = {offset: to_address(offset, segments, mode) for offset in offsets}
    text = subprocess.run(["addr2line", "-a", "-f", "-i", "-C", "-e", str(BINARY)],
                          input="\n".join(f"{a:#x}" for a in addresses.values()),
                          check=True, capture_output=True, text=True).stdout.splitlines()
    chains, current = {}, None
    by_address = {address: offset for offset, address in addresses.items()}
    index = 0
    while index < len(text):
        line = text[index]
        if line.startswith("0x") and index + 1 < len(text) and not text[index + 1].startswith("0x"):
            current = by_address.get(int(line, 16))
            chains[current] = []
            index += 1
            continue
        function, location = line, text[index + 1] if index + 1 < len(text) else "??:0"
        path, _, number = location.partition(":")
        number = number.split(" ")[0]
        chains[current].append((function, path, int(number) if number.isdigit() else 0))
        index += 2
    for chain in chains.values():
        # Level 0 carries the physical function's name beside the innermost
        # location; the innermost function itself is identified by location.
        if len(chain) > 1:
            chain[0] = ("", chain[0][1], chain[0][2])
    return chains, {"resolved": True, "offset_convention": mode, "symbol_agreement": votes,
                    "addresses": len(addresses), "executable": str(BINARY.relative_to(REPO)),
                    **({"executable_sha256": expected} if recorded or EXPECTED_BINARY_SHA256 else {})}


def record_summary(root, sessions, cases, attribution=None):
    histograms, totals = {}, {}
    harness_offsets = {}
    for (kind, label), case in cases.items():
        if kind != "record":
            continue
        histograms[label], totals[label] = [], []
        for session in sessions:
            stem = session / "record" / label
            status = stem.with_suffix(".status")
            total_file = stem.with_suffix(".total.txt")
            total = TOTAL_LINE.search(total_file.read_text()) if total_file.exists() else None
            if not status.exists() or status.read_text().strip() != "0" or total is None:
                # A failed case stays visible in its session files and is
                # excluded from the pooled samples.
                continue
            counts = defaultdict(int)
            for line in stem.with_suffix(".addresses.tsv").read_text().splitlines():
                match = ADDRESS_LINE.match(line)
                if not match:
                    continue
                samples, symbol, _, dso, offset = match.groups()
                address = int(offset, 16) if offset is not None else None
                key = (dso, address, symbol)
                counts[key] += int(samples)
                if address is not None and dso.endswith(HARNESS_DSOS):
                    harness_offsets[address] = symbol
            listed = sum(counts.values())
            if int(total.group(1)) != listed:
                sys.exit(f"{stem}: {listed} listed samples against {total.group(1)} recorded")
            histograms[label].append(counts)
            totals[label].append(listed)
    cache = root / "attribution-chains.json"
    recorded = recorded_digest(cache)
    if recorded and EXPECTED_BINARY_SHA256 and recorded != EXPECTED_BINARY_SHA256:
        sys.exit(f"{cache}: chains of executable {recorded}, not of --binary-sha256 {EXPECTED_BINARY_SHA256}")
    chains, resolution = resolve(harness_offsets, recorded)
    if chains is None and cache.exists():
        stored = json.loads(cache.read_text())
        chains = {int(k): [tuple(frame) for frame in v] for k, v in stored["chains"].items()}
        resolution = dict(stored["resolution"], reused=str(cache.name))
    elif chains is not None:
        cache.write_text(json.dumps({"resolution": resolution,
                                     "chains": {str(k): v for k, v in chains.items()}}) + "\n")
    rules = rules_of(resolution) if chains is not None else []

    def category_of(dso, offset, symbol):
        if offset is None:
            # An instruction pointer perf resolved to no object: on this host
            # the kernel-space addresses its own report lists as `[k]`, which
            # `cycles:u` still samples through interrupt skid and an
            # unprivileged session cannot map. They are recorded samples and
            # count toward the case total.
            return UNMAPPED
        if dso.endswith(HARNESS_DSOS):
            chain = (chains or {}).get(offset, [])
            category = categorize(chain, rules)
            if category != "other":
                return category
            return "other:" + (pathlib.Path(chain[0][1]).name if chain else symbol or "[unknown]")
        return categorize_symbol(dso, symbol)

    summary, rows = {}, defaultdict(int)
    for label, sessions_counts in histograms.items():
        pooled_total = sum(totals[label])
        if not pooled_total:
            summary[label] = dict(cases[("record", label)], sessions=0, total_samples=0,
                                  session_totals=[], categories=[], top_innermost=[])
            continue
        by_category = defaultdict(int)
        per_session = []
        lines = defaultdict(int)
        for counts, total in zip(sessions_counts, totals[label]):
            session_categories = defaultdict(int)
            for (dso, offset, symbol), samples in counts.items():
                category = category_of(dso, offset, symbol)
                by_category[category] += samples
                session_categories[category] += samples
                rows[(dso, offset, symbol, category)] += samples
                chain = (chains or {}).get(offset) if offset is not None and dso.endswith(HARNESS_DSOS) else None
                inner = (f"{pathlib.Path(chain[0][1]).name}:{chain[0][2]} in {short(chain[1][0] if len(chain) > 1 else chain[0][0])}"
                         if chain else f"{symbol} ({pathlib.Path(dso).name})")
                lines[inner] += samples
            per_session.append((total, session_categories))
        categories = []
        for category, samples in sorted(by_category.items(), key=lambda item: -item[1]):
            lower, upper = wilson_interval(samples, pooled_total)
            shares = [c.get(category, 0) / t for t, c in per_session if t]
            categories.append({"category": category, "samples": samples, "share": samples / pooled_total,
                               "wilson_lower": lower, "wilson_upper": upper,
                               "session_min": min(shares), "session_max": max(shares)})
        top = []
        for line, samples in sorted(lines.items(), key=lambda item: -item[1])[:25]:
            lower, upper = wilson_interval(samples, pooled_total)
            top.append({"innermost": line, "samples": samples, "share": samples / pooled_total,
                        "wilson_lower": lower, "wilson_upper": upper})
        summary[label] = dict(cases[("record", label)], sessions=len(sessions_counts),
                              total_samples=pooled_total, session_totals=totals[label],
                              categories=categories, top_innermost=top)
    with (attribution or root / "attribution.tsv").open("w", encoding="utf-8") as output:
        output.write("samples\tdso\toffset\tsymbol\tcategory\tinline chain (innermost first)\n")
        for (dso, offset, symbol, category), samples in sorted(rows.items(), key=lambda item: -item[1]):
            chain = (chains or {}).get(offset, []) if offset is not None and dso.endswith(HARNESS_DSOS) else []
            text = " <- ".join(f"{short(f)} {pathlib.Path(p).name}:{n}".strip() for f, p, n in chain)
            where = f"{offset:#x}" if offset is not None else "unmapped"
            output.write(f"{samples}\t{pathlib.Path(dso).name}\t{where}\t{symbol}\t{category}\t{text}\n")
    return summary, resolution


# -------------------------------------------------------------------- census

def census_summary(sessions):
    path = sessions[0] / "census.jsonl"
    if not path.exists():
        return None
    structure = json.loads(STRUCTURE.read_text())["codes"]
    codes = defaultdict(list)
    for line in path.read_text().splitlines():
        record = json.loads(line)
        codes[record["code"]].append(record)
    summary = {}
    for code, records in codes.items():
        edges = structure[CODE_KEYS[code]]["edges"]
        predicted = [edges * r["iterations"] + 2 * (r["iterations"] + 1) for r in records]
        summary[code] = {
            "frames": len(records),
            "iterations": [r["iterations"] for r in records],
            "allocations": [r["allocations"] for r in records],
            "predicted_allocations": predicted,
            "matches_edges_per_iteration_plus_two_syndrome_vectors": predicted == [r["allocations"] for r in records],
            "allocations_equal_deallocations": all(r["allocations"] == r["deallocations"] for r in records),
            "reallocations": sum(r["reallocations"] for r in records),
            "bytes_requested": sum(r["bytes_requested"] for r in records),
        }
    return summary


# ------------------------------------------------------------------ markdown

def pct(value):
    return f"{100 * value:.2f}%"


def markdown(summary):
    out = ["# Steady-state LDPC profile", "", "> **Diátaxis Type:** Reference", "",
           f"Generated by `dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/summarize-profile.py` from `{summary['source']}`: "
           f"n = {summary['sessions']} completed profile sessions, each under its own "
           "`dev/scripts/ccx1-bench-flock.sh --full-host` invocation. Methods are in the generator's "
           "docstring; `attribution.tsv` lists every resolved address.", ""]
    resolution = summary["resolution"]
    out += [f"Address resolution: {json.dumps(resolution)}.", ""]
    out += ["## Sampled shares by category", ""]
    for label, case in summary["records"].items():
        out += [f"### `{label}` ({case['arm']}, {case['code']}, {case['core_arm']}; "
                f"N = {case['total_samples']} samples over {case['sessions']} sessions)", "",
                "| Category | Samples | Share | Wilson 95% | Per-session share (range) |",
                "|---|---:|---:|---|---|"]
        for row in case["categories"]:
            out.append(f"| {row['category']} | {row['samples']} | {pct(row['share'])} | "
                       f"[{pct(row['wilson_lower'])}, {pct(row['wilson_upper'])}] | "
                       f"{pct(row['session_min'])}-{pct(row['session_max'])} |")
        out += ["", "| Innermost frame | Samples | Share | Wilson 95% |", "|---|---:|---:|---|"]
        for row in case["top_innermost"]:
            out.append(f"| `{row['innermost'].replace('|', '/')}` | {row['samples']} | {pct(row['share'])} | "
                       f"[{pct(row['wilson_lower'])}, {pct(row['wilson_upper'])}] |")
        out.append("")
    out += ["## Counters and time per frame", "",
            "Median over sessions [order-statistic interval, coverage in the last column].", "",
            "| Case | Workers | ns per frame per worker | IPC | Front-end stall / cycle | "
            "Branch miss / branch | L1d miss / load | cache-misses / cache-references | "
            "Instructions per frame | Coverage |", "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for label, case in summary["stats"].items():
        metrics = case["metrics"]

        def cell(name, digits):
            value = metrics.get(name)
            if not value:
                return "n/a"
            return f"{value['median']:.{digits}f} [{value['lower']:.{digits}f}, {value['upper']:.{digits}f}]"
        coverage = next(iter(metrics.values()))["coverage"] if metrics else 0
        out.append(f"| `{label}` | {','.join(map(str, case['workers_observed']))} | "
                   f"{cell('ns_per_frame_per_worker', 0)} | {cell('ipc', 3)} | "
                   f"{cell('frontend_stall_fraction', 4)} | {cell('branch_miss_fraction', 5)} | "
                   f"{cell('l1d_miss_fraction', 4)} | {cell('cache_miss_fraction', 4)} | "
                   f"{cell('instructions_per_frame', 0)} | {coverage:.3f} |")
    out += ["", "## Ratios", "",
            f"Ratio of medians over sessions with a {BOOTSTRAP_RESAMPLES}-draw percentile bootstrap "
            f"({BOOTSTRAP_GENERATOR}, seed per ratio).", "",
            "| Ratio | Kind | Estimate | 95% interval | Seed |", "|---|---|---:|---|---:|"]
    for row in summary["ratios"]:
        out.append(f"| `{row['id']}` | {row['kind']} | {row['estimate']:.3f} | "
                   f"[{row['lower']:.3f}, {row['upper']:.3f}] | {row['seed']} |")
    if summary["census"]:
        out += ["", "## Allocation census", "",
                "| Code | Frames | Allocations match edges x iterations + 2 x (iterations + 1) | "
                "Allocations equal deallocations | Reallocations |", "|---|---:|---|---|---:|"]
        for code, row in summary["census"].items():
            out.append(f"| {code} | {row['frames']} | "
                       f"{row['matches_edges_per_iteration_plus_two_syndrome_vectors']} | "
                       f"{row['allocations_equal_deallocations']} | {row['reallocations']} |")
    return "\n".join(out) + "\n"


# ------------------------------------------------------------------ self-test

SHAPES = [
    ("12\tsome::symbol+0x1a (/opt/ldpc-profile+0x7bb26)",
     ("12", "some::symbol", "1a", "/opt/ldpc-profile", "7bb26")),
    ("3\t[unknown] (/usr/lib/libc.so.6+0x176470)",
     ("3", "[unknown]", None, "/usr/lib/libc.so.6", "176470")),
    ("7\t[unknown] ([unknown])",
     ("7", "[unknown]", None, "[unknown]", None)),
]

# One inline-chain sample per batch decoder profile category, on a line of its
# span; the library frame ahead of it passes the decision outward.
SPAN_SAMPLES = [
    ("lane-check-step", LANE_PORTABLE, 300),
    ("lane-check-step", LANE_AVX2, 150),
    ("lane-variable-step", LANE_PORTABLE, 350),
    ("lane-variable-step", LANE_AVX2, 240),
    ("lane-transpose-in", BATCH, 380),
    ("lane-transpose-out", BATCH, 440),
    ("lane-syndrome", BATCH, 468),
    ("lane-decode-other", BATCH, 391),
    ("batch-wave-dispatch", BATCH, 323),
    ("min-sum-reduction", MIN_SUM, 29),
    ("min-sum-reduction", MIN_SUM, 129),
    ("check-node-loop", CORE, 608),
    ("variable-node-update", CORE, 640),
    ("syndrome-termination", CORE, 658),
    ("message-reset", CORE, 738),
    ("variable-node-update", CORE, 752),
    ("decode-loop-other", CORE, 770),
]
UPDATE_SPAN_SAMPLES = [
    ("min-sum-reduction", MIN_SUM, 68),
    ("min-sum-reduction", MIN_SUM, 142),
    ("check-node-loop", CORE, 1173),
    ("variable-node-update", CORE, 1216),
    ("syndrome-termination", CORE, 1238),
    ("message-reset", CORE, 1336),
    ("check-node-loop", CORE, 1345),
    ("variable-node-update", CORE, 1346),
    ("syndrome-termination", CORE, 1355),
    ("decode-loop-other", CORE, 1365),
]
# Harness chains of builds before and after the move under the epic directory.
HARNESS_SAMPLES = [
    ("conversion-output", "/w/dev/active/3be770d5/survey/harness/src/gf2.rs", 63),
    ("dispatch", "/w/dev/active/3be770d5/survey/harness/src/pool.rs", 40),
    ("conversion-output", "/w/dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/harness/src/gf2.rs", 63),
    ("dispatch", "/w/dev/active/1a379447-zen3-cpu-performance/3be770d5/survey/harness/src/pool.rs", 40),
    ("other", "/w/dev/active/3be770d5/survey/harness/src/bin/ldpc-profile.rs", 40),
]


def write_case(root, label, listing, recorded, log="rep-01 start 0 x load=[0]\nrep-01 done 0 load=[0]\n"):
    """A one-session profile directory carrying one record case."""
    (root / "repetitions.log").write_text(log + "series done 0 sessions=1\n")
    (root / "cases.expanded.tsv").write_text(
        "\t".join(["record", label, "gf2", "bundle", "dvb-t2-r12", "single-core", "8", "1", "quality"]) + "\n")
    stem = root / "rep-01" / "record" / label
    stem.parent.mkdir(parents=True, exist_ok=True)
    stem.with_suffix(".status").write_text("0\n")
    stem.with_suffix(".total.txt").write_text(f"              SAMPLE events:{recorded:>11}  (99.4%)\n")
    stem.with_suffix(".addresses.tsv").write_text("".join(line + "\n" for line in listing))


def self_test():
    """Parsing shapes and the total rule, against synthesized listings."""
    global BINARY, EXPECTED_BINARY_SHA256
    for line, expected in SHAPES:
        groups = ADDRESS_LINE.match(line)
        groups = groups.groups() if groups else None
        if groups != expected:
            sys.exit(f"self-test: {line!r} parsed as {groups}, expected {expected}")
    shim = "/w/dev/active/3be770d5/survey/harness/../../../c077a88b/survey/harness/cpp/aff3ct_shim.cpp"
    for rules, samples in ((BATCH_RULES, SPAN_SAMPLES), (UPDATE_RULES, UPDATE_SPAN_SAMPLES),
                           *((rules, HARNESS_SAMPLES) for rules in RULES_BY_EXECUTABLE.values()),
                           *((rules, [(category, shim, 9)]) for rules, category in
                             ((STEADY_RULES, "other"), (UPDATE_RULES, "conversion-output"),
                              (BATCH_RULES, "conversion-output")))):
        for category, path, line in samples:
            got = categorize([("core::iter::next", "/library/core/src/iter/mod.rs", 1), ("f", path, line)], rules)
            if got != category:
                sys.exit(f"self-test: {path}:{line} categorized as {got}, expected {category}")
    # The same executable name and chain take the spans of the recorded digest.
    chain = [["", CORE, 0], ["decode_codeword_into", CORE, 1346]]
    digests = {id(rules): digest for digest, rules in RULES_BY_EXECUTABLE.items()}
    for digest, expected in ((digests[id(UPDATE_RULES)], "variable-node-update"),
                             (digests[id(STEADY_RULES)], "decode-loop-other"), ("0" * 64, None)):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            BINARY = root / "absent-executable"
            write_case(root, "gf2-dvb-w1", ["5\tdecode+0x1 (/opt/ldpc-profile+0x10)"], 5)
            (root / "attribution-chains.json").write_text(json.dumps(
                {"resolution": {"resolved": True, "executable_sha256": digest}, "chains": {"16": chain}}))
            try:
                summary, _ = record_summary(root, completed_sessions(root), read_cases(root),
                                            root / "listing.tsv")
            except SystemExit as stop:
                if expected is not None:
                    raise
                print(f"self-test: an executable without spans stops the summary ({stop})")
                continue
            got = [row["category"] for row in summary["gf2-dvb-w1"]["categories"]]
            if expected is None or got != [expected]:
                sys.exit(f"self-test: executable {digest[:8]} categorized the chain as {got}, expected {expected}")
            if (root / "attribution.tsv").exists() or not (root / "listing.tsv").exists():
                sys.exit("self-test: the address listing ignored its named path")
    listing = [line for line, _ in SHAPES]
    label = "gf2-dvb-w1"
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        # An executable that does not exist leaves every chain unresolved, so
        # the check runs on the listing alone.
        BINARY = root / "absent-executable"
        write_case(root, label, listing, 22)
        cases = read_cases(root)
        summary, _ = record_summary(root, completed_sessions(root), cases)
        case = summary[label]
        shares = {row["category"]: row["samples"] for row in case["categories"]}
        if case["total_samples"] != 22 or sum(shares.values()) != 22:
            sys.exit(f"self-test: pooled {case['total_samples']} samples over categories {shares}")
        if shares.get(UNMAPPED) != 7:
            sys.exit(f"self-test: the unmapped shape contributed {shares.get(UNMAPPED)} samples, expected 7")
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        BINARY = root / "absent-executable"
        # Unresolved harness samples keep their symbol; other objects' samples name the object.
        write_case(root, label, ["5\tlane::kernel+0x1 (/opt/ldpc-batch-profile+0x10)"], 5)
        summary, _ = record_summary(root, completed_sessions(root), read_cases(root))
        got = [row["category"] for row in summary[label]["categories"]]
        if got != ["other:lane::kernel"]:
            sys.exit(f"self-test: a batch profiler sample categorized as {got}")
    cases = {("stat", f"{arm}-w1"): {"arm": arm, "code": "c", "core_arm": "single-core", "batch": 8, "passes": 1}
             for arm in ("lane", "single-frame")}
    per_session = {f"{arm}-w1": {"ns_per_frame_per_worker": [float(v) for v in values]}
                   for arm, values in (("lane", (2, 2, 2)), ("single-frame", (4, 4, 4)))}
    pairs = [(r["numerator"], r["denominator"], r["estimate"]) for r in scaling_summary(cases, per_session)]
    if pairs != [("lane-w1", "single-frame-w1", 0.5)]:
        sys.exit(f"self-test: route pairs {pairs}, expected lane-w1 over single-frame-w1 at 0.5")
    shape = {"arm": "a", "code": "c", "passes": 1}
    cases = {("stat", label): dict(shape, core_arm=core_arm, batch=batch)
             for label, core_arm, batch in (("w1-b1", "single-core", 1), ("w1-b8", "single-core", 8),
                                            ("p6-b8", "physical-cores-6", 8), ("p6-b4", "physical-cores-6", 4))}
    per_session = {label: {"ns_per_frame_per_worker": [float(v)] * 3}
                   for label, v in (("w1-b1", 8), ("w1-b8", 2), ("p6-b8", 6), ("p6-b4", 5))}
    pairs = [(r["numerator"], r["denominator"], r["estimate"]) for r in scaling_summary(cases, per_session)]
    if pairs != [("p6-b8", "w1-b8", 3.0)]:
        sys.exit(f"self-test: scaling pairs {pairs}, expected p6-b8 over its own batch's w1-b8 at 3.0 "
                 "and no ratio for the batch without a single-worker case")
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        BINARY = root / "absent-executable"
        # One sample of the listing withheld: the rule is equality, so this
        # stops the summary rather than reporting shares over 22 of 23.
        write_case(root, label, listing, 23)
        try:
            record_summary(root, completed_sessions(root), read_cases(root))
        except SystemExit as stop:
            print(f"self-test: the total rule rejects an incomplete listing ({stop})")
        else:
            sys.exit("self-test: a listing short of its recorded total was accepted")
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        write_case(root, label, listing, 22, log="rep-00 start 0 x load=[0]\nrep-00 done 0 load=[0]\n"
                                                 "rep-01 start 0 y load=[0]\nrep-01 done 0 load=[0]\n")
        try:
            completed_sessions(root)
        except SystemExit as stop:
            print(f"self-test: the log rule rejects a session under another identity ({stop})")
        else:
            sys.exit("self-test: a session done under another identity was accepted")
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        write_case(root, label, listing, 22, log="rep-01 done 0 load=[0]\n")
        try:
            completed_sessions(root)
        except SystemExit as stop:
            print(f"self-test: the log rule rejects a session done without a start ({stop})")
        else:
            sys.exit("self-test: a session done without a start was accepted")
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory)
        BINARY = root / "absent-executable"
        write_case(root, label, listing, 22)
        cache = root / "attribution-chains.json"
        cache.write_text(json.dumps({"resolution": {"resolved": True, "executable_sha256": "1" * 64},
                                     "chains": {}}))
        before = cache.read_bytes()
        EXPECTED_BINARY_SHA256 = "2" * 64
        try:
            record_summary(root, completed_sessions(root), read_cases(root))
        except SystemExit as stop:
            print(f"self-test: a --binary-sha256 other than the series' executable is refused ({stop})")
        else:
            sys.exit("self-test: chains were taken under an executable the series does not record")
        finally:
            EXPECTED_BINARY_SHA256 = None
        if cache.read_bytes() != before:
            sys.exit("self-test: a mismatching executable rewrote the series' chains")
    print("self-test: ok")


def main():
    global BINARY, EXPECTED_BINARY_SHA256
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", type=pathlib.Path)
    parser.add_argument("--json", type=pathlib.Path)
    parser.add_argument("--markdown", type=pathlib.Path)
    parser.add_argument("--attribution", type=pathlib.Path)
    parser.add_argument("--binary", type=pathlib.Path)
    parser.add_argument("--binary-sha256")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.root is None or args.json is None or args.markdown is None:
        parser.error("root, --json and --markdown are required")
    if (args.binary is None) != (args.binary_sha256 is None):
        parser.error("--binary and --binary-sha256 must be supplied together")
    if args.binary is not None:
        if not re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256):
            parser.error("--binary-sha256 must be 64 lowercase hexadecimal characters")
        BINARY = args.binary.resolve()
        EXPECTED_BINARY_SHA256 = args.binary_sha256
    root = args.root.resolve()
    sessions = completed_sessions(root)
    cases = read_cases(root)
    stats, per_session = stat_summary(root, sessions, cases)
    records, resolution = record_summary(root, sessions, cases, args.attribution)
    summary = {
        "schema": "ldpc-steady-profile-summary-v1",
        "source": str(root.relative_to(REPO)) if root.is_relative_to(REPO) else str(root),
        "sessions": len(sessions),
        "resolution": resolution,
        "records": records,
        "stats": stats,
        "ratios": scaling_summary(cases, per_session),
        "census": census_summary(sessions),
    }
    args.json.write_text(json.dumps(summary, indent=2) + "\n")
    args.markdown.write_text(markdown(summary))


if __name__ == "__main__":
    main()
