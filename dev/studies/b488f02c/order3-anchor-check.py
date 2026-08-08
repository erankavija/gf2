#!/usr/bin/env python3
"""Receipt for the order-3 sampling anchor cited in the feasibility study §4.7.

The anchor answers a gap the other checks leave open. Cross-backend equivalence
compares backends against each other, and every backend draws from the same
sampler, so neither can detect a sampler that biases the statistic under study.
The anchor closes it by estimating a quantity whose exact value is known:
Pr[per(A) = 0] for a uniform 3x3 matrix over F_q, which is computable by
enumerating all q^9 matrices.

This script does two things and keeps them separate.

1. It derives the exact value itself, by enumeration in Python, independently of
   the harness and of gf2-algebra. That is the ground truth the anchor is
   measured against, and it is checkable here without trusting any Rust code.
   For q = 3 it must reproduce [Scheinerman2024] Table 3's z(3) = 8163.

2. It runs the harness's own anchor test, which draws 400 000 order-3 matrices
   per field through the campaign's ChaCha20 sampler and its packed kernels and
   asserts the estimate lands within 4 sigma of the exact value. The harness is
   the thing under test, so the check must run its code rather than a
   reimplementation: a Python re-draw would validate Python.

3. It runs `anchor-report`, a standalone crate beside this script that links the
   harness as a library and reproduces the same draw, so the observed counts,
   estimates, Wilson intervals and z values appear here as numbers rather than
   as a pass verdict. A passing Rust assertion emits nothing, and adding a
   `println!` to the harness would move `harness_source_sha` and stale the
   pinned receipts (DEC-01, DEC-02); linking it instead exercises the same
   compiled sampler and kernels while changing no pinned binary.

The anchor's three parameters are literals inside the harness test rather than
exported constants, so `anchor-report` transcribes them. This script parses them
out of the test source and checks the transcription, which is what keeps the two
from drifting apart.

Usage:
    python3 order3-anchor-check.py > order3-anchor-2026-08-08.txt
"""

from __future__ import annotations

import hashlib
import itertools
import pathlib
import platform
import re
import subprocess
import sys
from fractions import Fraction

QS = (3, 5, 7)
HARNESS = pathlib.Path(__file__).resolve().parents[2] / "research" / "permanent-sampling-feas"
TEST = "equivalence::tests::sampled_zero_fraction_recovers_the_exact_value_at_order_3"


def permanent_3x3(m: tuple[int, ...], q: int) -> int:
    """The six-term permanent of a 3x3 matrix, independent of any kernel."""
    a, b, c, d, e, f, g, h, i = m
    return (a * e * i + a * f * h + b * d * i + b * f * g + c * d * h + c * e * g) % q


def exact_zero_fraction(q: int) -> tuple[int, int, Fraction]:
    total = q**9
    zeros = sum(
        1 for m in itertools.product(range(q), repeat=9) if permanent_3x3(m, q) == 0
    )
    return zeros, total, Fraction(zeros, total)


def read_test_parameters() -> dict[str, str]:
    """Read the anchor's constants out of the harness source, so the receipt
    cannot drift from the test it describes."""
    src = (HARNESS / "src" / "equivalence.rs").read_text()
    body = src[src.index("fn sampled_zero_fraction_recovers_the_exact_value_at_order_3") :]
    body = body[: body.index("\n    }")]
    grab = lambda pat, default: (re.search(pat, body).group(1) if re.search(pat, body) else default)
    return {
        "draws_per_field": grab(r"let draws = ([0-9_]+)usize", "?").replace("_", ""),
        "sampler_seed_root": grab(r"MatrixSampler::new\((0x[0-9A-Fa-f_]+)", "?"),
        "sampler_stream": grab(r"MatrixSampler::new\([^,]+,\s*q,\s*3,\s*([0-9_]+)\)", "?").replace("_", ""),
        "threshold_sigma": grab(r"z\.abs\(\) < ([0-9.]+)", "?"),
        "rng": "ChaCha20 via the harness MatrixSampler (rand_chacha)",
    }


def main() -> int:
    script = pathlib.Path(__file__).resolve()
    digest = hashlib.sha256(script.read_bytes()).hexdigest()
    params = read_test_parameters()

    print("# Order-3 sampling anchor for the b488f02c feasibility study.")
    print("# Checks that the campaign's sampler and kernels together recover a")
    print("# zero fraction whose exact value is known by enumeration.")
    print(f"# script: {script.name}")
    print(f"# script_sha256: {digest}")
    print(f"# invocation: python3 {script.name}")
    print(f"# python: {platform.python_version()} ({platform.python_implementation()})")
    print(f"# platform: {platform.platform()}")
    print("#")
    print("# Anchor parameters, read from the harness source at run time:")
    for k, v in params.items():
        print(f"#   {k}: {v}")
    print("#")
    print("# The exact values below are enumerated by THIS script in Python, so the")
    print("# ground-truth side of the comparison does not depend on the code under")
    print("# test. The sampled side is produced by the harness's own test.")
    print()

    print("## Exact order-3 zero fractions, by enumeration of all q^9 matrices")
    print()
    print("q  matrices   zeros      exact fraction     decimal")
    for q in QS:
        zeros, total, frac = exact_zero_fraction(q)
        print(f"{q}  {total:<9d}  {zeros:<9d}  {str(frac):<17s}  {float(frac):.8f}")
    print()
    z3, t3, f3 = exact_zero_fraction(3)
    ok_scheinerman = z3 == 8163
    print(f"scheinerman2024_table3_z3_expected: 8163")
    print(f"scheinerman2024_table3_z3_observed: {z3}")
    print(f"scheinerman2024_table3_agrees: {'yes' if ok_scheinerman else 'NO'}")
    print()

    print("## Observed draw, via the anchor-report crate")
    print()
    ar = pathlib.Path(__file__).resolve().parent / "anchor-report"
    build = subprocess.run(
        ["cargo", "build", "--release", "--quiet"], cwd=ar, capture_output=True, text=True
    )
    if build.returncode != 0:
        print("anchor_report_build: FAILED")
        print(build.stderr.strip()[:2000])
        return 1
    exe = ar / "target" / "release" / "anchor-report"
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True).stdout.strip()
    cargo = subprocess.run(["cargo", "--version"], capture_output=True, text=True).stdout.strip()
    git = lambda *a: subprocess.run(["git", *a], cwd=ar, capture_output=True, text=True).stdout.strip()
    head = git("rev-parse", "HEAD")
    crate_dirty = git("status", "--porcelain", "--", ".")
    crate_commit = git("log", "-1", "--format=%H", "--", ".")
    cpu = "unknown"
    try:
        for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass

    print("# Source identity: the crate is committed and unmodified, so the sha")
    print("# below names exactly the code that was built. Binary identity: the")
    print("# executable is hashed after the build, so the run is attributable")
    print("# without trusting the source path.")
    print(f"crate: {ar.name} (standalone, outside the root workspace)")
    print(f"invocation: cargo run --manifest-path {ar}/Cargo.toml --release")
    print(f"repo_head_at_run: {head}")
    print(f"crate_last_commit: {crate_commit}")
    print(f"crate_dir_dirty: {'yes' if crate_dirty else 'no'}")
    print(f"anchor_report_sha256: {hashlib.sha256(exe.read_bytes()).hexdigest()}")
    print(f"rustc: {rustc}")
    print(f"cargo: {cargo}")
    print(f"cpu: {cpu}")
    run = subprocess.run([str(exe)], capture_output=True, text=True)
    print()
    print(run.stdout.rstrip())
    observed_ok = run.returncode == 0
    print()
    # The transcription check: the crate's parameters must match the test source.
    used = re.search(r"anchor_parameters_used: seed_root=0x([0-9A-Fa-f]+) stream=(\d+) draws_per_field=(\d+)", run.stdout)
    match = (
        used is not None
        and used.group(1).lower() == params["sampler_seed_root"].replace("0x", "").replace("_", "").lower()
        and used.group(2) == params["sampler_stream"]
        and used.group(3) == params["draws_per_field"]
    )
    print(f"parameters_match_harness_test: {'yes' if match else 'NO'}")
    print()

    print("## Harness anchor test")
    print()
    cmd = [
        "cargo", "test", "--release", "--features", "hip",
        "--", "--exact", "--nocapture", TEST,
    ]
    print(f"command: {' '.join(cmd)}")
    print(f"cwd: {HARNESS}")
    proc = subprocess.run(cmd, cwd=HARNESS, capture_output=True, text=True)
    passed = proc.returncode == 0 and "1 passed" in proc.stdout
    for line in proc.stdout.splitlines():
        if "test result:" in line and "filtered out" in line and "1 passed" in line:
            print(f"result_line: {line.strip()}")
    print(f"exit_code: {proc.returncode}")
    print(f"anchor_passes: {'yes' if passed else 'NO'}")
    print()
    print("# The assertion above prints nothing on success; the observed numbers")
    print("# come from the anchor-report section, which reproduces the same draw")
    print("# against the same library code.")

    return 0 if (ok_scheinerman and passed and observed_ok and match) else 1


if __name__ == "__main__":
    sys.exit(main())
