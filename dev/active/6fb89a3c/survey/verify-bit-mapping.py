#!/usr/bin/env python3
"""Independent bit-mapping gate for the external comparison arms."""
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
    a = splitmix(case["seed"])
    b = splitmix(case["seed"] + 1)
    result = [next(a) ^ next(b) for _ in range(case["words"])]
    # One line, matching the harness's --dump-check output convention (and
    # check_family's list-of-lines comparison, shared with transpose_case).
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
            if unavailable_ok and any(marker in stderr.lower() for marker in ("cannot represent", "unavailable without padding", "multiple of 8")):
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
    """Reads (n, k) for `name` from the pre-existing, committed generator
    dump `dev/bench_results/4e732b56/generators.txt` (jit:4e732b56), which
    this issue reuses read-only rather than recomputing (@/inv/single-source-prose)."""
    for line in GENERATORS_PATH.read_text().splitlines():
        fields = line.split()
        if len(fields) == 5 and fields[0] == name:
            return int(fields[1]), int(fields[2])
    raise AssertionError(f"code {name!r} is absent from {GENERATORS_PATH}")


def rref(n: int, rows: list[str]) -> tuple[int, tuple[int, ...]]:
    """Canonical GF(2) reduced row echelon form under the given column order.

    Ported from `dev/active/4e732b56/baseline-survey/verify-generator-matrices.py`
    (jit:4e732b56), read-only reuse of an established validation technique
    (@/inv/convention-convergence), not a private reimplementation: two
    full-rank generator matrices span the same code exactly when their
    canonical RREFs under one shared column order are identical.
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


def check_genmatrix_row_space(name: str) -> None:
    """Validates that gf2's `bch_generator_matrix_by_encoding` (the
    established oracle `crates/gf2-coding/benches/bch_genmatrix.rs` already
    compares against, `SystematicLayout::MessageParityAscending`, the
    default) and M4RI's `genmatrix-rref` route (`SystematicLayout`-equivalent
    to `MessageParityDescending`, "repository column order", per
    `crates/gf2-coding/src/bch/encode.rs`'s documented coordinate formulas)
    span the same code, i.e. compute operation-equivalent generator matrices
    of code `name` in two different but individually well-defined systematic
    coordinate layouts.

    Raw bit-for-bit dumps of the two routes do NOT match directly (this was
    checked and falsified: their row bases differ before reduction, and their
    column layouts are related by a cyclic rotation vs. a reversal, not by a
    simple shared permutation of the raw dumps). The correct operation-
    equivalence criterion, reused from the established
    `verify-generator-matrices.py` methodology, is row-space equality: reindex
    each raw dump from its own layout into a shared internal-degree column
    order (`SystematicLayout`'s own documented formulas: gf2 raw column u
    carries degree (u + n - k) mod n; M4RI raw column c carries degree
    n - 1 - c), then compare canonical GF(2) RREF under that shared order.
    """
    n, k = read_code_dims(name)
    if not GF2_DUMP_CHECK.exists():
        raise RuntimeError(f"missing binary {GF2_DUMP_CHECK}")
    gf2_code, m4ri_binary = subprocess.run(
        [str(GF2_DUMP_CHECK), "bch-genmatrix", name], cwd=ROOT.parents[3],
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


def bitshuffle_mapping(cases: list[dict]) -> tuple[str, list[dict], dict]:
    candidates = [None, "input-byte-reverse", "output-byte-reverse", "both",
                  "element-byte-reverse", "input-byte-reverse+element-byte-reverse",
                  "output-byte-reverse+element-byte-reverse", "both+element-byte-reverse"]
    results = {}
    direct_failure = None
    for candidate in candidates:
        try:
            unavailable = check_family("bitshuffle_transpose_arm", cases, transpose_case, candidate, True)
            results[candidate or "direct"] = (True, unavailable, None)
        except AssertionError as error:
            results[candidate or "direct"] = (False, [], str(error))
            if candidate is None:
                direct_failure = str(error)
    passing = [name for name, (ok, unavailable, _) in results.items() if ok and not unavailable]
    chosen = passing[0] if passing else None
    return chosen or "no candidate", [], results | {"direct_failure": direct_failure}


def write_bitshuffle_report(cases: list[dict], chosen: str, details: dict) -> None:
    fixed = {"seed": 1}
    expected = transpose_case(fixed)
    direct_code, direct_output, direct_stderr = run_dump(ROOT / "bitshuffle_transpose_arm", fixed)
    chosen_adapter = None if chosen in ("direct", "no candidate") else chosen
    pass_code, pass_output, pass_stderr = run_dump(ROOT / "bitshuffle_transpose_arm", fixed, chosen_adapter)
    lines = [
        "# Bitshuffle bit mapping",
        "",
        "This file is generated by `verify-bit-mapping.py`. The harness emits "
        "Bitshuffle's decoded bit-plane output with no adapter in transport mode.",
        "",
    ]
    if details.get("direct_failure"):
        lines += ["The direct mapping fails:", "", f"`{details['direct_failure']}", ""]
    if chosen == "no candidate":
        lines += ["No tested adapter matches the reference mapping.", ""]
    else:
        lines += [f"The smallest passing adapter is `{chosen}`.", ""]
    # A fixed seed gives a compact, reproducible failing/passing example. The
    # full first row is enough to expose byte/bit order and remains a bit string.
    lines += ["Smallest reference example (transpose-64, seed 1), first output row:", "", f"expected `{expected[0]}`", ""]
    if direct_code == 0 and direct_output:
        lines += [f"direct actual `{direct_output[0]}`", ""]
    if pass_code == 0 and pass_output and chosen not in ("direct", "no candidate"):
        lines += [f"passing `{chosen}` actual `{pass_output[0]}`", ""]
    if direct_code != 0:
        lines += [f"direct dump command exits {direct_code}: `{direct_stderr}`", ""]
    if pass_code != 0 and chosen not in ("direct", "no candidate"):
        lines += [f"candidate dump command exits {pass_code}: `{pass_stderr}`", ""]
    lines += ["The report records the adapter search performed by this script; unavailable geometries are not padded or substituted.", ""]
    (ROOT / "bitshuffle-bit-mapping.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    transpose_fixed = [{"seed": seed} for seed in (1, 0x123456789ABCDEF0, 0xDEADBEEFCAFEBABE)]
    transpose_tiled = [{"rows": rows, "cols": cols, "seed": 7} for rows, cols in ((63, 63), (64, 64), (65, 65), (1, 64), (64, 1))]
    xor_cases = [
        {"words": words, "seed": 9, "alignment_bytes": 32}
        for words in (1, 7, 8, 9, 16, 32, 63, 64, 65)
    ]
    try:
        if not (ROOT / "bitshuffle_transpose_arm").exists():
            (ROOT / "bitshuffle-bit-mapping.md").write_text(
                "# Bitshuffle bit mapping\n\n"
                "This file is generated by `verify-bit-mapping.py`. The mapping search "
                "does not run because `bitshuffle_transpose_arm` is unavailable until "
                "the pinned external checkout builds. No adapter result is asserted.\n",
                encoding="utf-8",
            )
        unavailable_m4ri = check_family("m4ri_transpose_arm", transpose_fixed, transpose_case)
        print("PASS m4ri_transpose_arm transpose-64 (3 seeds)")
        unavailable_m4ri += check_family("m4ri_transpose_arm", transpose_tiled, transpose_case)
        print("PASS m4ri_transpose_arm transpose-tiled (5 dimensions)")
        chosen, _, details = bitshuffle_mapping(transpose_fixed)
        write_bitshuffle_report(transpose_fixed + transpose_tiled, chosen, details)
        if chosen == "no candidate":
            raise AssertionError("Bitshuffle has no matching tested adapter; see bitshuffle-bit-mapping.md")
        print(f"PASS bitshuffle_transpose_arm transpose mapping ({chosen})")
        unavailable_bitshuffle = check_family(
            "bitshuffle_transpose_arm", transpose_tiled, transpose_case,
            None if chosen == "direct" else chosen, True)
        if unavailable_bitshuffle:
            print(f"PASS bitshuffle_transpose_arm transpose-tiled ({len(unavailable_bitshuffle)} unavailable geometries recorded by harness)")
        else:
            print("PASS bitshuffle_transpose_arm transpose-tiled (5 dimensions)")
        check_family("isal_xor_arm", xor_cases, xor_case)
        print("PASS isal_xor_arm logical-xor (9 word counts, arity 3, 32-byte alignment)")
        for code_name in ("B1", "B2", "B3"):
            check_genmatrix_row_space(code_name)
        print("PASS bch-genmatrix row-space equality (gf2 vs m4ri, B1/B2/B3)")
        report = {
            "schema": "gf2-external-comparator-correctness-v1",
            "issue": "6fb89a3c",
            "status": "pass",
            "canonical_reference": "naive bit arithmetic in verify-bit-mapping.py",
            "transpose": {
                "m4ri": {
                    "validated_fixed_seeds": len(transpose_fixed),
                    "validated_geometries": [[case["rows"], case["cols"]] for case in transpose_tiled],
                    "mapping": "output[c][r] = input[r][c]; row words use bit c = 1u64 << c",
                    "padding": "M4RI zeroes unused tail bits in partial words",
                    "aliasing": "distinct input/output; fixed output allocation is reused during kernel timing",
                },
                "bitshuffle": {
                    "validated_fixed_seeds": len(transpose_fixed),
                    "mapping": chosen,
                    "geometry": "size=64 elements, elem_size=8 bytes, block_size=64",
                    "adaptation_cost": "zero for 64x64 direct layout",
                    "unavailable_geometries": [[case["rows"], case["cols"]] for case in unavailable_bitshuffle],
                    "unavailable_reason": "element count is not a multiple of 8; no padding substitution is timed",
                    "aliasing": "distinct input/output as required by the Bitshuffle interface",
                },
            },
            "logical_xor": {
                "validated_word_counts": [case["words"] for case in xor_cases],
                "mapping": "dest[w].bit[b] = src0[w].bit[b] XOR src1[w].bit[b], LSB-first",
                "isa_l_parity_arity": {"vects": 3, "sources": 2, "outputs": 1},
                "alignment_bytes": 32,
                "arrangement_cost": "the three-pointer array is formed inside every timed call",
                "aliasing": "unavailable: ISA-L does not contractually permit destination/source aliasing",
            },
            "bch_genmatrix": {
                "validated_codes": ["B1", "B2", "B3"],
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
