#!/usr/bin/env python3
"""Writes the source-evidence ledger of jit:54e70918.

Each claim below names a commit, a path and one whole line of that file there.
The script reads the file at the commit and writes `source-evidence.json`
beside itself with the line number of the first line equal to the text, the
number of lines holding it and the digest of the file, in the row shape the
shared ledger verifier checks. A claim whose line is absent fails the script.

Usage: make-source-evidence.py
"""

import hashlib
import json
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ISSUE = "54e70918"
# The tree before this issue's test changes, and the tree holding them.
BEFORE = "d424d91f1df3632a351511f567c9dcd17159b271"
AFTER = "45527d869770744f18e4d3fa24bf60744b6e79cd"

CLAIMS = [
    (
        "demo-returns-without-a-backend",
        BEFORE,
        "crates/gf2-core/tests/simd_equiv_demo.rs",
        "    if gf2_core::kernels::simd::maybe_simd().is_none() {",
        "Each occurrence opens a block that returns from its test before any "
        "comparison, so the test passes without a SIMD backend.",
    ),
    (
        "hoist-returns-without-a-backend",
        BEFORE,
        "crates/gf2-core/tests/simd_equiv_dispatch_hoist.rs",
        "    if gf2_core::kernels::simd::maybe_simd().is_none() {",
        "Each occurrence opens a block that returns from its test before any "
        "comparison, although the resolver under test answers without a backend.",
    ),
    (
        "backend-is-detected-once",
        AFTER,
        "crates/gf2-core/src/kernels/simd/mod.rs",
        "pub static SIMD_BACKEND: LazyLock<Option<SimdBackend>> = "
        "LazyLock::new(SimdBackend::detect);",
        "`maybe_simd` reads a process-wide value initialized from detection; the "
        "module publishes no setter or override.",
    ),
    (
        "detection-reads-the-processor",
        AFTER,
        "crates/gf2-kernels-simd/src/x86/mod.rs",
        '    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) && '
        'is_x86_feature_detected!("avx2") {',
        "The logical bundle depends on the target and the processor feature only, "
        "so a host with AVX2 cannot be made to report no bundle.",
    ),
]


def main():
    rows = []
    for claim, commit, path, verbatim, why in CLAIMS:
        full = subprocess.run(
            ["git", "-C", str(HERE), "rev-parse", f"{commit}^{{commit}}"],
            capture_output=True, check=True, text=True,
        ).stdout.strip()
        content = subprocess.run(
            ["git", "-C", str(HERE), "show", f"{full}:{path}"],
            capture_output=True, check=True,
        ).stdout
        lines = content.decode().splitlines()
        rows.append({
            "claim": claim,
            "project": "gf2",
            "commit": full,
            "path": path,
            "line": lines.index(verbatim) + 1,
            "verbatim": verbatim,
            "occurrences": sum(verbatim in line for line in lines),
            "sha256": hashlib.sha256(content).hexdigest(),
            "why": why,
        })
    ledger = {"schema": "source-evidence-v1", "issue": ISSUE, "claims": rows}
    (HERE / "source-evidence.json").write_text(json.dumps(ledger, indent=1) + "\n")
    print(f"{len(rows)} claims")


if __name__ == "__main__":
    main()
