#!/usr/bin/env python3
"""Freeze the source claims the dense-parity no-change outcome makes (jit:6e87c436).

Every claim names a file by package and package-relative path, a fragment of
one line, how often the fragment occurs in the file and why the outcome relies
on it. The script writes `source-evidence.json` beside itself with the line the
fragment occupies, the verbatim line, the file digest and the commit that last
changed the file. A fragment that is absent or changes its occurrence count
fails the script.
"""

import hashlib
import json
import subprocess

from locate import HERE, ROOT, repository_files

# (package, package-relative path, fragment, expected occurrences, why)
CLAIMS = [
    (
        "gf2-core", "src/matrix.rs",
        "    pub fn matvec(&self, x: &crate::BitVec) -> crate::BitVec {", 1,
        "The public dense product whose behaviour and selection this outcome leaves unchanged.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "        match matvec_route(self.stride_words) {", 1,
        "The public call selects its lane from the row stride alone.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "        if stride_words >= MATVEC_SIMD_MIN_WORDS_SELECTED {", 1,
        "One threshold comparison separates the scalar lane from the SIMD lane.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;", 1,
        "The ordinary build compares the stride with this conservative constant.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "const MATVEC_SIMD_MIN_WORDS_SELECTED: usize = crate::tuning::baked::MATVEC_SIMD_MIN_WORDS;",
        1,
        "A `gf2_tuning_baked` build substitutes the baked matvec threshold, not the "
        "`bit_backend` threshold.",
    ),
    (
        "gf2-core", "src/tuning/baked.rs",
        "pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;", 1,
        "The baked matvec threshold equals the conservative constant, so a baked build "
        "selects the same lane for every stride.",
    ),
    (
        "gf2-core", "src/tuning/baked.rs",
        "pub(crate) const SIMD_MIN_WORDS: usize = 8;", 1,
        "The baked `bit_backend.simd_min_words` value is a separate constant that the "
        "matvec selector does not read.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "                if let Some(fns) = crate::simd::maybe_simd() {", 1,
        "The SIMD lane runs only when the host kernel bundle is detected.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "self.matvec_scalar(x)", 2,
        "The scalar implementation serves the scalar lane and the undetected-bundle fallback.",
    ),
    (
        "gf2-core", "src/matrix.rs",
        "            y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);", 1,
        "The SIMD lane folds each row's parity from the fused AND-popcount bundle entry.",
    ),
    (
        "gf2-core", "src/lib.rs",
        "#![deny(unsafe_code)]", 1,
        "The crate holding `BitMatrix::matvec` admits no unsafe code.",
    ),
    (
        "gf2-core", "Cargo.toml",
        "simd = []", 1,
        "SIMD dispatch is an opt-in feature, so the scalar build is a separate test target.",
    ),
    (
        "gf2-core", "src/bitvec.rs",
        "        bv.mask_tail();", 3,
        "`BitVec::from_words` masks the tail after dropping surplus words, so a matvec "
        "operand carries zero padding.",
    ),
    (
        "gf2-kernels-simd", "src/x86/avx2.rs",
        "unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {", 1,
        "The fused AVX2 kernel body lives in the isolated kernel crate.",
    ),
    (
        "gf2-kernels-simd", "src/x86/avx2.rs",
        "/// The host supports AVX2; `super::detect_x86` establishes that before", 1,
        "The fused kernel's `# Safety` section opens with the required target feature "
        "and the runtime gate that guards the call.",
    ),
    (
        "gf2-kernels-simd", "src/x86/avx2.rs",
        "/// unaligned 32-byte load and every tail byte lies inside the common prefix.", 1,
        "The same section bounds every read by the shorter slice and admits aliasing "
        "of the two read-only operands.",
    ),
    (
        "gf2-kernels-simd", "src/x86/avx2.rs",
        "        unsafe { avx2_and_popcnt(lhs, rhs) }", 1,
        "The safe bundle entry is the unsafe boundary of the fused kernel.",
    ),
    (
        "gf2-kernels-simd", "src/x86/avx2.rs",
        "        // SAFETY: `detect_x86` returns this bundle only after detecting AVX2.", 3,
        "The fused entry and both carry-save entries state the detection contract at "
        "their unsafe calls.",
    ),
    (
        "gf2-kernels-simd", "src/x86/mod.rs",
        '    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) && is_x86_feature_detected!("avx2") {',
        1,
        "The bundle exists only after runtime AVX2 detection.",
    ),
    (
        "gf2-kernels-simd", "src/lib.rs",
        "    pub and_popcnt_csa_fn: fn(&[u64], &[u64]) -> u64,", 1,
        "The carry-save fused comparator stays a direct bundle field outside the matvec route.",
    ),
    (
        "gf2-core", "tests/simd_equiv/mod.rs",
        "pub const WORD_BOUNDARY_LENGTHS: &[usize] = &[0, 1, 63, 64, 65, 127, 128, 129, 255, 256, 257];",
        1,
        "The shared SIMD-equivalence helper owns the word-boundary length list.",
    ),
    (
        "gf2-core", "tests/simd_equiv_matvec.rs",
        "fn matvec_word_boundary_lengths_match_scalar_reference() {", 1,
        "Square boundary shapes compare the public product with a bit-level reference.",
    ),
    (
        "gf2-core", "tests/simd_equiv_matvec.rs",
        "const BOUNDARY_COLS: [usize; 12] = [0, 1, 63, 64, 65, 449, 511, 512, 513, 575, 576, 577];",
        1,
        "The deterministic boundary test spans empty, scalar-lane and eight- and "
        "nine-word SIMD-lane column counts.",
    ),
    (
        "gf2-core", "tests/simd_equiv_matvec.rs",
        "    for rows in [0, 1, 63, 64, 65] {", 1,
        "The same test spans the output word boundaries.",
    ),
    (
        "gf2-core", "tests/simd_equiv_matvec.rs",
        "                assert_zero_tail(&product, &context);", 1,
        "Each product's padding beyond its length is asserted zero.",
    ),
    (
        "gf2-core", "tests/simd_equiv_matvec.rs",
        "            let all_set = BitVec::from_words(vec![u64::MAX; cols.div_ceil(64) + 1], cols);",
        1,
        "A dirty-tailed, over-long word operand exercises `from_words` masking on the "
        "matvec path.",
    ),
    (
        "gf2-core", "tests/simd_equiv_matvec.rs",
        "fn matvec_matches_scalar_reference_proptest_sizes_0_to_1024() {", 1,
        "Random shapes on both lanes compare with the bit-level reference.",
    ),
    (
        "gf2-core", "tests/popcount_routes.rs",
        "fn every_fused_route_counts_slices_at_each_word_offset() {", 1,
        "Every fused route, the bundle entry included, runs on slices at each word offset.",
    ),
    (
        "gf2-core", "tests/popcount_routes.rs",
        "fn every_fused_route_counts_the_intersection() {", 1,
        "Every fused route runs on aligned buffers of the empty, boundary and block lengths.",
    ),
    (
        "gf2-core", "tests/popcount_routes.rs",
        "fn matrix_vector_parity_agrees_across_strides_that_change_the_route() {", 1,
        "The public product is checked on strides of both lanes.",
    ),
    (
        "gf2-core", "tests/matrix_selection_baked.rs",
        "        matvec_route(conservative),", 1,
        "The baked-build witness observes the SIMD lane at the conservative threshold.",
    ),
    (
        "dense-parity-harness", "src/routes.rs",
        "        route: matvec_route(stride_words),", 1,
        "An arm's reported `selected_path` lane is the public selector's answer for the "
        "cell's stride.",
    ),
]


def last_change(path):
    """The commit that last changed `path`; content digests decide validity."""
    return subprocess.run(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%H", "--", path],
        capture_output=True, check=True, text=True,
    ).stdout.strip()


def main():
    records = []
    for package, relative, fragment, occurrences, why in CLAIMS:
        path = f"{repository_files.package_directory(ROOT, package)}/{relative}"
        text = (ROOT / path).read_text()
        lines = text.splitlines()
        positions = [index + 1 for index, line in enumerate(lines) if fragment in line]
        if len(positions) != occurrences:
            raise SystemExit(
                f"{path}: expected {occurrences} occurrences of {fragment!r}, found {len(positions)}"
            )
        records.append(
            {
                "project": "gf2",
                "commit": last_change(path),
                "path": path,
                "line": positions[0],
                "occurrences": occurrences,
                "verbatim": lines[positions[0] - 1],
                "why": why,
                "sha256": hashlib.sha256(text.encode()).hexdigest(),
            }
        )
    output = HERE / "source-evidence.json"
    output.write_text(
        json.dumps({"schema": "source-evidence-v1", "issue": "6e87c436", "claims": records}, indent=2)
        + "\n"
    )
    print(f"{output.relative_to(ROOT)}: {len(records)} source claims")


if __name__ == "__main__":
    main()
