#!/usr/bin/env python3
"""Write `source-evidence.json`: every code claim the findings rely on.

Each claim names a project, its pinned revision, a path, a line and the
verbatim text of that line. The script reads the pinned sources, locates the
line by an exact substring and its occurrence index, and refuses to write a
claim whose text has moved, so the committed evidence is regenerated rather
than transcribed.

Usage: dev/active/6fb89a3c/survey/make-source-evidence.py  (from the repo root)
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path(subprocess.check_output(["git", "-C", ROOT, "rev-parse", "--show-toplevel"], text=True).strip())
COMMON_ROOT = pathlib.Path(subprocess.check_output(
    ["git", "-C", ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir"], text=True).strip()).parent
EXT = pathlib.Path(os.environ.get("GF2_SURVEY_EXT", COMMON_ROOT / ".agents/ext/6fb89a3c"))

PROJECTS = {
    "gf2": {"root": REPO, "revision": subprocess.check_output(["git", "-C", REPO, "rev-parse", "HEAD"], text=True).strip(),
            "revision_kind": "git commit (informational navigation metadata; receipts pin these files by content digest)"},
    "m4ri": {"root": EXT, "revision": "release tarball m4ri-20260122.tar.gz sha256 7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404",
             "revision_kind": "release tarball digest; paths are relative to the staging root: m4ri-src/ is the unpacked tarball, prefix/ the configured install"},
    "bitshuffle": {"root": EXT / "bitshuffle", "revision": "52aec3b80d05606c090956aecfe868489d96b95c", "revision_kind": "git commit (tag 0.5.2)"},
    "isa-l": {"root": EXT / "isa-l", "revision": "7c3479e0a9dac17f448603ec1ad64c7c625f530c", "revision_kind": "git commit (tag v2.32.1)"},
}

# (claim id, project, path, exact substring, occurrence index, why it matters)
CLAIMS = [
    # gf2 transpose route
    ("gf2-transpose-detect", "gf2", "crates/gf2-kernels-simd/src/transpose.rs", "pub fn detect() -> Option<TransposeFns> {", 0,
     "The kernel-isolated gf2 arm calls this runtime dispatcher."),
    ("gf2-transpose-avx2-runtime-check", "gf2", "crates/gf2-kernels-simd/src/transpose.rs", 'if is_x86_feature_detected!("avx2") {', 0,
     "The 64x64 kernel is selected at run time by CPU feature detection, so the conservative-portable build still runs the AVX2 lane on this host."),
    ("gf2-transpose-avx2-name", "gf2", "crates/gf2-kernels-simd/src/transpose.rs", 'name: "avx2-bit-twiddle",', 0,
     "The selected path name the receipts record for the fixed 64x64 cells."),
    ("gf2-bitmatrix-transpose", "gf2", "crates/gf2-core/src/matrix.rs", "pub fn transpose(&self) -> Self {", 0,
     "The whole-consumer gf2 arm calls this method."),
    ("gf2-bitmatrix-transpose-fresh-output", "gf2", "crates/gf2-core/src/matrix.rs", "let mut out = Self::zeros(self.cols, self.rows);", 0,
     "Every whole-consumer call allocates a fresh output, which the M4RI and Bitshuffle consumer arms match."),
    ("gf2-bitmatrix-zeros-alloc", "gf2", "crates/gf2-core/src/matrix.rs", "data: vec![0u64; total_words],", 0,
     "The fresh output is a zeroed heap vector; allocation is inside the timed call on the gf2 side."),
    ("gf2-bitmatrix-transpose-route", "gf2", "crates/gf2-core/src/matrix.rs", "if n_row_blocks <= transpose_simple_max_blocks && n_col_blocks <= transpose_simple_max_blocks {", 0,
     "63x63, 64x64 and 65x65 have at most two 64-bit blocks per side and take the simple block route."),
    ("gf2-bitmatrix-tile-zero-pad", "gf2", "crates/gf2-core/src/matrix.rs", "for slot in tile_in.iter_mut().take(64).skip(block_rows) {", 0,
     "Partial tiles (63 and 65) are zero-padded into a full 64x64 tile before the kernel runs; 65x65 therefore runs four tile transposes."),
    ("gf2-bitmatrix-tile-kernel-call", "gf2", "crates/gf2-core/src/matrix.rs", "transpose_64x64(&tile_in, &mut tile_out);", 0,
     "Every block of the tile grid costs one full 64x64 kernel call plus its tile load and store, so the 2x2 grid of 65x65 pays four although three of its tiles carry one row or one column of data."),
    ("gf2-bitmatrix-transpose-simple-threshold", "gf2", "crates/gf2-core/src/matrix.rs", "pub(crate) const TRANSPOSE_CACHE_TILE_THRESHOLD_BLOCKS: usize = 16;", 0,
     "The simple block route holds up to 16 blocks per side in the conservative profile, so every surveyed geometry takes it."),
    # gf2 logical route
    ("gf2-xor-inplace", "gf2", "crates/gf2-core/src/kernels/ops.rs", "pub fn xor_inplace(dst: &mut [u64], src: &[u64]) {", 0,
     "The production API accumulates into `dst`; the fresh-output arrangement copies source 0 into the destination first."),
    ("gf2-xor-dispatch-threshold", "gf2", "crates/gf2-core/src/kernels/backend.rs", "pub(crate) const SIMD_MIN_WORDS_DEFAULT: usize = 8;", 0,
     "Buffers of fewer than eight words take the scalar backend; 7, 8 and 9 words straddle this threshold."),
    ("gf2-xor-dispatch-compare", "gf2", "crates/gf2-core/src/kernels/backend.rs", "if _size >= SIMD_MIN_WORDS {", 0,
     "The size comparison that selects the SIMD backend."),
    ("gf2-avx2-xor-kernel", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", "unsafe fn avx2_xor_into(dst: &mut [u64], src: &[u64]) {", 0,
     "The SIMD backend's XOR kernel, four words per 256-bit vector with a scalar tail."),
    ("gf2-avx2-xor-instruction", "gf2", "crates/gf2-kernels-simd/src/x86/avx2.rs", "let r = _mm256_xor_si256(a, b);", 0,
     "The AVX2 instruction the disassembly probe counts as ymm operations in gf2_logical_xor_arm."),
    ("gf2-simd-feature-default", "gf2", "crates/gf2-coding/Cargo.toml", 'default = ["simd", "sim-observability"]', 0,
     "Production consumers in gf2-coding enable gf2-core's simd feature by default; the survey crate names it explicitly."),
    # gf2 BCH routes
    ("gf2-bch-generator-matrix-trait", "gf2", "crates/gf2-coding/src/traits.rs", "fn generator_matrix(&self) -> Result<Self::GeneratorMatrix, CodeError> {", 0,
     "The production materialization entry point measured by the `materialize` cells."),
    ("gf2-bch-generator-matrix-impl", "gf2", "crates/gf2-coding/src/bch/matrix.rs", "fn generator_matrix_into(&self, out: &mut Self::GeneratorMatrix) -> Result<(), CodeError> {", 0,
     "BchCode's materialization writes the generator through write_generator, which consults only the generator polynomial."),
    ("gf2-bch-write-generator", "gf2", "crates/gf2-coding/src/bch/matrix.rs", "fn write_generator<X, S, M>(code: &BchCode<X, S, M>, out: &mut M) -> Result<(), CodeError>", 0,
     "The production writer behind generator_matrix."),
    ("gf2-bch-reference-oracle", "gf2", "crates/gf2-coding/src/bch/matrix.rs", "pub(crate) fn write_generator_by_encoding<X, S, M>(", 0,
     "The test-support oracle the protocol-v1 pilot measured as the gf2 arm; it encodes each of the k basis vectors."),
    ("gf2-bch-reference-encodes-per-row", "gf2", "crates/gf2-coding/src/bch/matrix.rs", "code.encode_into(&message, &mut codeword)?;", 0,
     "One systematic encode per generator row: the O(k^2 r) cost the oracle's own documentation states."),
    ("gf2-bch-reference-complexity", "gf2", "crates/gf2-coding/src/bch/matrix.rs", "/// $O(k^2 r)$ base-field operations where the materialization walks the", 0,
     "The oracle documents its own cost as O(k^2 r) against a materialization that walks the output once."),
    ("gf2-bch-bench-materialize", "gf2", "crates/gf2-coding/benches/bch_genmatrix.rs", 'group.bench_function(BenchmarkId::new("materialize/fresh-alloc", row.name), |b| {', 0,
     "The established bench's production row, which the confirmatory BCH cells reproduce."),
    ("gf2-bch-bench-reference", "gf2", "crates/gf2-coding/benches/bch_genmatrix.rs", 'group.bench_function(BenchmarkId::new("reference/fresh-alloc", row.name), |b| {', 0,
     "The established bench's oracle row, which the exploratory reference cells reproduce."),
    # M4RI
    ("m4ri-license-header", "m4ri", "m4ri-src/m4ri/mzd.h", "*  Distributed under the terms of the GNU General Public License (GPL)", 0,
     "Source headers license M4RI under the GPL, version 2 or higher (next line)."),
    ("m4ri-license-header-version", "m4ri", "m4ri-src/m4ri/mzd.h", "*  version 2 or higher.", 0,
     "Together with the previous line: GPL-2.0-or-later."),
    ("m4ri-license-readme", "m4ri", "m4ri-src/README.md", "M4RI is available under the General Public License Version 2 or later (GPLv2+).", 0,
     "The README states GPLv2+; COPYING carries the GPLv2 text."),
    ("m4ri-transpose-entry", "m4ri", "m4ri-src/m4ri/mzd.c", "mzd_t *mzd_transpose(mzd_t *DST, mzd_t const *A) {", 0,
     "The M4RI transpose entry point both M4RI transpose arms call."),
    ("m4ri-transpose-fresh-output", "m4ri", "m4ri-src/m4ri/mzd.c", "DST = mzd_init(A->ncols, A->nrows);", 0,
     "With DST == NULL the consumer cells allocate a fresh output inside every call, matching gf2's BitMatrix::transpose."),
    ("m4ri-transpose-odd-whole-block", "m4ri", "m4ri-src/m4ri/mzd.c", "int js = ncols & nrows & 64;  // True if the total number of whole 64x64 matrices is odd.", 0,
     "At 64x64 and 65x65 the single whole 64x64 block is transposed on its own by _mzd_copy_transpose_64x64."),
    ("m4ri-transpose-64xlt64-strip", "m4ri", "m4ri-src/m4ri/mzd.c", "_mzd_copy_transpose_64xlt64(fwd + whole_64cols * rowstride_64_dst, fws + whole_64cols,", 0,
     "The column remainder of a 65-column matrix (a 64x1 strip) goes to a routine sized to the remaining columns, not to a full 64x64 tile."),
    ("m4ri-transpose-lt64x64-strip", "m4ri", "m4ri-src/m4ri/mzd.c", "_mzd_copy_transpose_lt64x64(fwd, fws, rowstride_dst, rowstride_src, nrows);", 0,
     "The row remainder of a 65-row matrix (a 1x64 strip) goes to a routine sized to the remaining rows."),
    ("m4ri-transpose-small-corner", "m4ri", "m4ri-src/m4ri/mzd.c", "_mzd_copy_transpose_small(fwd, fws, rowstride_dst, rowstride_src, nrows, ncols, maxsize);", 0,
     "The remaining corner (1x1 at 65x65) takes the small-matrix routines."),
    ("m4ri-transpose-small-path", "m4ri", "m4ri-src/m4ri/mzd.c", "if (maxsize < 64) {  // super-fast path for very small matrices", 0,
     "Every matrix below 64x64, such as 63x63, goes to the small-matrix routines instead of a padded 64x64 tile."),
    ("m4ri-rowstride-even", "m4ri", "m4ri-src/m4ri/mzd.c", "A->rowstride     = ((A->width & 1) == 0) ? A->width : A->width + 1;", 0,
     "M4RI pads every row to an even word count, so a 64-column matrix has a 128-byte row stride (gf2: 64 bytes)."),
    ("m4ri-mmc-calloc", "m4ri", "m4ri-src/m4ri/mzd.c", "A->data = m4ri_mmc_calloc(r, sizeof(word) * A->rowstride);", 0,
     "Matrix storage comes from M4RI's own memory cache, so per-call allocation is a cached block reuse rather than a malloc."),
    ("m4ri-mmc-enabled", "m4ri", "prefix/include/m4ri/m4ri_config.h", "#define __M4RI_ENABLE_MMC               1", 0,
     "The pinned build enables that cache."),
    ("m4ri-mmc-cache-lookup", "m4ri", "m4ri-src/m4ri/mmc.c", "if (mm[i].size == size) {", 0,
     "The cache hands back a previously freed block of the same size; consecutive fresh-output calls of one geometry hit it."),
    ("m4ri-excess-bits-policy", "m4ri", "m4ri-src/m4ri/mzd.h", "* if the matrix is not a window, the excess bits MUST be zero. In this case, they", 0,
     "M4RI keeps padding bits zero, so 63- and 65-column outputs carry canonical zero tails."),
    ("m4ri-echelonize-entry", "m4ri", "m4ri-src/m4ri/echelonform.h", "rci_t mzd_echelonize_m4ri(mzd_t *A, int full, int k);", 0,
     "The RREF entry point of the M4RI generator-matrix arm (full = 1, k = 0 selects k automatically)."),
    # Bitshuffle
    ("bitshuffle-compile-time-avx2", "bitshuffle", "src/bitshuffle_core.c", "#if defined(__AVX2__) && defined (__SSE2__)", 0,
     "The AVX2 route is chosen at compile time from -march=native; there is no runtime dispatch (no cpuid in the archive)."),
    ("bitshuffle-dispatcher", "bitshuffle", "src/bitshuffle_core.c", "count = bshuf_trans_bit_elem_AVX(in, out, size, elem_size);", 0,
     "bshuf_trans_bit_elem forwards to the AVX2 implementation in this build."),
    ("bitshuffle-avx-entry", "bitshuffle", "src/bitshuffle_core.c", "int64_t bshuf_trans_bit_elem_AVX(const void* in, void* out, const size_t size,", 0,
     "The AVX2 bit-transpose routine."),
    ("bitshuffle-avx-malloc-per-call", "bitshuffle", "src/bitshuffle_core.c", "void* tmp_buf = malloc(size * elem_size);", 3,
     "The AVX2 routine mallocs and frees a scratch buffer inside every call, a fixed per-call cost that the kernel-isolated comparison exposes."),
    ("bitshuffle-multiple-of-eight", "bitshuffle", "src/bitshuffle_core.c", "#define CHECK_MULT_EIGHT(n) do { if ((n) % 8) return -80; } while (0)", 0,
     "Element counts that are not multiples of eight are rejected, which makes unpadded 63- and 65-row transposes unavailable."),
    ("bitshuffle-block-multiple", "bitshuffle", "src/bitshuffle_core.c", "if (block_size % BSHUF_BLOCKED_MULT) return -81;", 0,
     "Block sizes must be multiples of eight; the adapter uses one block of the padded row count."),
    ("bitshuffle-blocked-mult", "bitshuffle", "src/bitshuffle_internals.h", "#define BSHUF_BLOCKED_MULT 8", 0,
     "The multiple the block and element counts must satisfy."),
    ("bitshuffle-using-avx2", "bitshuffle", "src/bitshuffle_core.h", "int bshuf_using_AVX2(void);", 0,
     "The library predicate the harness's --backend output reports."),
    # ISA-L
    ("isal-xor-gen-arity", "isa-l", "include/raid.h", "* @param vects   Number of source+dest vectors in array. Must be > 2.", 0,
     "xor_gen takes vects = sources + 1 with at least two sources; the sized cells use vects 3 and the parity cell vects 4."),
    ("isal-xor-gen-dest-last", "isa-l", "include/raid.h", "*                the last pointer. ie array[vects-1]. Src and dest", 0,
     "The destination is the last pointer of the array; the operation writes a fresh destination."),
    ("isal-xor-gen-alignment", "isa-l", "include/raid.h", "*                pointers must be aligned to 32B.", 0,
     "Source and destination pointers must be 32-byte aligned; other alignments are outside the contract and recorded unavailable."),
    ("isal-xor-gen-declaration", "isa-l", "include/raid.h", "xor_gen(int vects, int len, void **array);", 0,
     "The public multi-binary entry point, unavailable in this build."),
    ("isal-xor-gen-base-declaration", "isa-l", "include/raid.h", "xor_gen_base(int vects, int len, void **array);", 0,
     "The portable reference declared by the same header with the same contract."),
    ("isal-xor-gen-base-definition", "isa-l", "raid/raid_base.c", "xor_gen_base(int vects, int len, void **array)", 0,
     "The measured routine."),
    ("isal-xor-gen-base-byte-loop", "isa-l", "raid/raid_base.c", "src[vects - 1][i] = parity; // last pointer is dest", 0,
     "A byte-wise loop writing the destination last; the disassembly probe shows GCC did not vectorize it (33 general-purpose instructions)."),
    ("isal-multibinary-dispatch", "isa-l", "raid/raid_multibinary.asm", "mbin_dispatch_init6 xor_gen, xor_gen_base, xor_gen_sse, xor_gen_avx, xor_gen_avx, xor_gen_avx512", 0,
     "The public xor_gen dispatches to NASM-assembled SSE/AVX/AVX-512 kernels, none of which can be built without NASM on this host."),
    # Survey harness probes (outside the timed windows)
    ("survey-gf2-copy-probe", "gf2", "dev/active/6fb89a3c/survey/gf2-side/src/bin/gf2_logical_xor_arm.rs", "black_box(&mut *dest).copy_from_slice(black_box(src0));", 0,
     "gf2's arrangement probe (pack_ns) repeats the destination copy of the fresh-output call one million times behind black_box, so every repetition copies."),
    ("survey-isal-arrangement-probe-barrier", "gf2", "dev/active/6fb89a3c/survey/isal_xor_arm.c", '__asm__ volatile("" : : "r"(array) : "memory");', 0,
     "ISA-L's arrangement probe (dispatch_ns) must store the whole pointer array in every repetition; without this barrier GCC deleted the pilot's probe loop, which then reported 0 ns."),
    ("survey-isal-timed-arrangement", "gf2", "dev/active/6fb89a3c/survey/isal_xor_arm.c", "for (size_t s = 0; s < sources; s++) array[s] = src[s];", 0,
     "The timed ISA-L call forms the same pointer array before xor_gen_base; the probe times that formation alone."),
    ("survey-bitshuffle-pack-probe", "gf2", "dev/active/6fb89a3c/survey/bitshuffle_transpose_arm.c", "static uint64_t pack_probe_ns(bit_ctx *ctx)", 0,
     "The Bitshuffle adapter's pack probe averages 100000 warm repetitions of the pack copy the consumer call performs; the unpack probe follows it."),
    ("survey-m4ri-genmatrix-reduction-probe", "gf2", "dev/active/6fb89a3c/survey/m4ri_genmatrix_arm.c", "start = monotonic_ns(); mzd_echelonize_m4ri(probe, 1, 0); dispatch_ns = monotonic_ns() - start; mzd_free(probe);", 0,
     "The M4RI generator-matrix arm reports one reduction of a fresh unreduced copy in the dispatch_ns field and one mzd_copy in pack_ns; the tables label them as the reduction and the copy."),
]


def locate(project: dict, path: str, needle: str, occurrence: int) -> tuple[int, str]:
    lines = (project["root"] / path).read_text(errors="replace").splitlines()
    hits = [(number, line) for number, line in enumerate(lines, start=1) if needle in line]
    if len(hits) <= occurrence:
        raise SystemExit(f"{path}: substring {needle!r} occurrence {occurrence} not found ({len(hits)} hits)")
    return hits[occurrence]


def main() -> None:
    claims = []
    for claim, project_name, path, needle, occurrence, why in CLAIMS:
        project = PROJECTS[project_name]
        line, text = locate(project, path, needle, occurrence)
        claims.append({"claim": claim, "project": project_name, "revision": project["revision"], "path": path,
                       "line": line, "text": text.strip(), "why": why})
    document = {
        "schema": "comparator-source-evidence-v1",
        "issue": "6fb89a3c",
        "projects": {name: {"revision": p["revision"], "revision_kind": p["revision_kind"]} for name, p in PROJECTS.items()},
        "claims": claims,
    }
    (ROOT / "source-evidence.json").write_text(json.dumps(document, indent=2) + "\n")
    print(f"{len(claims)} claims -> {ROOT / 'source-evidence.json'}")


if __name__ == "__main__":
    main()
