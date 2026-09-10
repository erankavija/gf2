#!/usr/bin/env python3
"""Emit the source evidence behind the c7113c5a survey's code claims.

Usage:
  inspect-sources.py --gf2-commit <sha> --gf2x-mirror <gf2x clone> --output <json>

Every claim names a project, a pinned commit, a path and a needle. The script
reads the file at that commit with `git show`, requires the needle to occur on
exactly the stated occurrence, and records the line number and the verbatim
line, so no line number or quotation is transcribed by hand. The search entries
record every non-definition use of the public long-product entry points in the
gf2 crates at the pinned commit, classified by where the match sits.
"""

import argparse
import json
import re
import subprocess
import sys

GF2X_COMMIT = "27ba588f03bf6e1e74763903bab25e6e8bb6d0f0"

WIDE = "crates/gf2-core/src/gf2m/wide.rs"
BARRETT = "crates/gf2-core/src/gf2m/barrett.rs"

# claim, project, path, needle, occurrence (1-based), why
CLAIMS = [
    ("public-clmul-wide", "gf2", WIDE,
     "pub fn clmul_wide<const N: usize, const M: usize>", 1,
     "clmul_wide is the documented public long-product entry point."),
    ("public-clmul-wide-delegates", "gf2", WIDE,
     "clmul_wide_slice::<N>(a, b, &mut out);", 1,
     "clmul_wide's body delegates to clmul_wide_slice; it performs no capability dispatch."),
    ("public-clmul-wide-slice", "gf2", WIDE,
     "pub fn clmul_wide_slice<const N: usize>", 1,
     "clmul_wide_slice is public; the survey's public-API cell calls it."),
    ("clmul-wide-slice-word-product", "gf2", WIDE,
     "let product: u128 = super::barrett::clmul(a[i], b[j]);", 1,
     "Each of the N^2 word products of the public path calls barrett::clmul."),
    ("barrett-clmul-is-scalar", "gf2", BARRETT,
     "gf2_kernels_simd::clmul_u64_scalar(a, b)", 1,
     "barrett::clmul delegates to the scalar carry-less multiply."),
    ("scalar-clmul-bit-loop", "gf2", "crates/gf2-kernels-simd/src/clmul_scalar.rs",
     "while b_remaining != 0 {", 1,
     "The scalar product loops once per set bit of one operand; it issues no PCLMULQDQ."),
    ("dispatch-helper-crate-private", "gf2", WIDE,
     "pub(crate) fn clmul_wide_slice_product<const N: usize>(", 1,
     "The dispatching long-product helper is crate-private, so external callers cannot reach it by name."),
    ("dispatch-helper-n4", "gf2", WIDE,
     "if let Some(fns) = crate::simd::maybe_gf2m_wide256() {", 1,
     "The helper dispatches N = 4 to the wide256 kernel."),
    ("dispatch-helper-n9", "gf2", WIDE,
     "if let Some(fns) = crate::simd::maybe_gf2m_wide571() {", 1,
     "The helper dispatches N = 9 to the wide571 kernel."),
    ("dispatch-helper-fallback", "gf2", WIDE,
     "clmul_wide_slice::<N>(a, b, out);", 1,
     "Every other N, and N = 4 or 9 without a kernel, falls back to the scalar schoolbook."),
    ("mul-ref-uses-helper", "gf2", WIDE,
     "clmul_wide_slice_product::<N>(&self.words, &rhs.words, &mut product);", 1,
     "Gf2mWide::mul_ref computes its unreduced product through the dispatching helper."),
    ("mul-ref-reduces", "gf2", WIDE,
     "let reduced = reducer.reduce_slice(&product);", 1,
     "Gf2mWide::mul_ref then applies BarrettReducerWide::reduce_slice; the survey times the product and reports this reduction separately."),
    ("mul-ref-rustdoc-names-public-path", "gf2", WIDE,
     "carry-less multiplications (via `clmul_wide::<N, {2*N}>`) plus", 1,
     "mul_ref's Complexity rustdoc names clmul_wide as its mechanism while its body uses the dispatching helper."),
    ("barrett-wide-uses-helper", "gf2", BARRETT,
     "super::wide::clmul_wide_slice_product::<N>(a, b_stored, out);", 1,
     "The wide Barrett reducer's own products also use the dispatching helper."),
    ("cached-wide-detection", "gf2", "crates/gf2-core/src/lib.rs",
     ".get_or_init(gf2_kernels_simd::gf2m_wide::detect_wide)", 1,
     "gf2-core caches gf2_kernels_simd::gf2m_wide::detect_wide, the detection the survey's gf2 arm calls, so both reach the same kernel."),
    ("wide256-ymm-kernel", "gf2", "crates/gf2-kernels-simd/src/gf2m_wide.rs",
     "clmul: clmul_wide4_ymm_safe,", 1,
     "On an AVX2+VPCLMULQDQ host the detection publishes the YMM 4-limb kernel."),
    ("wide571-ymm-kernel", "gf2", "crates/gf2-kernels-simd/src/gf2m_wide.rs",
     "clmul: clmul_wide9_ymm_safe,", 1,
     "On an AVX2+VPCLMULQDQ host the detection publishes the YMM 9-limb kernel."),
    ("raw-batch-default-sequential", "gf2", "crates/gf2-kernels-simd/src/gf2m.rs",
     "detect_with_clmul_batch_preference(ClmulBatchLane::Sequential)", 1,
     "The default detection selects the sequential PCLMULQDQ raw-batch lane."),
    ("raw-batch-lane-tag", "gf2", "crates/gf2-kernels-simd/src/x86/clmul.rs",
     "pub(crate) const CLMUL_BATCH_PATH_XMM: &str = \"pclmulqdq-scalar-xmm\";", 1,
     "The lane tag the survey's gf2 arm reports for the sequential lane."),
    ("dot-product-uses-raw-batch", "gf2", "crates/gf2-core/src/field/vec.rs",
     "let batch_fn = sample.clmul_batch_fn()?;", 1,
     "FieldVec::simd_dot_product consumes the raw carry-less batch."),
    ("dot-product-reduces-once", "gf2", "crates/gf2-core/src/field/vec.rs",
     "let reducer = sample.barrett_reducer()?;", 1,
     "The dot product reduces its XOR-accumulated products with a Barrett reducer."),
    ("wide-bench-modulus-256", "gf2", "crates/gf2-core/benches/gf2m_wide_mul.rs",
     "const MODULUS: [u64; 4] = [0x425, 0, 0, 0];", 1,
     "gf2's own wide-multiplication benchmark uses x^256 + x^10 + x^5 + x^2 + 1; the reduction probe uses the same modulus."),
    ("wide-bench-modulus-571", "gf2", "crates/gf2-core/benches/gf2m_wide_mul.rs",
     "const MODULUS: [u64; 9] = [0x425, 0, 0, 0, 0, 0, 0, 0, 0];", 1,
     "gf2's own benchmark uses x^571 + x^10 + x^5 + x^2 + 1 for GF(2^571); the reduction probe uses the same modulus."),
    ("gf2x-public-entry", "gf2x", "gf2x.h.in.in",
     "extern int GF2X_EXPORTED gf2x_mul_r(unsigned long *c,", 1,
     "gf2x_mul_r is gf2x's exported reentrant multiplication; the survey measures it."),
    ("gf2x-basecase-branch", "gf2x", "gf2x.c",
     "if (sa < GF2X_MUL_KARA_THRESHOLD) {", 1,
     "Operands below the Karatsuba threshold take the hand-written basecase."),
    ("gf2x-basecase-4", "gf2x", "gf2x.c",
     "case 4: gf2x_mul4(c, a, b); return;", 1,
     "Four-word balanced operands run gf2x_mul4."),
    ("gf2x-basecase-9", "gf2x", "gf2x.c",
     "case 9: gf2x_mul9(c, a, b); return;", 1,
     "Nine-word balanced operands run gf2x_mul9."),
    ("gf2x-fft-gate", "gf2x", "gf2x.c",
     "if (sc >= GF2X_TERNARY_FFT_MINIMUM_SIZE && K && K != 1) {", 1,
     "The FFT runs only when the tuned table selects K other than 0 or 1."),
    ("gf2x-balanced-toom", "gf2x", "gf2x.c",
     "gf2x_mul_toom(dst, a, b, sa, xpool->stk);", 1,
     "Balanced operands above the basecase go to gf2x_mul_toom."),
    ("gf2x-toom-selector", "gf2x", "toom.c",
     "switch (gf2x_best_toom(n)) {", 1,
     "gf2x_mul_toom switches on gf2x_best_toom."),
    ("gf2x-best-toom-table", "gf2x", "toom.c",
     "return best_tab[n-1];", 1,
     "Within the tuning limit the choice is the tuned GF2X_BEST_TOOM_TABLE entry, not a threshold comparison."),
    ("gf2x-gpl-condition", "gf2x", "configure.ac",
     "AM_CONDITIONAL([GPL_CODE_PRESENT],[grep -q \"released under the GPL\" \"$srcdir/toom-gpl.c\"])", 1,
     "GPL_CODE_PRESENT, and with it the GPL-3.0-or-later license and the Toom-Cook routines, follows from toom-gpl.c's contents."),
    ("gf2x-kara-only-without-gpl", "gf2x", "toom.c",
     "return GF2X_SELECT_KARA;", 2,
     "Without GPL code gf2x_best_toom always returns Karatsuba."),
    ("gf2x-appends-mpclmul", "gf2x", "config/acinclude.m4",
     "CFLAGS=\"$CFLAGS -mpclmul\"", 1,
     "configure appends -mpclmul to CFLAGS whenever the compiler accepts it, independent of the requested optimisation flags."),
    ("gf2x-mul1-header-target", "gf2x", "already_tuned/x86_64_pclmul/gf2x_mul1.h",
     "../../lowlevel/mul1cl.c", 1,
     "The x86_64_pclmul gf2x_mul1.h configure links is a symlink to lowlevel/mul1cl.c."),
    ("gf2x-mul1-pclmul", "gf2x", "lowlevel/mul1cl.c",
     "_mm_storeu_si128((__m128i*)c, _mm_clmulepi64_si128(aa, bb, 0));", 1,
     "The selected word basecase multiplies with the 128-bit PCLMULQDQ intrinsic."),
    ("gf2x-mul4-header-target", "gf2x", "already_tuned/x86_64_pclmul/gf2x_mul4.h",
     "../../lowlevel/mul4clk.c", 1,
     "The selected gf2x_mul4 is lowlevel/mul4clk.c."),
    ("gf2x-mul4-karatsuba", "gf2x", "lowlevel/mul4clk.c",
     "/* specialized Karatsuba with 3 calls to mul2, i.e., 9 multiplications */", 1,
     "gf2x's four-word basecase uses 9 word multiplications against the 16 of gf2's schoolbook kernel."),
    ("gf2x-mul9-header-target", "gf2x", "already_tuned/x86_64_pclmul/gf2x_mul9.h",
     "../../lowlevel/mul9cl.c", 1,
     "The selected gf2x_mul9 is lowlevel/mul9cl.c."),
    ("gf2x-mul9-thirty", "gf2x", "lowlevel/mul9cl.c",
     "/* variant with 30 multiplications */", 1,
     "gf2x's nine-word basecase uses 30 word multiplications against the 81 of gf2's schoolbook kernel."),
    ("wide256-kernel-pairs-16-products", "gf2", "crates/gf2-kernels-simd/src/x86/gf2m_wide.rs",
     "/// two 64×64 carry-less products per instruction. We pair the 16 scalar", 1,
     "gf2's 4-limb YMM kernel performs all 16 schoolbook word products, two per 256-bit VPCLMULQDQ."),
    ("wide571-kernel-81-products", "gf2", "crates/gf2-kernels-simd/src/x86/gf2m_wide.rs",
     "/// The 81 scalar word-pair products are scheduled row-major two at a time.", 1,
     "gf2's 9-limb YMM kernel performs all 81 schoolbook word products, two per 256-bit VPCLMULQDQ."),
]

SEARCH_PATTERN = r"clmul_wide(_slice)?(::<|\()"


def show(repo, commit, path):
    return subprocess.run(
        ["git", "-C", repo, "show", f"{commit}:{path}"],
        check=True, capture_output=True, text=True,
    ).stdout.split("\n")


def locate(lines, needle, occurrence):
    hits = [index for index, line in enumerate(lines) if needle in line]
    if len(hits) < occurrence:
        raise SystemExit(f"needle {needle!r} occurs {len(hits)} times, need occurrence {occurrence}")
    return hits[occurrence - 1] + 1


def classify(path, line_number, text, test_start):
    stripped = text.strip()
    if path.startswith("crates/") and "/benches/" in path:
        return "benchmark"
    if stripped.startswith(("///", "//!", "//")):
        return "comment or rustdoc"
    if test_start is not None and line_number > test_start:
        return "test module"
    if re.search(r"\bfn clmul_wide(_slice)?<", stripped):
        return "definition"
    return "code"


def search(repo, commit):
    output = subprocess.run(
        ["git", "-C", repo, "grep", "-n", "-E", SEARCH_PATTERN, commit, "--", "crates/"],
        check=True, capture_output=True, text=True,
    ).stdout
    test_starts = {}
    matches = []
    for raw in output.splitlines():
        _, path, number, text = raw.split(":", 3)
        if path not in test_starts:
            lines = show(repo, commit, path)
            starts = [i + 1 for i, l in enumerate(lines) if l.strip() == "#[cfg(test)]"]
            test_starts[path] = starts[0] if starts else None
        kind = classify(path, int(number), text, test_starts[path])
        matches.append({"path": path, "line": int(number), "text": text, "kind": kind})
    return {
        "command": f"git grep -n -E '{SEARCH_PATTERN}' {commit} -- crates/",
        "matches": matches,
        "code_matches_outside_definitions": [
            m for m in matches if m["kind"] == "code"
        ],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--gf2-commit", required=True)
    parser.add_argument("--gf2x-mirror", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    repo = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], check=True, capture_output=True, text=True,
    ).stdout.strip()
    gf2_commit = subprocess.run(
        ["git", "-C", repo, "rev-parse", args.gf2_commit],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    roots = {"gf2": (repo, gf2_commit), "gf2x": (args.gf2x_mirror, GF2X_COMMIT)}
    claims = []
    for claim, project, path, needle, occurrence, why in CLAIMS:
        root, commit = roots[project]
        lines = show(root, commit, path)
        line = locate(lines, needle, occurrence)
        claims.append({
            "claim": claim,
            "project": project,
            "commit": commit,
            "path": path,
            "line": line,
            "text": lines[line - 1],
            "why": why,
        })
    report = {
        "schema": "c7113c5a-source-evidence-v1",
        "commits": {"gf2": gf2_commit, "gf2x": GF2X_COMMIT},
        "claims": claims,
        "searches": [dict(search(repo, gf2_commit), purpose=(
            "Every use of the public long-product entry points in the gf2 crates; "
            "a production caller would appear as kind 'code'."))],
    }
    with open(args.output, "w") as handle:
        json.dump(report, handle, indent=2)
        handle.write("\n")
    print(f"wrote {len(claims)} claims to {args.output}", file=sys.stderr)


if __name__ == "__main__":
    main()
