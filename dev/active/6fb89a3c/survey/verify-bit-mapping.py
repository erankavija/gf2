#!/usr/bin/env python3
"""Independent bit-mapping gate for the external comparison arms.

Every arm's `--dump-check` output is compared against naive bit arithmetic
computed here from the same SplitMix64 seeds. The gate covers the canonical
64x64 transpose, whole-consumer geometries on both sides of the word
boundary, the Bitshuffle padded-geometry adapter and its unavailability
without padding, XOR/parity at several word counts and arities, and the
BCH generator-matrix row-space equivalence. It writes
`correctness-report.json` and `bitshuffle-bit-mapping.md`.
"""
from __future__ import annotations

import os
import json
import subprocess
import sys
from pathlib import Path

MASK = (1 << 64) - 1
ROOT = Path(__file__).resolve().parent
GF2_DUMP_CHECK = Path(os.environ.get(
    "GF2_SURVEY_GF2_DUMP",
    ROOT / "gf2-side" / "target" / "release" / "gf2_dump_check",
))
GENERATORS_PATH = ROOT.parents[2] / "bench_results" / "4e732b56" / "generators.txt"

TRANSPOSE_FIXED = [{"seed": seed} for seed in (1, 0x123456789ABCDEF0, 0xDEADBEEFCAFEBABE)]
TRANSPOSE_TILED = [{"rows": rows, "cols": cols, "seed": 7}
                   for rows, cols in ((63, 63), (64, 64), (65, 65), (1, 64), (64, 1), (100, 130))]
XOR_CASES = [{"words": words, "seed": 9, "alignment_bytes": 32} for words in (1, 7, 8, 9, 16, 32, 63, 64, 65)]
PARITY_CASES = [{"words": 64, "seed": 9, "alignment_bytes": 32, "sources": 3},
                {"words": 9, "seed": 9, "alignment_bytes": 32, "sources": 4}]


def splitmix(seed: int):
    while True:
        seed = (seed + 0x9E3779B97F4A7C15) & MASK
        z = seed
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
        yield (z ^ (z >> 31)) & MASK


def transpose_case(case: dict) -> list[str]:
    rng = splitmix(case["seed"])
    if "rows" not in case:
        rows, cols = 64, 64
        matrix = []
        for _ in range(rows):
            word = next(rng)
            matrix.append([word >> c & 1 for c in range(cols)])
    else:
        rows, cols = case["rows"], case["cols"]
        matrix = [[next(rng) & 1 for _ in range(cols)] for _ in range(rows)]
    return ["".join(str(matrix[r][c]) for r in range(rows)) for c in range(cols)]


def xor_case(case: dict) -> list[str]:
    streams = [splitmix(case["seed"] + s) for s in range(case.get("sources", 2))]
    result = []
    for _ in range(case["words"]):
        word = 0
        for stream in streams:
            word ^= next(stream)
        result.append(word)
    return ["".join(str((word >> bit) & 1) for word in result for bit in range(64))]


def run_dump(binary: Path, case: dict, adapter: str | None = None) -> tuple[int, list[str], str]:
    env = os.environ.copy()
    if adapter is None:
        env.pop("GF2_BITSHUFFLE_ADAPTER", None)
    else:
        env["GF2_BITSHUFFLE_ADAPTER"] = adapter
    proc = subprocess.run(
        [str(binary), "--dump-check", json.dumps(case, separators=(",", ":"))],
        cwd=ROOT.parents[3], text=True, capture_output=True, env=env,
    )
    return proc.returncode, proc.stdout.splitlines(), proc.stderr.strip()


UNAVAILABLE_MARKERS = ("cannot represent", "unavailable without padding", "multiple of 8")


def check_family(binary_name: str, cases: list[dict], expected_fn, adapter: str | None = None,
                 unavailable_ok: bool = False):
    binary = ROOT / binary_name
    if not binary.exists():
        raise RuntimeError(f"missing binary {binary}")
    unavailable = []
    for case in cases:
        expected = expected_fn(case)
        code, actual, stderr = run_dump(binary, case, adapter)
        if code != 0:
            if unavailable_ok and any(marker in stderr.lower() for marker in UNAVAILABLE_MARKERS):
                unavailable.append(case)
                continue
            raise AssertionError(f"{binary_name} case {case}: exit {code}: {stderr}")
        if actual != expected:
            limit = min(len(actual), len(expected))
            first = next((i for i in range(limit) if actual[i] != expected[i]), limit)
            raise AssertionError(
                f"{binary_name} case {case}: mismatch at row/line {first}; "
                f"expected={expected[first] if first < len(expected) else '<missing>'} "
                f"actual={actual[first] if first < len(actual) else '<missing>'}"
            )
    return unavailable


def read_code_dims(name: str) -> tuple[int, int]:
    """Reads (n, k) for `name` from the committed generator dump
    `dev/bench_results/4e732b56/generators.txt` (jit:4e732b56), reused
    read-only rather than recomputed (@/inv/single-source-prose)."""
    for line in GENERATORS_PATH.read_text().splitlines():
        fields = line.split()
        if len(fields) == 5 and fields[0] == name:
            return int(fields[1]), int(fields[2])
    raise AssertionError(f"code {name!r} is absent from {GENERATORS_PATH}")


def rref(n: int, rows: list[str]) -> tuple[int, tuple[int, ...]]:
    """Canonical GF(2) reduced row echelon form under the given column order.

    Ported from `dev/active/4e732b56/baseline-survey/verify-generator-matrices.py`
    (jit:4e732b56): two full-rank generator matrices span the same code
    exactly when their canonical RREFs under one shared column order agree.
    """
    pivots: dict[int, int] = {}
    for row in rows:
        vec = int(row, 2)
        for pivot_col, pivot_vec in pivots.items():
            if (vec >> (n - 1 - pivot_col)) & 1:
                vec ^= pivot_vec
        if not vec:
            continue
        col = n - vec.bit_length()
        for pivot_col, pivot_vec in pivots.items():
            if (pivot_vec >> (n - 1 - col)) & 1:
                pivots[pivot_col] = pivot_vec ^ vec
        pivots[col] = vec
    return len(pivots), tuple(pivots[col] for col in sorted(pivots))


def check_genmatrix_row_space(name: str, route: str) -> None:
    """Validates that gf2's `bch_generator_matrix_by_encoding`
    (`SystematicLayout::MessageParityAscending`) and M4RI's RREF of the
    shifted generator polynomial (repository column order, descending
    degree) span the same code.

    Raw dumps do not match bit for bit: the row bases differ before
    reduction and the column layouts differ (gf2 raw column u carries degree
    (u + n - k) mod n; M4RI raw column c carries degree n - 1 - c). The
    operation-equivalent observable is row-space equality after reindexing
    both dumps to internal polynomial-degree order.
    """
    n, k = read_code_dims(name)
    if not GF2_DUMP_CHECK.exists():
        raise RuntimeError(f"missing binary {GF2_DUMP_CHECK}")
    gf2_code, m4ri_binary = subprocess.run(
        [str(GF2_DUMP_CHECK), "bch-genmatrix", name, route], cwd=ROOT.parents[3],
        text=True, capture_output=True, check=True,
    ).stdout.splitlines(), ROOT / "m4ri_genmatrix_arm"
    m4ri_proc = subprocess.run(
        [str(m4ri_binary), "--dump-check", json.dumps({"code": name, "seed": 1}, separators=(",", ":"))],
        cwd=ROOT.parents[3], text=True, capture_output=True,
    )
    if m4ri_proc.returncode != 0:
        raise AssertionError(f"m4ri_genmatrix_arm --dump-check {name}: exit {m4ri_proc.returncode}: {m4ri_proc.stderr.strip()}")
    m4ri_rows = m4ri_proc.stdout.splitlines()
    if len(gf2_code) != k or any(len(row) != n for row in gf2_code):
        raise AssertionError(f"gf2 dump for {name} has unexpected shape: {len(gf2_code)} rows")
    if len(m4ri_rows) != k or any(len(row) != n for row in m4ri_rows):
        raise AssertionError(f"m4ri dump for {name} has unexpected shape: {len(m4ri_rows)} rows")
    gf2_by_degree = ["".join(row[(d + k) % n] for d in range(n)) for row in gf2_code]
    m4ri_by_degree = ["".join(row[n - 1 - d] for d in range(n)) for row in m4ri_rows]
    gf2_rref = rref(n, gf2_by_degree)
    m4ri_rref = rref(n, m4ri_by_degree)
    if gf2_rref != m4ri_rref or gf2_rref[0] != k:
        raise AssertionError(
            f"genmatrix {name}: row spaces disagree (gf2 rank {gf2_rref[0]}, m4ri rank {m4ri_rref[0]}, "
            f"equal={gf2_rref == m4ri_rref}); the two routes are not computing the same code"
        )
    if gf2_code == m4ri_rows:
        raise AssertionError(f"genmatrix {name}: raw dumps unexpectedly identical; the layout premise changed")


def bitshuffle_mapping(cases: list[dict]) -> tuple[str, dict]:
    """Searches byte/bit-order permutations for the fixed 64x64 mapping; the
    direct mapping is expected to be the smallest passing one."""
    candidates = [None, "input-byte-reverse", "output-byte-reverse", "both",
                  "element-byte-reverse", "input-byte-reverse+element-byte-reverse",
                  "output-byte-reverse+element-byte-reverse", "both+element-byte-reverse"]
    results = {}
    direct_failure = None
    for candidate in candidates:
        try:
            check_family("bitshuffle_transpose_arm", cases, transpose_case, candidate, False)
            results[candidate or "direct"] = "pass"
        except AssertionError as error:
            results[candidate or "direct"] = "fail"
            if candidate is None:
                direct_failure = str(error)
    passing = [name for name, verdict in results.items() if verdict == "pass"]
    chosen = passing[0] if passing else "no candidate"
    return chosen, results | {"direct_failure": direct_failure}


def write_bitshuffle_report(chosen: str, details: dict, unavailable: list[dict], adapted: list[dict]) -> None:
    fixed = {"seed": 1}
    expected = transpose_case(fixed)
    direct_code, direct_output, direct_stderr = run_dump(ROOT / "bitshuffle_transpose_arm", fixed)
    lines = [
        "# Bitshuffle bit mapping",
        "",
        "This file is generated by `verify-bit-mapping.py`. The harness emits "
        "Bitshuffle's decoded bit-plane output with no adapter in transport mode.",
        "",
        f"Byte/bit-order permutation search over the fixed 64x64 case: {json.dumps({k: v for k, v in details.items() if k != 'direct_failure'})}.",
        "",
    ]
    if details.get("direct_failure"):
        lines += ["The direct mapping fails:", "", f"`{details['direct_failure']}`", ""]
    if chosen == "no candidate":
        lines += ["No tested permutation matches the reference mapping.", ""]
    else:
        lines += [f"The smallest passing mapping is `{chosen}`: plane `c` holds column `c`, element `r` at byte `r / 8` bit `r % 8`, LSB-first.", ""]
    lines += ["Smallest reference example (transpose-64, seed 1), first output row:", "", f"expected `{expected[0]}`", ""]
    if direct_code == 0 and direct_output:
        lines += [f"direct actual `{direct_output[0]}`", ""]
    if direct_code != 0:
        lines += [f"direct dump command exits {direct_code}: `{direct_stderr}`", ""]
    lines += [
        "## Geometry",
        "",
        "Bitshuffle requires the element count (matrix rows) to be a multiple of eight. "
        f"Without the padded adapter these geometries are unavailable: {json.dumps([[c['rows'], c['cols']] for c in unavailable])}.",
        "",
        "With `\"adapter\": \"padded\"` the arm pads rows to a multiple of eight, packs each row "
        "into `ceil(cols / 8)` bytes, transposes one Bitshuffle block and unpacks the first `cols` planes "
        f"into the canonical output; these geometries then match the naive reference: {json.dumps([[c['rows'], c['cols']] for c in adapted])}. "
        "The pack and unpack copies are skipped exactly when the canonical layout already coincides with "
        "Bitshuffle's (pack: rows % 8 == 0 and cols % 64 == 0; unpack: rows % 64 == 0 and cols % 8 == 0), "
        "so the 64x64 consumer pays no adapter copy.",
        "",
    ]
    (ROOT / "bitshuffle-bit-mapping.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    try:
        unavailable_m4ri = check_family("m4ri_transpose_arm", TRANSPOSE_FIXED, transpose_case)
        print("PASS m4ri_transpose_arm transpose-64 (3 seeds)")
        unavailable_m4ri += check_family("m4ri_transpose_arm", TRANSPOSE_TILED, transpose_case)
        print(f"PASS m4ri_transpose_arm transpose-tiled ({len(TRANSPOSE_TILED)} geometries)")
        if unavailable_m4ri:
            raise AssertionError(f"M4RI reported unavailable geometries: {unavailable_m4ri}")

        chosen, details = bitshuffle_mapping(TRANSPOSE_FIXED)
        if chosen != "direct":
            write_bitshuffle_report(chosen, details, [], [])
            raise AssertionError(f"Bitshuffle mapping is {chosen!r}, not direct; see bitshuffle-bit-mapping.md")
        print("PASS bitshuffle_transpose_arm transpose-64 mapping (direct, 3 seeds)")
        # Without the adapter the harness must refuse row counts that are
        # not multiples of eight and pass the rest unchanged.
        unavailable_bitshuffle = check_family("bitshuffle_transpose_arm", TRANSPOSE_TILED, transpose_case, None, True)
        expected_unavailable = [case for case in TRANSPOSE_TILED if case["rows"] % 8 != 0]
        if unavailable_bitshuffle != expected_unavailable:
            raise AssertionError(f"Bitshuffle unavailability differs: {unavailable_bitshuffle} vs {expected_unavailable}")
        print(f"PASS bitshuffle_transpose_arm transpose-tiled without adapter ({len(unavailable_bitshuffle)} geometries unavailable, rest exact)")
        adapted_cases = [dict(case, adapter="padded") for case in TRANSPOSE_TILED]
        if check_family("bitshuffle_transpose_arm", adapted_cases, transpose_case):
            raise AssertionError("padded adapter reported an unavailable geometry")
        print(f"PASS bitshuffle_transpose_arm transpose-tiled with padded adapter ({len(adapted_cases)} geometries)")
        write_bitshuffle_report(chosen, details, unavailable_bitshuffle, adapted_cases)

        check_family("isal_xor_arm", XOR_CASES, xor_case)
        print(f"PASS isal_xor_arm logical-xor ({len(XOR_CASES)} word counts, vects 3, 32-byte alignment)")
        check_family("isal_xor_arm", PARITY_CASES, xor_case)
        print(f"PASS isal_xor_arm parity ({[c['sources'] for c in PARITY_CASES]} sources)")
        unaligned = run_dump(ROOT / "isal_xor_arm", {"words": 8, "seed": 9, "alignment_bytes": 8})
        if unaligned[0] == 0:
            raise AssertionError("isal_xor_arm accepted an 8-byte alignment outside the xor_gen contract")
        print("PASS isal_xor_arm refuses alignment outside the documented 32-byte contract")

        for route in ("materialize", "reference"):
            for code_name in ("B1", "B2", "B3"):
                check_genmatrix_row_space(code_name, route)
            print(f"PASS bch-genmatrix row-space equality (gf2 {route} vs m4ri, B1/B2/B3)")

        report = {
            "schema": "gf2-external-comparator-correctness-v2",
            "issue": "6fb89a3c",
            "status": "pass",
            "canonical_reference": "naive bit arithmetic in verify-bit-mapping.py from the same SplitMix64 seeds",
            "transpose": {
                "m4ri": {
                    "validated_fixed_seeds": len(TRANSPOSE_FIXED),
                    "validated_geometries": [[case["rows"], case["cols"]] for case in TRANSPOSE_TILED],
                    "mapping": "output[c][r] = input[r][c]; row words use bit c = 1u64 << c",
                    "padding": "M4RI keeps excess bits of a non-window matrix zero (mzd.h policy); the dumps read only valid bits",
                    "aliasing": "distinct input/output; the kernel-isolated cell reuses a preallocated output, whole-consumer calls allocate a fresh one",
                },
                "bitshuffle": {
                    "validated_fixed_seeds": len(TRANSPOSE_FIXED),
                    "mapping": chosen,
                    "mapping_search": {k: v for k, v in details.items() if k != "direct_failure"},
                    "fixed_geometry": "size=64 elements, elem_size=8 bytes, block_size=64",
                    "unavailable_without_adapter": [[case["rows"], case["cols"]] for case in unavailable_bitshuffle],
                    "unavailable_reason": "element count (rows) is not a multiple of 8",
                    "validated_with_padded_adapter": [[case["rows"], case["cols"]] for case in adapted_cases],
                    "adapter": "rows padded to a multiple of 8 and packed into ceil(cols/8)-byte elements; one block of size rows_padded; the first cols planes are unpacked into the canonical cols x rows output; pack/unpack copies skipped when the layouts coincide",
                    "aliasing": "distinct input/output as required by the Bitshuffle interface",
                },
            },
            "logical_xor": {
                "validated_word_counts": [case["words"] for case in XOR_CASES],
                "validated_parity_arities": [{"words": c["words"], "sources": c["sources"], "vects": c["sources"] + 1} for c in PARITY_CASES],
                "mapping": "dest[w].bit[b] = XOR over sources of src[s][w].bit[b], LSB-first",
                "isa_l_parity_arity": "vects = sources + 1; two sources (vects 3) in the sized cells, three sources (vects 4) in the arity cell",
                "alignment_bytes": 32,
                "unaligned": "unavailable: raid.h requires source and destination pointers aligned to 32 bytes; the harness refuses other alignments",
                "arrangement_cost": "the pointer array is formed inside every timed call and one formation is reported separately as dispatch_ns; the gf2 fresh-destination copy is inside every timed call and reported separately as pack_ns",
                "aliasing": "unavailable: raid.h names distinct source pointers and one destination pointer; gf2's dst ^= src accumulate has no ISA-L equivalent",
            },
            "bch_genmatrix": {
                "validated_codes": ["B1", "B2", "B3"],
                "validated_gf2_routes": ["materialize (BchCode::generator_matrix, production)", "reference (bch_generator_matrix_by_encoding, test-support oracle)"],
                "equivalence": "full-rank row-space equality after both documented layouts are reindexed to internal polynomial-degree order",
                "raw_dump_identity": "falsified; bases and systematic column layouts differ",
            },
        }
        (ROOT / "correctness-report.json").write_text(
            json.dumps(report, indent=2) + "\n", encoding="utf-8"
        )
        return 0
    except (AssertionError, RuntimeError, OSError) as error:
        print(f"FAIL verify-bit-mapping.py: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
