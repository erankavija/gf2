//! AVX2 64×64 bit-block transpose lanes.
//!
//! Every kernel here answers the block contract [`crate::transpose`] states
//! and is reached only through [`crate::transpose::lane`], which publishes a
//! safe pointer to it once `is_x86_feature_detected!("avx2")` holds.
//! `src/x86/asm/transpose.asm.txt` is the release disassembly of all four.

use core::arch::x86_64::*;

/// AVX2 lane: the four wide stages in YMM registers over a stack copy.
///
/// Runs the mask-shift-XOR recursion of `@/citation/Warren2012` Section 7-3.
/// The block is copied into a 512-byte local the stages mutate in place, and
/// copied back into the caller's output at the end.
///
/// # Safety
///
/// The caller must ensure the AVX2 feature is enabled at runtime.
/// `crate::transpose::lane` only publishes a function pointer to this fn
/// when `is_x86_feature_detected!("avx2")` returns true.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn transpose_64x64_avx2(input: &[u64; 64], output: &mut [u64; 64]) {
    // The buffer is aligned to 8 bytes; YMM loads/stores use
    // `loadu`/`storeu`, so 32-byte alignment is not required.
    let mut buf: [u64; 64] = *input;
    let buf_ptr = buf.as_mut_ptr();

    // Generic stage helper: at distance `j` with mask `m`, pair rows
    // (R_i, R_{i+j}) for i ∈ [0, j), repeated every 2j rows. The
    // bit-twiddle:
    //
    //   t = ((R_i >> j) ^ R_{i+j}) & m
    //   R_i      ^= t << j
    //   R_{i+j}  ^= t
    //
    // For j ≥ 4, four consecutive rows fit in a single YMM register;
    // the lo group is contiguous at offset i and the hi group is
    // contiguous at offset i+j, so a pair of YMM loads pulls them
    // into matching lanes. We process 4 rows at a time per YMM op.

    macro_rules! stage_ymm {
        ($j:expr, $mask:expr) => {{
            let j: usize = $j;
            let m = _mm256_set1_epi64x($mask as i64);
            let mut i = 0usize;
            while i < 64 {
                let lo = _mm256_loadu_si256(buf_ptr.add(i) as *const __m256i);
                let hi = _mm256_loadu_si256(buf_ptr.add(i + j) as *const __m256i);
                let t = _mm256_and_si256(_mm256_xor_si256(_mm256_srli_epi64(lo, $j), hi), m);
                let lo_new = _mm256_xor_si256(lo, _mm256_slli_epi64(t, $j));
                let hi_new = _mm256_xor_si256(hi, t);
                _mm256_storeu_si256(buf_ptr.add(i) as *mut __m256i, lo_new);
                _mm256_storeu_si256(buf_ptr.add(i + j) as *mut __m256i, hi_new);
                i += 4;
                // Skip the upper half of the just-handled 2j-block.
                if i % (2 * j) == j {
                    i += j;
                }
            }
        }};
    }

    stage_ymm!(32, 0x0000_0000_FFFF_FFFFu64);
    stage_ymm!(16, 0x0000_FFFF_0000_FFFFu64);
    stage_ymm!(8, 0x00FF_00FF_00FF_00FFu64);
    stage_ymm!(4, 0x0F0F_0F0F_0F0F_0F0Fu64);

    // Stages 5–6: j=2, 1. Pairs are (R_r, R_{r+2}) and (R_r, R_{r+1}),
    // which are not contiguous 4-row YMM lane pairs, so the stages run as
    // scalar word ops.
    let masks: [(usize, u64); 2] = [(2, 0x3333_3333_3333_3333), (1, 0x5555_5555_5555_5555)];
    for &(j, m) in &masks {
        let mut i = 0usize;
        while i < 64 {
            let mut r = i;
            while r < i + j {
                let a = buf[r];
                let b = buf[r + j];
                let t = ((a >> j) ^ b) & m;
                buf[r] = a ^ (t << j);
                buf[r + j] = b ^ t;
                r += 1;
            }
            i += 2 * j;
        }
    }

    *output = buf;
}

/// AVX2 PSHUFB lane: transpose 64×64 as 8×8 byte tiles.
///
/// This path uses `vpshufb` as a byte-local bit-reversal LUT, then assembles
/// each 8×8 transposed tile into the corresponding output byte with a scalar
/// bit loop. It is the explicit PSHUFB candidate of the lane family that
/// [`crate::transpose::TransposeLane`] names.
///
/// # Safety
///
/// The caller must ensure the AVX2 feature is enabled at runtime.
/// `crate::transpose::lane` only publishes a function pointer to this fn
/// when `is_x86_feature_detected!("avx2")` returns true.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn transpose_64x64_avx2_pshufb(input: &[u64; 64], output: &mut [u64; 64]) {
    let mut bytes = [0u8; 64 * 8];
    for (row, word) in input.iter().enumerate() {
        bytes[row * 8..row * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }

    let mut bit_reversed = [0u8; 64 * 8];
    let lut = _mm256_setr_epi8(
        0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3, 11, 7, 15, 0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5,
        13, 3, 11, 7, 15,
    );
    let lo_mask = _mm256_set1_epi8(0x0f);
    let hi_mask = _mm256_set1_epi8(0xf0u8 as i8);

    for (src, dst) in bytes
        .chunks_exact(32)
        .zip(bit_reversed.chunks_exact_mut(32))
    {
        let v = _mm256_loadu_si256(src.as_ptr() as *const __m256i);
        let lo = _mm256_and_si256(v, lo_mask);
        let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), lo_mask);
        let rev_lo = _mm256_shuffle_epi8(lut, lo);
        let rev_hi = _mm256_shuffle_epi8(lut, hi);
        let rev = _mm256_or_si256(
            _mm256_and_si256(_mm256_slli_epi16(rev_lo, 4), hi_mask),
            rev_hi,
        );
        _mm256_storeu_si256(dst.as_mut_ptr() as *mut __m256i, rev);
    }

    let mut out = [0u64; 64];
    for row_block in 0..8 {
        for col_block in 0..8 {
            for row_in_block in 0..8 {
                let row = row_block * 8 + row_in_block;
                let row_bit = 1u64 << row;
                let rev_byte = bit_reversed[row * 8 + col_block];
                for col_in_block in 0..8 {
                    if (rev_byte >> (7 - col_in_block)) & 1 != 0 {
                        out[col_block * 8 + col_in_block] |= row_bit;
                    }
                }
            }
        }
    }

    *output = out;
}

/// AVX2 lane: 64×64 bit-block transpose with every stage in YMM registers.
///
/// The algorithm is the same six-stage mask-shift-XOR recursion as
/// [`transpose_64x64_avx2`]. The first stage reads the caller's `input` and
/// writes the caller's `output` and the remaining five run in place on
/// `output`, so no block is copied through a stack scratch; the narrow
/// stages (j ∈ {2, 1}) run four rows at a time through `vpermq`, `vpshufd`
/// and `vpblendd`.
///
/// # Safety
///
/// The caller must ensure the AVX2 feature is enabled at runtime.
/// `crate::transpose::lane` only publishes a function pointer to this fn
/// when `is_x86_feature_detected!("avx2")` returns true.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn transpose_64x64_avx2_ymm6(input: &[u64; 64], output: &mut [u64; 64]) {
    let inp = input.as_ptr();
    let outp = output.as_mut_ptr();

    // Stage 1 (j = 32) reads the caller's input and writes the caller's
    // output: the whole kernel's only pass over `input`, and the pass that
    // makes the remaining stages in-place work on `output`.
    {
        let m = _mm256_set1_epi64x(0x0000_0000_FFFF_FFFFu64 as i64);
        let mut i = 0usize;
        while i < 32 {
            let lo = _mm256_loadu_si256(inp.add(i) as *const __m256i);
            let hi = _mm256_loadu_si256(inp.add(i + 32) as *const __m256i);
            let t = _mm256_and_si256(_mm256_xor_si256(_mm256_srli_epi64(lo, 32), hi), m);
            _mm256_storeu_si256(
                outp.add(i) as *mut __m256i,
                _mm256_xor_si256(lo, _mm256_slli_epi64(t, 32)),
            );
            _mm256_storeu_si256(outp.add(i + 32) as *mut __m256i, _mm256_xor_si256(hi, t));
            i += 4;
        }
    }

    // Stages 2–4 (j ∈ {16, 8, 4}) keep pairing contiguous four-row runs, now
    // in place on the output.
    macro_rules! stage_wide {
        ($j:expr, $mask:expr) => {{
            let j: usize = $j;
            let m = _mm256_set1_epi64x($mask as i64);
            let mut i = 0usize;
            while i < 64 {
                let lo = _mm256_loadu_si256(outp.add(i) as *const __m256i);
                let hi = _mm256_loadu_si256(outp.add(i + j) as *const __m256i);
                let t = _mm256_and_si256(_mm256_xor_si256(_mm256_srli_epi64(lo, $j), hi), m);
                _mm256_storeu_si256(
                    outp.add(i) as *mut __m256i,
                    _mm256_xor_si256(lo, _mm256_slli_epi64(t, $j)),
                );
                _mm256_storeu_si256(outp.add(i + j) as *mut __m256i, _mm256_xor_si256(hi, t));
                i += 4;
                // Skip the upper half of the just-handled 2j-block.
                if i % (2 * j) == j {
                    i += j;
                }
            }
        }};
    }
    stage_wide!(16, 0x0000_FFFF_0000_FFFFu64);
    stage_wide!(8, 0x00FF_00FF_00FF_00FFu64);
    stage_wide!(4, 0x0F0F_0F0F_0F0F_0F0Fu64);

    // Stage 5 (j = 2): within rows [i, i+4) the pairs are (i, i+2) and
    // (i+1, i+3), so `vpermq` puts rows (i, i+1) in both halves of one
    // register and rows (i+2, i+3) in both halves of the other. The low
    // half of the updated low operand and the high half of the updated high
    // operand are the four result rows.
    {
        let m = _mm256_set1_epi64x(0x3333_3333_3333_3333u64 as i64);
        let mut i = 0usize;
        while i < 64 {
            let v = _mm256_loadu_si256(outp.add(i) as *const __m256i);
            let lo = _mm256_permute4x64_epi64(v, 0b01_00_01_00);
            let hi = _mm256_permute4x64_epi64(v, 0b11_10_11_10);
            let t = _mm256_and_si256(_mm256_xor_si256(_mm256_srli_epi64(lo, 2), hi), m);
            let lo_new = _mm256_xor_si256(lo, _mm256_slli_epi64(t, 2));
            let hi_new = _mm256_xor_si256(hi, t);
            _mm256_storeu_si256(
                outp.add(i) as *mut __m256i,
                _mm256_blend_epi32(lo_new, hi_new, 0b1111_0000),
            );
            i += 4;
        }
    }

    // Stage 6 (j = 1): the pairs are (i, i+1) and (i+2, i+3), each inside
    // one 128-bit half, so `vpshufd` broadcasts the low quadword of each
    // half into one operand and the high quadword into the other.
    {
        let m = _mm256_set1_epi64x(0x5555_5555_5555_5555u64 as i64);
        let mut i = 0usize;
        while i < 64 {
            let v = _mm256_loadu_si256(outp.add(i) as *const __m256i);
            let lo = _mm256_shuffle_epi32(v, 0b01_00_01_00);
            let hi = _mm256_shuffle_epi32(v, 0b11_10_11_10);
            let t = _mm256_and_si256(_mm256_xor_si256(_mm256_srli_epi64(lo, 1), hi), m);
            let lo_new = _mm256_xor_si256(lo, _mm256_slli_epi64(t, 1));
            let hi_new = _mm256_xor_si256(hi, t);
            _mm256_storeu_si256(
                outp.add(i) as *mut __m256i,
                _mm256_blend_epi32(lo_new, hi_new, 0b1100_1100),
            );
            i += 4;
        }
    }
}

/// AVX2 lane: 64×64 bit-block transpose through a byte transpose and
/// `vpmovmskb` bit-plane extraction.
///
/// The input is a 64×8 matrix of bytes, byte `q` of row `r` carrying columns
/// `8q..8q+8`. The kernel first transposes it to the 8×64 byte matrix whose
/// row `q` is that byte column, eight rows at a time through the SSE
/// interleave ladder (`vpunpck{l,h}bw`, `vpunpck{l,h}wd`, `vpunpck{l,h}dq`);
/// the eight rows enter the ladder in the order that cancels the ladder's own
/// permutation, so no fixup shuffle follows it.
///
/// Each byte-transposed row is then 64 bytes whose bit `b` is the matrix
/// entry of column `8q + b`. `vpmovmskb` reads bit 7 of each of 32 bytes in
/// one instruction, so two extractions give the whole output word of column
/// `8q + 7`, and `vpaddb` of a register with itself doubles every byte and
/// steps the extraction down to the next column. Eight steps per byte column
/// produce all 64 output words.
///
/// # Safety
///
/// The caller must ensure the AVX2 feature is enabled at runtime.
/// `crate::transpose::lane` only publishes a function pointer to this fn
/// when `is_x86_feature_detected!("avx2")` returns true.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn transpose_64x64_avx2_movemask(input: &[u64; 64], output: &mut [u64; 64]) {
    // `planes[q]` is byte column `q` of all 64 rows.
    let mut planes = [[0u8; 64]; 8];

    // Byte transpose, eight rows per step. The interleave ladder emits the
    // eight rows of a byte column in the order (p, r, q, s, t, v, u, w) of
    // its four operand halves, so the rows are loaded as (0,2), (1,3),
    // (4,6), (5,7) and come out ascending.
    let mut group = 0usize;
    while group < 64 {
        let x0 = _mm_set_epi64x(input[group + 2] as i64, input[group] as i64);
        let x1 = _mm_set_epi64x(input[group + 3] as i64, input[group + 1] as i64);
        let x2 = _mm_set_epi64x(input[group + 6] as i64, input[group + 4] as i64);
        let x3 = _mm_set_epi64x(input[group + 7] as i64, input[group + 5] as i64);

        let t0 = _mm_unpacklo_epi8(x0, x1);
        let t1 = _mm_unpackhi_epi8(x0, x1);
        let t2 = _mm_unpacklo_epi8(x2, x3);
        let t3 = _mm_unpackhi_epi8(x2, x3);

        let u0 = _mm_unpacklo_epi16(t0, t1);
        let u1 = _mm_unpackhi_epi16(t0, t1);
        let u2 = _mm_unpacklo_epi16(t2, t3);
        let u3 = _mm_unpackhi_epi16(t2, t3);

        let v0 = _mm_unpacklo_epi32(u0, u2);
        let v1 = _mm_unpackhi_epi32(u0, u2);
        let v2 = _mm_unpacklo_epi32(u1, u3);
        let v3 = _mm_unpackhi_epi32(u1, u3);

        // Each half of `v0..v3` is one byte column's eight bytes for this
        // row group, in ascending byte columns 0..8.
        let columns = [
            _mm_extract_epi64(v0, 0) as u64,
            _mm_extract_epi64(v0, 1) as u64,
            _mm_extract_epi64(v1, 0) as u64,
            _mm_extract_epi64(v1, 1) as u64,
            _mm_extract_epi64(v2, 0) as u64,
            _mm_extract_epi64(v2, 1) as u64,
            _mm_extract_epi64(v3, 0) as u64,
            _mm_extract_epi64(v3, 1) as u64,
        ];
        for (q, packed) in columns.iter().enumerate() {
            planes[q][group..group + 8].copy_from_slice(&packed.to_le_bytes());
        }
        group += 8;
    }

    // Bit-plane extraction. Bit 7 of byte `r` of `planes[q]` is the matrix
    // entry of row `r` and column `8q + 7`, which is output word `8q + 7`
    // bit `r`; doubling every byte steps down to column `8q + 6`, and so on.
    for (q, plane) in planes.iter().enumerate() {
        let mut lo = _mm256_loadu_si256(plane.as_ptr() as *const __m256i);
        let mut hi = _mm256_loadu_si256(plane.as_ptr().add(32) as *const __m256i);
        for step in 0..8usize {
            let low = _mm256_movemask_epi8(lo) as u32 as u64;
            let high = _mm256_movemask_epi8(hi) as u32 as u64;
            output[q * 8 + 7 - step] = low | (high << 32);
            lo = _mm256_add_epi8(lo, lo);
            hi = _mm256_add_epi8(hi, hi);
        }
    }
}
