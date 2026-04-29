//! Bit-matrix transpose kernels.
//!
//! This module exports the safe dispatch table for fast 64×64 bit-block
//! transpose, plus a register-tiled scalar fallback usable on any
//! target. Unsafe x86 AVX2 implementations are isolated in
//! [`crate::x86::transpose`]; this module only exposes safe
//! function-pointer wrappers via [`TransposeFns`] returned by
//! [`detect`]. Callers without a usable SIMD lane receive `None` and
//! must fall back to [`transpose_64x64_scalar`] (which has no CPU
//! feature requirement).
//!
//! # PPC-spiral context
//!
//! Issue `1c1c4242` (kernel B1 in `dev/plans/gf2_core_ppc_spiral.md`)
//! drives the PPC spiral for [`gf2_core::BitMatrix::transpose`]:
//!
//! - **V0**: criterion baseline pinned via `crates/gf2-core/benches/matrix_transpose.rs`.
//! - **V4**: register-tiled bit-twiddle 64×64 transpose
//!   ([`transpose_64x64_scalar`], based on Hacker's Delight ch. 7-3).
//! - **V3a**: AVX2 PSHUFB-based 8×8 byte tiles within YMM registers
//!   ([`detect_pshufb`], committed for artefact inspection).
//! - **V3b**: AVX2 YMM bit-twiddle lane
//!   ([`crate::x86::transpose::transpose_64x64_avx2`]), used by default after
//!   measurement because it outperforms the PSHUFB lane on the recovery host.
//! - **V7**: optional cache-tiling outer loop for very large
//!   matrices (driven from the `gf2-core` side).
//!
//! Both the scalar fallback and the AVX2 path operate on a 64-row
//! "bit-block": 64 contiguous u64 words, each word's bit `j`
//! interpreted as the matrix entry at column `j`. The transpose writes
//! 64 output words where bit `i` of word `j` equals bit `j` of input
//! word `i`.

/// Safe 64×64 bit-block transpose function pointer.
pub type Transpose64x64Fn = fn(&[u64; 64], &mut [u64; 64]);

/// Bundle of dispatched bit-matrix-transpose kernels.
///
/// Currently exposes a single 64×64 block primitive. Callers tile
/// arbitrary `rows × cols` matrices on top of this primitive.
#[derive(Copy, Clone)]
pub struct TransposeFns {
    /// In-place 64×64 bit-block transpose, reading from 64 input words
    /// and writing 64 output words. The input and output buffers must
    /// not overlap.
    pub transpose_64x64: Transpose64x64Fn,
    /// Human-readable tag of the chosen lane. One of
    /// `"avx2-bit-twiddle"` or `"scalar-bit-twiddle"`.
    pub name: &'static str,
}

/// Detect and return the best available bit-matrix-transpose kernels.
///
/// Returns `None` only when no SIMD lane is available; callers may
/// instead always use [`transpose_64x64_scalar`] directly, which has
/// no CPU feature requirement.
///
/// On x86_64, prefers AVX2 bit-twiddle → scalar fallback.
///
/// # Examples
///
/// ```
/// if let Some(fns) = gf2_kernels_simd::transpose::detect() {
///     let mut input = [0u64; 64];
///     input[0] = 0xFF; // first row has 8 set bits in cols 0..8
///     let mut output = [0u64; 64];
///     (fns.transpose_64x64)(&input, &mut output);
///     // After transpose, the first 8 output rows have bit 0 set.
///     for i in 0..8 {
///         assert_eq!(output[i] & 1, 1);
///     }
/// }
/// ```
pub fn detect() -> Option<TransposeFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return detect_x86();
    }
    #[allow(unreachable_code)]
    {
        // Always-available scalar lane on non-x86.
        Some(TransposeFns {
            transpose_64x64: transpose_64x64_scalar_safe,
            name: "scalar-bit-twiddle",
        })
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_x86() -> Option<TransposeFns> {
    use std::arch::is_x86_feature_detected;

    // V3: prefer the AVX2 YMM bit-twiddle lane.
    if is_x86_feature_detected!("avx2") {
        return Some(TransposeFns {
            transpose_64x64: transpose_64x64_avx2_safe,
            name: "avx2-bit-twiddle",
        });
    }

    // V4 fallback: scalar Hacker's Delight bit-twiddle (always available).
    Some(TransposeFns {
        transpose_64x64: transpose_64x64_scalar_safe,
        name: "scalar-bit-twiddle",
    })
}

/// Detect and return the AVX2 PSHUFB transpose lane when available.
///
/// This lane exists as the explicit B1 PSHUFB implementation and asm artefact
/// target. The main [`detect`] function may still choose a faster AVX2
/// bit-twiddle lane for production dispatch.
pub fn detect_pshufb() -> Option<Transpose64x64Fn> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        use std::arch::is_x86_feature_detected;

        if is_x86_feature_detected!("avx2") {
            return Some(transpose_64x64_avx2_pshufb_safe);
        }
    }
    #[allow(unreachable_code)]
    None
}

// ---------------------------------------------------------------------------
// Safe function-pointer wrappers
// ---------------------------------------------------------------------------

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn transpose_64x64_avx2_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    // SAFETY: `detect_x86` only returns this pointer when AVX2 is
    // available at runtime. Callers who bypass `detect` must uphold
    // that precondition.
    unsafe { crate::x86::transpose::transpose_64x64_avx2(input, output) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn transpose_64x64_avx2_pshufb_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    // SAFETY: `detect_pshufb` only returns this pointer when AVX2 is
    // available at runtime. Callers who bypass `detect_pshufb` must uphold
    // that precondition.
    unsafe { crate::x86::transpose::transpose_64x64_avx2_pshufb(input, output) }
}

fn transpose_64x64_scalar_safe(input: &[u64; 64], output: &mut [u64; 64]) {
    transpose_64x64_scalar(input, output)
}

// ---------------------------------------------------------------------------
// Public scalar reference (V4): Hacker's Delight ch. 7-3
// ---------------------------------------------------------------------------

/// In-place 64×64 bit-block transpose using the recursive
/// bit-interleave / mask-and-shift pattern (Hacker's Delight ch. 7-3).
///
/// `input[r]` is interpreted as the `r`-th row, with bit `c` carrying
/// the matrix entry `(r, c)`. After the call, `output[c]` is the
/// transposed row, with bit `r` carrying `(r, c)`.
///
/// This is the V4 register-tiled scalar reference: 6 stages of
/// mask-and-XOR-swap, halving the swap distance each stage. The
/// algorithm runs in O(N log N) bit operations on N×N tiles —
/// dramatically better than the O(N²) naive double-loop.
///
/// # Algorithm sketch
///
/// The 64×64 bit matrix is viewed as a recursive partition into 32×32
/// quadrants, then 16×16, etc. At each stage the kernel swaps the
/// off-diagonal sub-quadrants of each pair using a mask-shift-XOR
/// idiom that interchanges bit columns at distance `j` with bit rows
/// at distance `j` simultaneously across all pairs.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::transpose::transpose_64x64_scalar;
/// // Identity matrix maps to itself under transpose.
/// let mut input = [0u64; 64];
/// for i in 0..64 {
///     input[i] = 1u64 << i;
/// }
/// let mut output = [0u64; 64];
/// transpose_64x64_scalar(&input, &mut output);
/// assert_eq!(input, output);
/// ```
///
/// # Complexity
///
/// O(64 · log₂ 64) = O(384) word operations. Bench numbers under
/// `dev/benchmarks/ppc-baselines.json` entry `B1`.
pub fn transpose_64x64_scalar(input: &[u64; 64], output: &mut [u64; 64]) {
    // Copy input into a scratch buffer; we mutate it in place.
    let mut buf: [u64; 64] = *input;

    // Stage masks. Each mask is a 64-bit pattern selecting alternating
    // groups of 2^k columns. The swap distance is the same as the
    // group width.
    //
    //   k=5  width=32, mask = 0x00000000_FFFFFFFF
    //   k=4  width=16, mask = 0x0000FFFF_0000FFFF
    //   k=3  width=8,  mask = 0x00FF00FF_00FF00FF
    //   k=2  width=4,  mask = 0x0F0F0F0F_0F0F0F0F
    //   k=1  width=2,  mask = 0x33333333_33333333
    //   k=0  width=1,  mask = 0x55555555_55555555
    const MASKS: [u64; 6] = [
        0x0000_0000_FFFF_FFFF,
        0x0000_FFFF_0000_FFFF,
        0x00FF_00FF_00FF_00FF,
        0x0F0F_0F0F_0F0F_0F0F,
        0x3333_3333_3333_3333,
        0x5555_5555_5555_5555,
    ];

    // For each stage, walk pairs of rows separated by `1 << k` and
    // swap the lower / upper halves selected by the stage mask.
    let mut k = 5usize;
    loop {
        let j = 1usize << k;
        let m = MASKS[5 - k];
        // For each block of size 2j, swap rows [i..i+j) with rows
        // [i+j..i+2j) using the mask-shift-XOR idiom.
        let mut i = 0usize;
        while i < 64 {
            let mut r = i;
            while r < i + j {
                let a = buf[r];
                let b = buf[r + j];
                // t = ((a >> j) ^ b) & m
                // a' = a ^ (t << j)
                // b' = b ^ t
                let t = ((a >> j) ^ b) & m;
                buf[r] = a ^ (t << j);
                buf[r + j] = b ^ t;
                r += 1;
            }
            i += 2 * j;
        }
        if k == 0 {
            break;
        }
        k -= 1;
    }

    *output = buf;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpose_identity_matrix_is_idempotent() {
        // Identity matrix transposes to itself.
        let mut input = [0u64; 64];
        for (i, slot) in input.iter_mut().enumerate() {
            *slot = 1u64 << i;
        }
        let mut output = [0u64; 64];
        transpose_64x64_scalar(&input, &mut output);
        assert_eq!(input, output);
    }

    #[test]
    fn test_transpose_zero_matrix() {
        let input = [0u64; 64];
        let mut output = [!0u64; 64];
        transpose_64x64_scalar(&input, &mut output);
        assert_eq!(output, [0u64; 64]);
    }

    #[test]
    fn test_transpose_all_ones() {
        let input = [!0u64; 64];
        let mut output = [0u64; 64];
        transpose_64x64_scalar(&input, &mut output);
        assert_eq!(output, [!0u64; 64]);
    }

    #[test]
    fn test_transpose_single_bit_corners() {
        // Bit at (0, 0) → bit at (0, 0).
        let mut input = [0u64; 64];
        input[0] = 1;
        let mut output = [0u64; 64];
        transpose_64x64_scalar(&input, &mut output);
        let mut expected = [0u64; 64];
        expected[0] = 1;
        assert_eq!(output, expected);

        // Bit at (0, 63) → bit at (63, 0).
        let mut input = [0u64; 64];
        input[0] = 1u64 << 63;
        let mut output = [0u64; 64];
        transpose_64x64_scalar(&input, &mut output);
        let mut expected = [0u64; 64];
        expected[63] = 1;
        assert_eq!(output, expected);

        // Bit at (63, 0) → bit at (0, 63).
        let mut input = [0u64; 64];
        input[63] = 1;
        let mut output = [0u64; 64];
        transpose_64x64_scalar(&input, &mut output);
        let mut expected = [0u64; 64];
        expected[0] = 1u64 << 63;
        assert_eq!(output, expected);
    }

    #[test]
    fn test_transpose_double_is_identity() {
        // (A^T)^T = A for random patterns.
        let mut rng = 0x9E3779B97F4A7C15u64;
        let mut input = [0u64; 64];
        for slot in input.iter_mut() {
            // SplitMix64 PRNG for reproducibility without bringing in `rand`.
            rng = rng.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^= z >> 31;
            *slot = z;
        }
        let mut once = [0u64; 64];
        let mut twice = [0u64; 64];
        transpose_64x64_scalar(&input, &mut once);
        transpose_64x64_scalar(&once, &mut twice);
        assert_eq!(input, twice);
    }

    #[test]
    fn test_transpose_matches_naive() {
        // Build random 64x64 matrix, transpose with the kernel and the
        // naive bit-by-bit algorithm; assert they agree.
        let mut rng = 0xDEADBEEFCAFEBABEu64;
        let mut input = [0u64; 64];
        for slot in input.iter_mut() {
            rng = rng.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^= z >> 31;
            *slot = z;
        }
        let mut kernel_out = [0u64; 64];
        transpose_64x64_scalar(&input, &mut kernel_out);

        let mut naive_out = [0u64; 64];
        for (r, &row_bits) in input.iter().enumerate() {
            for (c, slot) in naive_out.iter_mut().enumerate() {
                let bit = (row_bits >> c) & 1;
                *slot |= bit << r;
            }
        }
        assert_eq!(kernel_out, naive_out);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_dispatched_kernel_matches_scalar() {
        // Ensure the dispatched lane (whichever one detect chose) agrees
        // with the scalar reference on a random sample of inputs.
        let Some(fns) = detect() else {
            return;
        };
        let mut rng = 0xFEEDFACEBADC0FFEu64;
        for _ in 0..32 {
            let mut input = [0u64; 64];
            for slot in input.iter_mut() {
                rng = rng.wrapping_add(0x9E3779B97F4A7C15);
                let mut z = rng;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
                z ^= z >> 31;
                *slot = z;
            }
            let mut a = [0u64; 64];
            let mut b = [0u64; 64];
            transpose_64x64_scalar(&input, &mut a);
            (fns.transpose_64x64)(&input, &mut b);
            assert_eq!(a, b, "dispatched lane '{}' diverged from scalar", fns.name);
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_pshufb_kernel_matches_scalar() {
        let Some(pshufb) = detect_pshufb() else {
            return;
        };
        let mut rng = 0x1234_5678_9ABC_DEF0u64;
        for _ in 0..32 {
            let mut input = [0u64; 64];
            for slot in input.iter_mut() {
                rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = rng;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 31;
                *slot = z;
            }
            let mut expected = [0u64; 64];
            let mut actual = [0u64; 64];
            transpose_64x64_scalar(&input, &mut expected);
            pshufb(&input, &mut actual);
            assert_eq!(expected, actual, "PSHUFB lane diverged from scalar");
        }
    }
}
