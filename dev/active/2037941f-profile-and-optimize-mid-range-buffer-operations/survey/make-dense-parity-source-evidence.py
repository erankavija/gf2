#!/usr/bin/env python3
"""Reproduce the source claims the dense-parity addendum makes (jit:96c94b81).

Every claim names a project, a repository-relative path, the line, the verbatim
line at that position and why the addendum relies on it. Each claim also states
how many times its fragment occurs in that file, so a fragment that gains or
loses an occurrence fails this script.

The committed ledger is a record pinned to the commits its rows name, and
receipts pin its digest. Each claim is resolved in the file as it is at the
commit its ledger row records (`git show`; an unavailable object is an error),
never in the working tree, so the output is the committed ledger byte for byte
on any tree. The script refuses to write a ledger that differs from the
committed one; `--check` reports the difference and writes nothing.
"""

import argparse
import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(
    subprocess.run(
        ["git", "-C", str(pathlib.Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"],
        capture_output=True, check=True, text=True,
    ).stdout.strip()
)
STORY = pathlib.Path("dev/active/2037941f-profile-and-optimize-mid-range-buffer-operations")
OUTPUT = STORY / "survey" / "dense-parity-source-evidence.json"

# (path, fragment, expected occurrences, why)
CLAIMS = [
    (
        "crates/gf2-core/src/matrix.rs",
        "    pub fn matvec(&self, x: &crate::BitVec) -> crate::BitVec {",
        1,
        "The allocated whole-consumer cell times exactly this public method, "
        "whose return value is the freshly allocated output.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "        match matvec_route(self.stride_words) {",
        1,
        "Route selection happens inside the public call, so it is inside the "
        "allocated cost boundary.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "pub fn matvec_route(stride_words: usize) -> MatvecRoute {",
        1,
        "The selector is public, so an arm observes the route each cell takes "
        "without forcing it through a private hook.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "        if stride_words >= MATVEC_SIMD_MIN_WORDS_SELECTED {",
        1,
        "The threshold comparison puts every anchor stride from eight words "
        "upward on the SIMD route.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;",
        1,
        "Eight words is the conservative default definition of the selector "
        "threshold, which fixes the lowest anchor stride.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "        for row in self.data.chunks_exact(self.stride_words).take(self.rows) {",
        1,
        "The SIMD lane walks whole rows of the declared stride, so one cell's "
        "call count is its row count.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "            y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);",
        1,
        "The production route reaches the fused kernel once per row through "
        "the bundle function pointer and folds parity from the low bit.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "    fn row_dot_parity_scalar(row: &[u64], x_words: &[u64]) -> bool {",
        1,
        "The four-accumulator scalar row parity is private, so the reference "
        "arm reaches it only through a build without the simd feature.",
    ),
    (
        "crates/gf2-core/src/matrix.rs",
        "        let stride_words = if cols == 0 { 0 } else { cols.div_ceil(64) };",
        2,
        "Stride is the ceiling of columns over 64 at both construction sites, "
        "which fixes each anchor cell's column count.",
    ),
    (
        "crates/gf2-core/src/bitvec.rs",
        "    pub fn with_capacity(bits: usize) -> Self {",
        1,
        "The output allocation is one reservation of the ceiling of rows over "
        "64 words, made inside the timed public call.",
    ),
    (
        "crates/gf2-core/src/bitvec.rs",
        "    pub fn push_bit(&mut self, bit: bool) {",
        1,
        "Each output bit is appended through a branch on the parity value, so "
        "output density is part of the allocated cost.",
    ),
    (
        "crates/gf2-kernels-simd/src/x86/avx2.rs",
        "unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {",
        1,
        "This is the kernel identity the bundle field reaches on an AVX2 "
        "host, and the body the complexity budget is measured against.",
    ),
    (
        "crates/gf2-kernels-simd/src/x86/avx2.rs",
        "    fn and_popcnt_fn(lhs: &[u64], rhs: &[u64]) -> u64 {",
        1,
        "The bundle field is this wrapper, so an arm that devirtualises it "
        "measures a different route than a consumer reaches.",
    ),
    (
        "crates/gf2-kernels-simd/src/lib.rs",
        "    pub and_popcnt_csa_fn: fn(&[u64], &[u64]) -> u64,",
        1,
        "The carry-save fused comparator stays a direct comparator field that "
        "no automatic resolver selects, so it is not a candidate here.",
    ),
    (
        "crates/gf2-core/Cargo.toml",
        "simd = []",
        1,
        "The simd feature is opt-in, so the scalar reference arm is a "
        "separately built executable rather than a runtime toggle.",
    ),
    (
        "dev/tools/tuning-campaign-support/src/trial_ledger.rs",
        "        .filter(|c| c.role != CellRole::Exploratory)",
        2,
        "Reservation and verification both count non-exploratory cells only, "
        "so a retained exploratory row spends no comparison.",
    ),
    (
        "dev/tools/tuning-campaign-support/src/receipt.rs",
        "                || f64::from(settings.bootstrap_resamples) * corrected_alpha / 2.0 < 20.0;",
        1,
        "P-20 requires twenty expected draws per bootstrap tail, which bounds "
        "each family's confirmatory cell count.",
    ),
]


def recorded_commits():
    """The commit each claim's ledger row records, in claim order."""
    try:
        rows = json.loads((ROOT / OUTPUT).read_text())["claims"]
    except (OSError, ValueError, KeyError) as error:
        raise SystemExit(f"{OUTPUT}: no ledger records the claims' commits: {error}") from error
    if [row["path"] for row in rows] != [claim[0] for claim in CLAIMS]:
        raise SystemExit(f"{OUTPUT}: the ledger rows are not the claims' paths in order")
    return [row["commit"] for row in rows]


def file_at(commit, path):
    done = subprocess.run(
        ["git", "-C", str(ROOT), "show", f"{commit}:{path}"], capture_output=True
    )
    if done.returncode != 0:
        raise SystemExit(f"{path} is unavailable at {commit}: {done.stderr.decode().strip()}")
    return done.stdout


def reproduce():
    """The ledger text the recorded commits yield."""
    records = []
    for (path, fragment, occurrences, why), commit in zip(CLAIMS, recorded_commits()):
        content = file_at(commit, path)
        lines = content.decode().splitlines()
        positions = [index + 1 for index, line in enumerate(lines) if fragment in line]
        if not positions:
            raise SystemExit(f"{path}: the claimed line is absent at {commit}: {fragment}")
        if len(positions) != occurrences:
            raise SystemExit(
                f"{path}: expected {occurrences} occurrences of {fragment!r} at {commit}, "
                f"found {len(positions)}"
            )
        records.append(
            {
                "project": "gf2",
                "commit": commit,
                "path": path,
                "line": positions[0],
                "occurrences": len(positions),
                "verbatim": lines[positions[0] - 1],
                "sha256": hashlib.sha256(content).hexdigest(),
                "why": why,
            }
        )
    document = {
        "schema": "dense-parity-source-evidence-v1",
        "issue": "96c94b81",
        "addendum_identity": "2037941f-dense-parity-v2",
        "claims": records,
    }
    return json.dumps(document, indent=2) + "\n"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare the reproduction with the committed ledger and write nothing",
    )
    arguments = parser.parse_args(argv)
    text = reproduce()
    if (ROOT / OUTPUT).read_text() != text:
        raise SystemExit(f"{OUTPUT} differs from its reproduction at the recorded commits; not written")
    if not arguments.check:
        (ROOT / OUTPUT).write_text(text)
    print(f"{OUTPUT}: {len(CLAIMS)} source claims reproduce the committed ledger")


if __name__ == "__main__":
    main()
