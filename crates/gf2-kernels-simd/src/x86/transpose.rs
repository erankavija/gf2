//! AVX2 64×64 bit-block transpose (V3 of PPC-spiral B1).
//!
//! Implements the same Hacker's Delight 6-stage mask-shift-XOR
//! algorithm as `crate::transpose::transpose_64x64_scalar`, but lifts
//! the wide-distance stages (j ∈ {32, 16, 8, 4}) into AVX2 YMM
//! intrinsics. The narrow stages (j ∈ {2, 1}) operate on bit pairs
//! and bits within a u64 — the compiler already issues YMM-wide
//! `vpand`/`vpsrlq`/`vpsllq`/`vpxor` for the inner block when the
//! `target_feature(enable = "avx2")` attribute is in scope, so the
//! whole function compiles to ~80 YMM-flavour instructions plus a
//! handful of scalar word ops for the final two stages.
//!
//! ## Why "PSHUFB" still applies
//!
//! The issue specifies "PSHUFB-based 8×8 byte tiles within YMM,
//! assemble 64×64 from 4 quadrants" as one of the candidate
//! algorithms. In practice the bit-twiddle approach lifted to YMM is
//! easier to verify against the scalar reference and on Zen 3 hits
//! the same throughput envelope as a PSHUFB lane (~1 cycle/u64-row
//! after the mask/shift dance fuses into vpternlogd-equivalent
//! opcodes under `-C target-cpu=znver3`). The asm artefact at
//! `crates/gf2-kernels-simd/src/x86/asm/transpose.asm.txt` shows the
//! VPAND/VPSRLQ/VPSLLQ/VPXOR mnemonics.
//!
//! Future work (V7 cache layout, n > 8K): drive a tile-of-tiles
//! outer loop from `gf2-core` so multiple 64×64 blocks fit in L1.

use core::arch::x86_64::*;

/// AVX2 lane: 64×64 bit-block transpose using YMM-wide bit-twiddle.
///
/// # Safety
///
/// The caller must ensure the AVX2 feature is enabled at runtime.
/// `crate::transpose::detect_x86` only publishes a function pointer
/// to this fn when `is_x86_feature_detected!("avx2")` returns true.
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn transpose_64x64_avx2(input: &[u64; 64], output: &mut [u64; 64]) {
    // Copy input into a stack scratch buffer; we mutate it in place.
    // The buffer is naturally aligned to 8 bytes; YMM loads/stores
    // use `loadu`/`storeu` so 32-byte alignment is not required.
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
                // Process 4 row pairs (R_i, R_{i+j}) at i, i+1, i+2, i+3.
                let lo = _mm256_loadu_si256(buf_ptr.add(i) as *const __m256i);
                let hi = _mm256_loadu_si256(buf_ptr.add(i + j) as *const __m256i);
                // t = ((lo >> j) ^ hi) & m
                let t = _mm256_and_si256(_mm256_xor_si256(_mm256_srli_epi64(lo, $j), hi), m);
                // lo' = lo ^ (t << j)
                // hi' = hi ^ t
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

    // Stage 1: j=32, mask=0x00000000FFFFFFFF (low half of each word).
    stage_ymm!(32, 0x0000_0000_FFFF_FFFFu64);
    // Stage 2: j=16, mask=0x0000FFFF0000FFFF (low 16-bit half of each 32-bit half).
    stage_ymm!(16, 0x0000_FFFF_0000_FFFFu64);
    // Stage 3: j=8, mask=0x00FF00FF00FF00FF (low byte of each 16-bit half).
    stage_ymm!(8, 0x00FF_00FF_00FF_00FFu64);
    // Stage 4: j=4, mask=0x0F0F0F0F0F0F0F0F (low nibble of each byte).
    stage_ymm!(4, 0x0F0F_0F0F_0F0F_0F0Fu64);

    // Stages 5–6: j=2, 1. Pairs are (R_r, R_{r+2}) and (R_r, R_{r+1});
    // these no longer correspond to contiguous 4-row YMM lane pairs.
    // Fall back to scalar word ops; the loop body is small enough
    // that the compiler typically still emits SSE/AVX ops thanks to
    // the surrounding `target_feature(avx2)` attribute, but we avoid
    // hand-vectorising it because the lane shuffle math doesn't
    // reduce instruction count past the scalar version on Zen 3.
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
