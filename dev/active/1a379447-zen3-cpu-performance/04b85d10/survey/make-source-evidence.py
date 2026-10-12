#!/usr/bin/env python3
"""Writes the code-claim register of the bit-storage survey (jit:04b85d10).

Every route, dispatch-boundary and scratch-versus-spill claim the findings
make about production code is a row here: project, commit, path, line, the
verbatim line text read from the checkout at generation time, and why the
line matters. The script refuses a row whose cited line does not contain the
expected fragment, so a citation cannot drift away from the claim it supports
without this file failing to regenerate.

Usage: make-source-evidence.py [--check]  (from anywhere; `--check` writes nothing
and compares the committed ledger with the checkout, ignoring the commit)
"""

import json
import pathlib
import subprocess
import sys

SURVEY = pathlib.Path(__file__).resolve().parent
REPO = pathlib.Path(
    subprocess.check_output(["git", "-C", str(SURVEY), "rev-parse", "--show-toplevel"], text=True).strip()
)
OUTPUT = SURVEY / "source-evidence.json"

# (claim id, path, line hint, identifying fragment, why)
CLAIMS = [
    # --- logical row dispatch ---
    ("row-xor-public-entry", "crates/gf2-core/src/matrix.rs", 742,
     "use crate::kernels::ops::xor_inplace;",
     "BitMatrix::row_xor takes the size-dispatched xor_inplace route, so the `ops-dispatched` arm is the production row route."),
    ("xor-inplace-resolves-per-call", "crates/gf2-core/src/kernels/ops.rs", 49,
     "let xor = resolve_xor_inplace(dst.len());",
     "xor_inplace re-selects the backend on every call; hoisting it is the `ops-resolved` arm."),
    ("resolve-xor-simd-fn", "crates/gf2-core/src/kernels/ops.rs", 30,
     ".map(|backend| backend.xor_fn)",
     "Above the cutover the resolved kernel is the detected LogicalFns::xor_fn."),
    ("simd-min-words-default", "crates/gf2-core/src/kernels/backend.rs", 83,
     "pub(crate) const SIMD_MIN_WORDS_DEFAULT: usize = 8;",
     "The conservative build's scalar/SIMD cutover is eight words; four-word rows take scalar."),
    ("select-backend-threshold", "crates/gf2-core/src/kernels/backend.rs", 95,
     "if _size >= SIMD_MIN_WORDS {",
     "select_backend_for_size compares the word count against the cutover."),
    ("baked-simd-min-words", "crates/gf2-core/src/kernels/backend.rs", 81,
     "const SIMD_MIN_WORDS: usize = SIMD_MIN_WORDS_DEFAULT;",
     "Without `--cfg gf2_tuning_baked`, which the measured build does not set, the cutover is the conservative default; a baked build selects the calibrated value."),
    ("simd-backend-xor", "crates/gf2-core/src/kernels/simd/mod.rs", 48,
     "(self.fns.xor_fn)(dst, src)",
     "The `simd-backend` arm calls the same detected xor_fn with detection hoisted."),
    ("avx2-xor-into", "crates/gf2-kernels-simd/src/x86/avx2.rs", 36,
     "unsafe fn avx2_xor_into(dst: &mut [u64], src: &[u64]) {",
     "The AVX2 row XOR kernel the detected LogicalFns publishes on this host."),
    # --- dense RREF ---
    ("rref-block-width", "crates/gf2-core/src/alg/rref.rs", 64,
     "_ => 8,",
     "rref selects an eight-wide Gray table above 512 columns, so the 1024-square row is blocked M4RI."),
    ("rref-clone", "crates/gf2-core/src/alg/rref.rs", 91,
     "let mut work = matrix.clone();",
     "RREF clones the input matrix once per call; that copy is part of the consumer's allocation traffic."),
    ("rref-resolves-xor-once", "crates/gf2-core/src/alg/rref.rs", 95,
     "let xor = crate::kernels::ops::resolve_xor_inplace(work.stride_words());",
     "RREF resolves the XOR kernel once per call for table construction."),
    ("rref-table-per-block", "crates/gf2-core/src/alg/rref.rs", 212,
     "let mut table = vec![0u64; table_rows * suffix_words];",
     "One Gray table is allocated per pivot block, which is where RREF's per-call allocations come from."),
    ("rref-row-apply", "crates/gf2-core/src/alg/rref.rs", 234,
     "work.row_xor_slice_from(row, first_word, &table[src_start..src_start + suffix_words]);",
     "Elimination rows are applied through the inline suffix XOR, not through the dispatched xor_inplace."),
    # --- dense matvec ---
    ("matvec-route", "crates/gf2-core/src/matrix.rs", 1071,
     "match matvec_route(self.stride_words) {",
     "BitMatrix::matvec dispatches on the row stride in words."),
    ("matvec-simd-min-words", "crates/gf2-core/src/matrix.rs", 18,
     "pub(crate) const MATVEC_SIMD_MIN_WORDS: usize = 8;",
     "Rows of eight or more words take the SIMD matvec; the 4096-column row has 64 words."),
    ("matvec-fused-kernel", "crates/gf2-core/src/matrix.rs", 1193,
     "y.push_bit((fns.and_popcnt_fn)(row, x_words) & 1 == 1);",
     "The SIMD matvec computes each output bit with the fused AND-popcount kernel and bit-appends it."),
    ("matvec-output-alloc", "crates/gf2-core/src/matrix.rs", 1190,
     "let mut y = crate::BitVec::with_capacity(self.rows);",
     "The matvec output is a fresh BitVec per call: the one allocation the profile counts."),
    ("matvec-scalar-private", "crates/gf2-core/src/matrix.rs", 1144,
     "fn matvec_scalar(&self, x: &crate::BitVec) -> crate::BitVec {",
     "The scalar matvec lane is private; the public entry that reaches it is matvec_with_route, which the harness does not call."),
    ("matvec-with-route", "crates/gf2-core/src/matrix.rs", 1087,
     "pub fn matvec_with_route(&self, x: &crate::BitVec, route: MatvecRoute) -> crate::BitVec {",
     "matvec_with_route runs either lane on request; the consumer survey calls only matvec."),
    ("avx2-and-popcnt", "crates/gf2-kernels-simd/src/x86/avx2.rs", 365,
     "unsafe fn avx2_and_popcnt(lhs: &[u64], rhs: &[u64]) -> u64 {",
     "The fused AND-popcount kernel the matvec profile attributes most cycles to."),
    # --- sparse LDPC syndrome ---
    ("ldpc-syndrome", "crates/gf2-coding/src/ldpc/core.rs", 68,
     "self.h.matvec(codeword)",
     "LdpcCode::syndrome is the parity-check matvec."),
    ("ldpc-h-dual", "crates/gf2-coding/src/ldpc/core.rs", 16,
     "h: SpBitMatrixDual,",
     "The parity-check matrix is the CSR/CSC dual."),
    ("ldpc-dual-matvec", "crates/gf2-core/src/sparse.rs", 1226,
     "self.csr.matvec(x)",
     "The dual's matvec delegates to the CSR side."),
    ("ldpc-csr-bit-at-a-time", "crates/gf2-core/src/sparse.rs", 992,
     "acc ^= x.get(c);",
     "The CSR matvec gathers one bit per stored index; no packed word kernel sits below it."),
    ("ldpc-syndrome-output-alloc", "crates/gf2-core/src/sparse.rs", 986,
     "let mut y = BitVec::with_capacity(self.rows);",
     "The syndrome is a fresh BitVec per call."),
    # --- population count and any-nonzero ---
    ("count-ones-entry", "crates/gf2-core/src/bitvec.rs", 411,
     "crate::kernels::ops::popcount(&self.data) as usize",
     "BitVec::count_ones is the size-dispatched popcount."),
    ("popcount-dispatch", "crates/gf2-core/src/kernels/ops.rs", 241,
     "match select_backend_for_size(buf.len()) {",
     "popcount re-selects the backend per call at the same eight-word cutover."),
    ("simd-backend-popcount", "crates/gf2-core/src/kernels/simd/mod.rs", 56,
     "(self.fns.popcnt_fn)(buf)",
     "The `simd-backend` popcount arm calls the detected popcnt_fn directly."),
    ("avx2-popcnt", "crates/gf2-kernels-simd/src/x86/avx2.rs", 307,
     "unsafe fn avx2_popcnt(buf: &[u64]) -> u64 {",
     "The AVX2 population-count kernel."),
    ("find-first-one-simd", "crates/gf2-core/src/bitvec.rs", 611,
     "return (fns.find_first_one_fn)(&self.data).filter(|&pos| pos < self.len_bits);",
     "BitVec::find_first_one reaches the detected early-exit kernel."),
    ("avx2-find-first-one", "crates/gf2-kernels-simd/src/x86/avx2.rs", 481,
     "unsafe fn avx2_find_first_one(buf: &[u64]) -> Option<usize> {",
     "The AVX2 first-set-bit kernel that can stop at the first nonzero vector."),
    ("ldpc-is-valid-count", "crates/gf2-coding/src/ldpc/core.rs", 74,
     "syndrome.count_ones() == 0",
     "LdpcCode::is_valid_codeword spells its zero test as a full population count."),
    ("ldpc-bp-early-termination", "crates/gf2-coding/src/ldpc/core.rs", 754,
     "if early_termination && self.syndrome_passes() {",
     "The LDPC BP decoder's per-iteration early-termination check is syndrome_passes, which XORs each check's hard decisions and stops at the first unsatisfied check; it takes no count."),
    ("ldpc-bp-final-check", "crates/gf2-coding/src/ldpc/core.rs", 761,
     "let syndrome_check_passed = self.syndrome_passes();",
     "The decoder's terminal syndrome check is the same early-exit routine."),
    ("orbgrand-count", "crates/gf2-coding/src/grand/orbgrand.rs", 403,
     "let is_zero = syndrome.count_ones() == 0;",
     "ORBGRAND tests each incrementally updated candidate syndrome with a full count."),
    ("bp-osd-count", "crates/gf2-coding/src/osd/bp_osd.rs", 250,
     ".is_some_and(|word| self.corrector.parity_check().matvec(word).count_ones() == 0);",
     "BP-OSD's post-correction check spells the zero test as a full count."),
    ("product-row-count", "crates/gf2-coding/src/product/mod.rs", 370,
     "if syn.count_ones() > 0 {",
     "Product-code row validity tests a full count per component row."),
    ("product-col-count", "crates/gf2-coding/src/product/mod.rs", 381,
     "if syn.count_ones() > 0 {",
     "Product-code column validity tests a full count per component column."),
    ("gldpc-component-check", "crates/gf2-coding/src/gldpc/mod.rs", 441,
     "if !self.component.is_valid_codeword(&local) {",
     "GLDPC validity checks every component through the counted spelling."),
    # --- transpose ---
    ("transpose-detect-avx2", "crates/gf2-kernels-simd/src/transpose.rs", 74,
     "[TransposeLane::Avx2BitTwiddle, TransposeLane::Scalar];",
     "transpose::detect publishes the first supported lane of PRODUCTION_PREFERENCE: the AVX2 bit-twiddle lane when AVX2 is present, else the scalar lane."),
    ("transpose-pshufb-alternative", "crates/gf2-kernels-simd/src/transpose.rs", 131,
     "TransposeLane::Avx2Pshufb => transpose_64x64_avx2_pshufb_safe,",
     "The PSHUFB lane is reachable through transpose::lane but is absent from PRODUCTION_PREFERENCE, so production detection never returns it."),
    ("dense-transpose-resolves-once", "crates/gf2-core/src/matrix.rs", 888,
     "self.transpose_with_block_kernel(Self::resolved_block_kernel())",
     "BitMatrix::transpose resolves the block kernel once per call."),
    ("dense-transpose-output-alloc", "crates/gf2-core/src/matrix.rs", 923,
     "let mut out = Self::zeros(self.cols, self.rows);",
     "The dense transpose allocates its output matrix per call."),
    ("dense-transpose-route", "crates/gf2-core/src/matrix.rs", 930,
     "let route = transpose_route(n_row_blocks, n_col_blocks);",
     "The outer-loop strategy is the profile-selected simple or macro-tiled loop."),
    ("dense-transpose-tile-scratch", "crates/gf2-core/src/matrix.rs", 1000,
     "let mut tile_in = [0u64; 64];",
     "The tiled driver holds one 64-word input block and one output block: intentional scratch, not a spill."),
    ("transpose-scalar-scratch", "crates/gf2-kernels-simd/src/transpose.rs", 194,
     "let mut buf: [u64; 64] = *input;",
     "The portable 64x64 transpose copies the block into a local it mutates in place: intentional scratch."),
    ("transpose-avx2-scratch", "crates/gf2-kernels-simd/src/x86/transpose.rs", 36,
     "let mut buf: [u64; 64] = *input;",
     "The AVX2 64x64 transpose holds the same local block: intentional scratch."),
    # --- packed BCH batch ---
    ("bch-batch-into-selects", "crates/gf2-coding/src/bch/encode.rs", 1920,
     "let family = select_family::<X::Base, S>(&plan, messages.len());",
     "encode_batch_into selects the family once per batch through the canonical selector."),
    ("bch-batch-into-delegates", "crates/gf2-coding/src/bch/encode.rs", 1921,
     "self.encode_batch_family_into(family, messages, layout, workspace, codewords)",
     "The current route and the forced-family arms share encode_batch_family_into."),
    ("bch-family-into-partition", "crates/gf2-coding/src/bch/encode.rs", 1958,
     "encode_partition(family, &plan, messages, &mut workspace.registers, codewords);",
     "encode_batch_family_into runs one partition over the caller's workspace."),
    ("bch-family-admission", "crates/gf2-coding/src/bch/encode.rs", 450,
     "EncodeFamily::PolyRemainderScalar => true,",
     "The reference family is always admitted; the others need the profile's minimum batch."),
    ("bch-bitslice-admission", "crates/gf2-coding/src/bch/encode.rs", 456,
     "batch_len >= selectors.bitslice_interleaved_min_batch()",
     "Bitslice is admitted only above the active profile's minimum batch."),
    ("bch-fold-admission", "crates/gf2-coding/src/bch/encode.rs", 458,
     "EncodeFamily::ClmulFold => batch_len >= selectors.clmul_fold_min_batch(),",
     "Fold is admitted only above the active profile's minimum batch."),
    ("bch-reference-serial-reduce", "crates/gf2-coding/src/bch/encode.rs", 1116,
     "packed_serial_reduce(plan, message, register, low);",
     "The reference family's per-message recurrence."),
    ("bch-reference-place", "crates/gf2-coding/src/bch/encode.rs", 1117,
     "packed_place_codeword(plan, message, register, codeword);",
     "Every per-frame family places the codeword through packed_place_codeword."),
    ("bch-place-word-wise", "crates/gf2-coding/src/bch/encode.rs", 1717,
     "codeword.write_bits(0, message.words(), dimension);",
     "packed_place_codeword writes the message as one packed run at coordinate 0 and the parity as packed runs, not bit by bit."),
    ("bch-fold-reduce", "crates/gf2-coding/src/bch/encode.rs", 1246,
     "packed_fold_reduce(plan, message, register, low, fold[0]);",
     "The fold family reduces through packed_fold_reduce before the shared write."),
    ("bch-bitslice-batch", "crates/gf2-coding/src/bch/encode.rs", 1298,
     "packed_bitslice_batch(plan, messages, &mut registers.lanes, codewords);",
     "The bitslice family reduces whole lane groups through packed_bitslice_batch."),
    ("bch-allocating-entry", "crates/gf2-coding/src/bch/encode.rs", 2015,
     "let mut codewords = vec![S::zeroed(plan.length(), plan.symbol_zero()); messages.len()];",
     "encode_batch allocates the output collection per call."),
    ("bch-allocating-scratch", "crates/gf2-coding/src/bch/encode.rs", 2018,
     "with_encode_scratch(|registers: &mut EncodeRegisters<S::Word>| {",
     "encode_batch reduces over thread-local scratch instead of a caller workspace."),
    ("bch-parallel-selects-once", "crates/gf2-coding/src/bch/encode.rs", 2080,
     "let family = select_family::<X::Base, S>(&plan, messages.len());",
     "encode_batch_parallel_into selects the family once and offers no forced-family variant."),
    ("bch-parallel-join", "crates/gf2-coding/src/bch/encode.rs", 2337,
     "rayon::join(",
     "Partitions recurse through rayon::join under the parallel feature."),
    ("bch-unpack-parity", "crates/gf2-kernels-simd/src/bch_encode.rs", 189,
     "pub fn unpack_parity(&self, register: &[u64], redundancy: usize, parity: &mut [u64]) {",
     "The bitslice family reads parity back through a 64x64 block transpose per parity word."),
    ("bitslice-reduce-avx2", "crates/gf2-kernels-simd/src/x86/bch_encode.rs", 40,
     "pub(crate) unsafe fn bitslice_reduce_avx2(register: &mut [u64], masks: &[u64], slices: &[u64]) {",
     "The AVX2 bitslice reduction declares no local buffer: its frame traffic is register pressure."),
    ("fold-block-pclmul", "crates/gf2-kernels-simd/src/x86/bch_encode.rs", 110,
     "pub(crate) unsafe fn fold_block_pclmul(",
     "The PCLMULQDQ fold block declares no local buffer: its frame traffic is register pressure."),
    ("bitslice-reduce-scalar", "crates/gf2-kernels-simd/src/bch_encode.rs", 583,
     "pub fn bitslice_reduce_scalar(register: &mut [u64], masks: &[u64], slices: &[u64]) {",
     "The scalar bitslice reduction declares no local buffer."),
    ("fold-block-scalar", "crates/gf2-kernels-simd/src/bch_encode.rs", 489,
     "pub fn fold_block_scalar(",
     "The scalar fold block declares no local buffer."),
    ("packed-table-reduce-def", "crates/gf2-coding/src/bch/encode.rs", 1455,
     "fn packed_table_reduce(",
     "packed_table_reduce reduces over the caller's register slice and declares no local buffer."),
    ("packed-fold-reduce-def", "crates/gf2-coding/src/bch/encode.rs", 1585,
     "fn packed_fold_reduce(",
     "packed_fold_reduce reduces over the caller's register slice and declares no local buffer."),
    # --- DVB-T2 compatibility BCH ---
    ("dvb-bch-mother-encode", "crates/gf2-coding/src/transform/mod.rs", 744,
     ".encode_into(&mother_message, &mut mother_codeword)?;",
     "Shortened::encode_into encodes the mother-length message with the mother code, so the DVB-T2 BCH code runs the packed mother encoder per message."),
    ("dvb-bch-mother-buffers", "crates/gf2-coding/src/transform/mod.rs", 735,
     "let mut mother_message = Self::Symbols::zeroed(self.mother.k(), &zero);",
     "Each call allocates a mother-length message buffer and, below it, a mother-length codeword buffer."),
    ("dvb-bch-symbol-moves", "crates/gf2-coding/src/transform/mod.rs", 750,
     "codeword.set(derived_position, symbol)?;",
     "The kept coordinates are copied from the mother codeword one symbol at a time."),
]


def main():
    commit = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "HEAD"], check=True, capture_output=True, text=True
    ).stdout.strip()
    claims = []
    errors = []
    for claim, path, hint, fragment, why in CLAIMS:
        lines = (REPO / path).read_text(encoding="utf-8").splitlines()
        # The fragment identifies the line; the hint disambiguates a fragment
        # that occurs more than once by picking the occurrence nearest to it.
        matches = [index + 1 for index, text in enumerate(lines) if fragment in text]
        if not matches:
            errors.append(f"{claim}: {path} has no line containing {fragment!r}")
            continue
        line = min(matches, key=lambda candidate: abs(candidate - hint))
        if abs(line - hint) > 12:
            errors.append(f"{claim}: {path}:{hint} names no line near {fragment!r} (found {line})")
            continue
        text = lines[line - 1]
        claims.append({
            "claim": claim,
            "project": "gf2",
            "commit": commit,
            "path": path,
            "line": line,
            "text": text.strip(),
            "why": why,
        })
    if errors:
        raise SystemExit("\n".join(errors))
    document = {
        "schema": "bit-storage-source-evidence-v1",
        "issue": "04b85d10",
        "note": (
            "Line numbers and verbatim text are read from the checkout at the "
            "recorded commit; regenerate after any change to a cited file. The "
            "commit is navigation metadata; the producing-input snapshot in "
            "each receipt is the content identity."
        ),
        "claims": claims,
    }
    if "--check" in sys.argv[1:]:
        committed = json.loads(OUTPUT.read_text(encoding="utf-8"))

        def without_commit(value):
            return {**value, "claims": [{k: v for k, v in c.items() if k != "commit"} for c in value["claims"]]}

        if without_commit(committed) != without_commit(document):
            raise SystemExit(f"{OUTPUT.name} differs from the checkout; regenerate it")
        print(f"{OUTPUT.name}: {len(claims)} claims match the checkout")
        return
    OUTPUT.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    print(f"{OUTPUT.name}: {len(claims)} claims at {commit[:8]}")


if __name__ == "__main__":
    main()
