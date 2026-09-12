//! F_7 SIMD batch kernels (R2 Candidate A 3-bit + 2^16 LUT).
//!
//! The F_7 encoding follows R2 Candidate A (`dev/plans/f10152f6/r2_f7_encoding_decision.md`):
//! each `u64` packs **16 elements** at 4-bit-aligned slots. Slot `i` occupies
//! bits `[4i .. 4i+4)`. Canonical values are `0..=6`; the high bit of each
//! slot is always zero for canonical packings (since 6 = `0b0110 < 8`).
//!
//! Binary ops use a 64 KiB lookup table keyed by a packed 16-bit byte-pair
//! `(a_byte | (b_byte << 8))`. Three static LUTs (add, sub, mul) are built
//! at compile time in `gf2-algebra`; this module re-uses the same byte-pair
//! design with scalar LUT lookups per 8-byte chunk inside the SIMD wrapper.
//!
//! # SIMD strategy
//!
//! AVX2 batch entry points live in [`crate::x86::bipedal_avx2_packed7`]; they
//! batch byte-pair LUT lookups per u64 within each AVX2 register. A gather-based
//! approach could improve throughput but is deferred; the scalar-LUT-inside-SIMD
//! path already benefits from register-widened loop overhead reduction.
//!
//! # No `BipedalLikeConfig` integration
//!
//! The generic [`super::framework::BipedalLikeConfig`] trait imposes a 2-stream
//! `(mag, sgn)` shape per operand. The F_7 LUT encoding fits in 1 plane per
//! operand, so a `Config7: BipedalLikeConfig` impl that zeroed the unused `sgn`
//! stream would be technically faithful but dead code, since the production F_7
//! path already uses the dedicated single-plane LUT batch entry points. F_7
//! therefore ships *only* via [`crate::x86::bipedal_avx2_packed7`], the
//! runtime-detection bundle [`F7AvxFns`], and the scalar fallbacks below.
//! See JIT issue `1f769232`'s `## Amendment 2026-05-14` for the rationale.
//!
//! # Runtime detection
//!
//! [`has_avx2_f7`] caches the CPUID result in a `OnceLock<bool>`. [`detect_avx2_f7`]
//! returns a [`F7AvxFns`] bundle when the host supports AVX2.

// ---------------------------------------------------------------------------
// Compile-time LUT construction (mirrored from gf2-algebra::packed::packed7)
// ---------------------------------------------------------------------------
//
// These LUTs are identical in construction to the ones in gf2-algebra but are
// defined here so this crate has zero runtime dependency on gf2-algebra in
// production (gf2-algebra is a dev-dep only). The test module imports
// gf2-algebra types for proptest cross-checks.

/// Build the `ADD7_LUT` at compile time.
///
/// `ADD7_LUT[key]` where `key = (a_byte as usize) | ((b_byte as usize) << 8)`.
/// Low nibble of result = `(a_lo + b_lo) % 7`; high nibble = `(a_hi + b_hi) % 7`.
/// Non-canonical nibbles (≥ 7) produce 0.
const fn build_add7_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut ap: usize = 0;
    while ap < 256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        let mut bp: usize = 0;
        while bp < 256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                let r0 = (a0 + b0) % 7;
                let r1 = (a1 + b1) % 7;
                let key = (bp << 8) | ap;
                lut[key] = r0 | (r1 << 4);
            }
            bp += 1;
        }
        ap += 1;
    }
    lut
}

/// Build the `SUB7_LUT` at compile time.
///
/// `SUB7_LUT[key]` where `key = (a_byte as usize) | ((b_byte as usize) << 8)`.
/// Low nibble of result = `(a_lo - b_lo + 7) % 7`; high nibble = `(a_hi - b_hi + 7) % 7`.
/// Non-canonical nibbles (≥ 7) produce 0.
const fn build_sub7_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut ap: usize = 0;
    while ap < 256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        let mut bp: usize = 0;
        while bp < 256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                let r0 = (a0 + 7 - b0) % 7;
                let r1 = (a1 + 7 - b1) % 7;
                let key = (bp << 8) | ap;
                lut[key] = r0 | (r1 << 4);
            }
            bp += 1;
        }
        ap += 1;
    }
    lut
}

/// Build the `MUL7_LUT` at compile time.
///
/// `MUL7_LUT[key]` where `key = (a_byte as usize) | ((b_byte as usize) << 8)`.
/// Low nibble of result = `(a_lo * b_lo) % 7`; high nibble = `(a_hi * b_hi) % 7`.
/// Non-canonical nibbles (≥ 7) produce 0.
const fn build_mul7_lut() -> [u8; 65536] {
    let mut lut = [0u8; 65536];
    let mut ap: usize = 0;
    while ap < 256 {
        let a0 = (ap & 0xf) as u8;
        let a1 = (ap >> 4) as u8;
        let mut bp: usize = 0;
        while bp < 256 {
            let b0 = (bp & 0xf) as u8;
            let b1 = (bp >> 4) as u8;
            if a0 < 7 && a1 < 7 && b0 < 7 && b1 < 7 {
                let r0 = (a0 * b0) % 7;
                let r1 = (a1 * b1) % 7;
                let key = (bp << 8) | ap;
                lut[key] = r0 | (r1 << 4);
            }
            bp += 1;
        }
        ap += 1;
    }
    lut
}

/// F_7 addition LUT: 64 KiB, built at compile time.
///
/// Same construction as `gf2_algebra::packed::packed7::ADD_LUT`.
pub(crate) static ADD7_LUT: [u8; 65536] = build_add7_lut();

/// F_7 subtraction LUT: 64 KiB, built at compile time.
///
/// Same construction as `gf2_algebra::packed::packed7::SUB_LUT`.
pub(crate) static SUB7_LUT: [u8; 65536] = build_sub7_lut();

/// F_7 multiplication LUT: 64 KiB, built at compile time.
///
/// Same construction as `gf2_algebra::packed::packed7::MUL_LUT`.
pub(crate) static MUL7_LUT: [u8; 65536] = build_mul7_lut();

// ---------------------------------------------------------------------------
// Scalar word-level helpers
// ---------------------------------------------------------------------------

/// Apply a binary LUT op to a single pair of packed-F_7 `u64` words.
///
/// `lut[a_byte | (b_byte << 8)]` returns a byte whose low nibble is the
/// result for the low element pair and high nibble for the high element pair.
/// 8 lookups per `u64` (one per byte pair).
///
/// # Arguments
///
/// * `a` — first packed word (16 F_7 elements at 4-bit slots 0..=15).
/// * `b` — second packed word.
/// * `lut` — one of `ADD7_LUT`, `SUB7_LUT`, or `MUL7_LUT`.
///
/// # Complexity
///
/// `O(1)`: 8 LUT lookups + 8 shift/mask/OR ops.
#[inline(always)]
pub(crate) fn binary7_op_word(a: u64, b: u64, lut: &[u8; 65536]) -> u64 {
    let mut r: u64 = 0;
    let mut i = 0usize;
    while i < 8 {
        let ap = ((a >> (8 * i)) & 0xff) as usize;
        let bp = ((b >> (8 * i)) & 0xff) as usize;
        let key = ap | (bp << 8);
        r |= (lut[key] as u64) << (8 * i);
        i += 1;
    }
    r
}

/// Scalar F_7 neg on a single packed `u64` word.
///
/// Negation = `0 - a` via `SUB7_LUT`.
///
/// # Complexity
///
/// `O(1)`: 8 LUT lookups.
#[inline(always)]
pub(crate) fn neg7_word(a: u64) -> u64 {
    binary7_op_word(0u64, a, &SUB7_LUT)
}

// ---------------------------------------------------------------------------
// Scalar batch entry points (used as scalar fallback)
// ---------------------------------------------------------------------------

/// Scalar fallback batch add for F_7.
///
/// Processes `n_words` packed F_7 words, applying `binary7_op_word` with `ADD7_LUT`
/// per word. All three slices must have length `n_words`.
///
/// # Complexity
///
/// `O(n_words)`.
pub fn scalar_add7_batch(a: &[u64], b: &[u64], out: &mut [u64]) {
    let n = a.len();
    debug_assert_eq!(n, b.len());
    debug_assert_eq!(n, out.len());
    for i in 0..n {
        out[i] = binary7_op_word(a[i], b[i], &ADD7_LUT);
    }
}

/// Scalar fallback batch sub for F_7.
///
/// # Arguments / Complexity
///
/// Same contract as [`scalar_add7_batch`].
pub fn scalar_sub7_batch(a: &[u64], b: &[u64], out: &mut [u64]) {
    let n = a.len();
    debug_assert_eq!(n, b.len());
    debug_assert_eq!(n, out.len());
    for i in 0..n {
        out[i] = binary7_op_word(a[i], b[i], &SUB7_LUT);
    }
}

/// Scalar fallback batch mul for F_7.
///
/// # Arguments / Complexity
///
/// Same contract as [`scalar_add7_batch`].
pub fn scalar_mul7_batch(a: &[u64], b: &[u64], out: &mut [u64]) {
    let n = a.len();
    debug_assert_eq!(n, b.len());
    debug_assert_eq!(n, out.len());
    for i in 0..n {
        out[i] = binary7_op_word(a[i], b[i], &MUL7_LUT);
    }
}

/// Scalar fallback batch neg for F_7.
///
/// # Arguments
///
/// * `a` — input packed word slice.
/// * `out` — output packed word slice.
///
/// # Complexity
///
/// `O(a.len())`.
pub fn scalar_neg7_batch(a: &[u64], out: &mut [u64]) {
    let n = a.len();
    debug_assert_eq!(n, out.len());
    for i in 0..n {
        out[i] = neg7_word(a[i]);
    }
}

// ---------------------------------------------------------------------------
// Runtime AVX2 detection for F_7
// ---------------------------------------------------------------------------

/// Returns `true` when the CPU supports AVX2, `false` otherwise.
///
/// Caches the CPUID result in a `OnceLock<bool>`, same pattern as
/// [`super::bipedal3::has_avx2`].
///
/// # Complexity
///
/// `O(1)` after the first call.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn has_avx2_f7() -> bool {
    use std::sync::OnceLock;
    static AVX2: OnceLock<bool> = OnceLock::new();
    *AVX2.get_or_init(|| {
        use std::arch::is_x86_feature_detected;
        is_x86_feature_detected!("avx2")
    })
}

/// Binary batch kernel for F_7: `a_words op b_words -> out_words`.
///
/// All three slices must have the same length (a multiple of 4 for AVX2).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type F7BinaryKernelFn = fn(&[u64], &[u64], &mut [u64]);

/// Unary batch kernel for F_7: `a_words -> out_words`.
///
/// Both slices must have the same length.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type F7UnaryKernelFn = fn(&[u64], &mut [u64]);

/// Function-pointer bundle for the F_7 AVX2 batch kernels.
///
/// Returned by [`detect_avx2_f7`] when the host supports AVX2. The
/// function-pointer fields are safe to call — AVX2 availability was verified
/// during detection.
///
/// All binary ops take two input word slices and one output slice; all must
/// share the same length (multiple of 4 for the AVX2 path). `neg` takes one
/// input and one output.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::bipedal::packed7::detect_avx2_f7;
/// let maybe_fns = detect_avx2_f7();
/// // `maybe_fns.is_some()` on any AVX2-capable x86_64 host.
/// let _ = maybe_fns;
/// ```
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[derive(Copy, Clone)]
pub struct F7AvxFns {
    /// Apply F_7 add: `a + b -> out` element-wise mod 7.
    pub add_fn: F7BinaryKernelFn,
    /// Apply F_7 sub: `a - b -> out` element-wise mod 7.
    pub sub_fn: F7BinaryKernelFn,
    /// Apply F_7 mul: `a * b -> out` element-wise mod 7.
    pub mul_fn: F7BinaryKernelFn,
    /// Apply F_7 neg: `0 - a -> out` element-wise mod 7.
    pub neg_fn: F7UnaryKernelFn,
}

/// Detect AVX2 at runtime and return a [`F7AvxFns`] bundle if available.
///
/// Returns `None` on non-x86 targets or when the runtime CPU lacks AVX2.
/// Callers must then fall back to the scalar batch functions.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::bipedal::packed7::detect_avx2_f7;
/// let maybe_fns = detect_avx2_f7();
/// let _ = maybe_fns;
/// ```
///
/// # Complexity
///
/// `O(1)`; CPUID is cached via `OnceLock`.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn detect_avx2_f7() -> Option<F7AvxFns> {
    use std::sync::OnceLock;
    static FNS: OnceLock<Option<F7AvxFns>> = OnceLock::new();
    *FNS.get_or_init(detect_avx2_f7_uncached)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_avx2_f7_uncached() -> Option<F7AvxFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(F7AvxFns {
            add_fn: add7_safe,
            sub_fn: sub7_safe,
            mul_fn: mul7_safe,
            neg_fn: neg7_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn add7_safe(a: &[u64], b: &[u64], out: &mut [u64]) {
    // SAFETY: `detect_avx2_f7` only sets this fn ptr when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2_packed7::run_add7_batch(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn sub7_safe(a: &[u64], b: &[u64], out: &mut [u64]) {
    // SAFETY: `detect_avx2_f7` only sets this fn ptr when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2_packed7::run_sub7_batch(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn mul7_safe(a: &[u64], b: &[u64], out: &mut [u64]) {
    // SAFETY: `detect_avx2_f7` only sets this fn ptr when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2_packed7::run_mul7_batch(a, b, out) }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn neg7_safe(a: &[u64], out: &mut [u64]) {
    // SAFETY: `detect_avx2_f7` only sets this fn ptr when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2_packed7::run_neg7_batch(a, out) }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ------ Scalar helpers ------

    fn scalar_f7_add(a: u64, b: u64) -> u64 {
        (a + b) % 7
    }
    fn scalar_f7_sub(a: u64, b: u64) -> u64 {
        (a + 7 - b) % 7
    }
    fn scalar_f7_mul(a: u64, b: u64) -> u64 {
        (a * b) % 7
    }
    fn scalar_f7_neg(a: u64) -> u64 {
        (7 - a) % 7
    }

    /// Decode lane `i` (0..=15) from a packed F_7 `u64` word.
    fn decode_f7_lane(w: u64, i: usize) -> u64 {
        (w >> (4 * i)) & 0xf
    }

    /// Splat a single F_7 value across all 16 lanes of a packed u64 word.
    fn splat_f7_word(v: u64) -> u64 {
        v.wrapping_mul(0x1111_1111_1111_1111u64)
    }

    // ------ LUT spot-check ------

    #[test]
    fn test_add7_lut_spot_check() {
        // (3 + 4) % 7 = 0
        let a_byte: usize = 3; // low nibble=3, high nibble=0
        let b_byte: usize = 4;
        let key = a_byte | (b_byte << 8);
        assert_eq!(ADD7_LUT[key] & 0xf, 0, "(3+4)%7=0");
        // (6 + 6) % 7 = 5
        let a2: usize = 6;
        let b2: usize = 6;
        let key2 = a2 | (b2 << 8);
        assert_eq!(ADD7_LUT[key2] & 0xf, 5, "(6+6)%7=5");
    }

    #[test]
    fn test_sub7_lut_spot_check() {
        // (2 - 5 + 7) % 7 = 4
        let a_byte: usize = 2;
        let b_byte: usize = 5;
        let key = a_byte | (b_byte << 8);
        assert_eq!(SUB7_LUT[key] & 0xf, 4, "(2-5+7)%7=4");
    }

    #[test]
    fn test_mul7_lut_spot_check() {
        // (3 * 4) % 7 = 5
        let a_byte: usize = 3;
        let b_byte: usize = 4;
        let key = a_byte | (b_byte << 8);
        assert_eq!(MUL7_LUT[key] & 0xf, 5, "(3*4)%7=5");
    }

    // ------ Truth-table smoke tests ------

    #[test]
    fn test_binary7_op_word_truth_table() {
        for a in 0u64..7 {
            for b in 0u64..7 {
                let aw = splat_f7_word(a);
                let bw = splat_f7_word(b);
                let r_add = binary7_op_word(aw, bw, &ADD7_LUT);
                let r_sub = binary7_op_word(aw, bw, &SUB7_LUT);
                let r_mul = binary7_op_word(aw, bw, &MUL7_LUT);
                for lane in 0..16 {
                    assert_eq!(
                        decode_f7_lane(r_add, lane),
                        scalar_f7_add(a, b),
                        "add({a},{b}) lane {lane}"
                    );
                    assert_eq!(
                        decode_f7_lane(r_sub, lane),
                        scalar_f7_sub(a, b),
                        "sub({a},{b}) lane {lane}"
                    );
                    assert_eq!(
                        decode_f7_lane(r_mul, lane),
                        scalar_f7_mul(a, b),
                        "mul({a},{b}) lane {lane}"
                    );
                }
                let r_neg = neg7_word(aw);
                for lane in 0..16 {
                    assert_eq!(
                        decode_f7_lane(r_neg, lane),
                        scalar_f7_neg(a),
                        "neg({a}) lane {lane}"
                    );
                }
            }
        }
    }

    // ------ LCG word generator ------

    /// Build n packed F_7 words using a deterministic LCG (canonical values only).
    fn make_f7_words(n_words: usize, seed: u64) -> Vec<u64> {
        let mut words = Vec::with_capacity(n_words);
        let mut state = seed;
        for _ in 0..n_words {
            let mut w = 0u64;
            let mut lanes_left = 16usize;
            while lanes_left > 0 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let mut x = state;
                while lanes_left > 0 {
                    let r = x & 0xf;
                    x >>= 4;
                    if r < 7 {
                        let slot = 16 - lanes_left;
                        w |= r << (4 * slot);
                        lanes_left -= 1;
                    }
                    if x == 0 {
                        break;
                    }
                }
            }
            words.push(w);
        }
        words
    }

    // ------ Word-boundary tests for scalar batch ops ------

    fn check_scalar7_add(n_words: usize) {
        let a = make_f7_words(n_words, 0x1111_DEAD_BEEF);
        let b = make_f7_words(n_words, 0x2222_CAFE_F00D);
        let mut out = vec![0u64; n_words];
        scalar_add7_batch(&a, &b, &mut out);
        for wi in 0..n_words {
            for li in 0..16 {
                let av = decode_f7_lane(a[wi], li);
                let bv = decode_f7_lane(b[wi], li);
                assert_eq!(
                    decode_f7_lane(out[wi], li),
                    scalar_f7_add(av, bv),
                    "scalar add7 word={wi} lane={li}"
                );
            }
        }
    }

    fn check_scalar7_neg(n_words: usize) {
        let a = make_f7_words(n_words, 0xABCD_EF01_2345);
        let mut out = vec![0u64; n_words];
        scalar_neg7_batch(&a, &mut out);
        for wi in 0..n_words {
            for li in 0..16 {
                let av = decode_f7_lane(a[wi], li);
                assert_eq!(
                    decode_f7_lane(out[wi], li),
                    scalar_f7_neg(av),
                    "scalar neg7 word={wi} lane={li}"
                );
            }
        }
    }

    #[test]
    fn test_scalar7_batch_word_boundaries() {
        for &n in &[0usize, 1, 4, 63, 64, 65] {
            check_scalar7_add(n);
            check_scalar7_neg(n);
        }
    }

    // ------ AVX2 parity tests ------

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    mod avx2_parity {
        use super::*;
        use crate::x86::bipedal_avx2_packed7 as avx2;

        fn check_avx2_add7(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let a = make_f7_words(n_words, 0x1111_2222_3333);
            let b = make_f7_words(n_words, 0x4444_5555_6666);
            let mut avx_out = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths multiple of 4.
            unsafe {
                avx2::run_add7_batch(&a, &b, &mut avx_out);
            }
            let mut sc_out = vec![0u64; n_words];
            scalar_add7_batch(&a, &b, &mut sc_out);
            assert_eq!(
                avx_out.as_slice(),
                sc_out.as_slice(),
                "AVX2 add7 mismatch (n_words={n_words})"
            );
        }

        fn check_avx2_sub7(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let a = make_f7_words(n_words, 0xAAAA_BBBB_CCCC);
            let b = make_f7_words(n_words, 0xDDDD_EEEE_FFFF);
            let mut avx_out = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths multiple of 4.
            unsafe {
                avx2::run_sub7_batch(&a, &b, &mut avx_out);
            }
            let mut sc_out = vec![0u64; n_words];
            scalar_sub7_batch(&a, &b, &mut sc_out);
            assert_eq!(avx_out.as_slice(), sc_out.as_slice(), "AVX2 sub7 mismatch");
        }

        fn check_avx2_mul7(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let a = make_f7_words(n_words, 0x1234_5678_9ABC);
            let b = make_f7_words(n_words, 0xDEF0_1234_5678);
            let mut avx_out = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths multiple of 4.
            unsafe {
                avx2::run_mul7_batch(&a, &b, &mut avx_out);
            }
            let mut sc_out = vec![0u64; n_words];
            scalar_mul7_batch(&a, &b, &mut sc_out);
            assert_eq!(avx_out.as_slice(), sc_out.as_slice(), "AVX2 mul7 mismatch");
        }

        fn check_avx2_neg7(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let a = make_f7_words(n_words, 0xFEDC_BA98_7654);
            let mut avx_out = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths multiple of 4.
            unsafe {
                avx2::run_neg7_batch(&a, &mut avx_out);
            }
            let mut sc_out = vec![0u64; n_words];
            scalar_neg7_batch(&a, &mut sc_out);
            assert_eq!(avx_out.as_slice(), sc_out.as_slice(), "AVX2 neg7 mismatch");
        }

        // Word-boundary tests at n_words = {0, 4, 16, 64}.

        #[test]
        fn test_avx2_add7_matches_scalar_l0() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_add7(0);
        }

        #[test]
        fn test_avx2_sub7_matches_scalar_l0() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_sub7(0);
        }

        #[test]
        fn test_avx2_mul7_matches_scalar_l0() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_mul7(0);
        }

        #[test]
        fn test_avx2_neg7_matches_scalar_l0() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_neg7(0);
        }

        #[test]
        fn test_avx2_add7_matches_scalar_l4() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_add7(4);
        }

        #[test]
        fn test_avx2_sub7_matches_scalar_l4() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_sub7(4);
        }

        #[test]
        fn test_avx2_mul7_matches_scalar_l4() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_mul7(4);
        }

        #[test]
        fn test_avx2_neg7_matches_scalar_l4() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_neg7(4);
        }

        #[test]
        fn test_avx2_add7_matches_scalar_l16() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_add7(16);
        }

        #[test]
        fn test_avx2_sub7_matches_scalar_l16() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_sub7(16);
        }

        #[test]
        fn test_avx2_mul7_matches_scalar_l16() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_mul7(16);
        }

        #[test]
        fn test_avx2_neg7_matches_scalar_l16() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_neg7(16);
        }

        #[test]
        fn test_avx2_add7_matches_scalar_l64() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_add7(64);
        }

        #[test]
        fn test_avx2_sub7_matches_scalar_l64() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_sub7(64);
        }

        #[test]
        fn test_avx2_mul7_matches_scalar_l64() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_mul7(64);
        }

        #[test]
        fn test_avx2_neg7_matches_scalar_l64() {
            if !has_avx2_f7() {
                return;
            }
            check_avx2_neg7(64);
        }

        // -- Proptest cross-checks (1000 cases per op) vs scalar --

        fn f7_word_batch_strategy() -> impl Strategy<Value = (usize, u64, u64)> {
            (
                prop_oneof![Just(0usize), Just(4), Just(8), Just(16)],
                any::<u64>(),
                any::<u64>(),
            )
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1000))]

            /// Cross-check AVX2 add7 vs scalar on 1000 random packed F_7 word batches.
            #[test]
            fn test_avx2_add7_proptest((n_words, seed_a, seed_b) in f7_word_batch_strategy()) {
                if !has_avx2_f7() {
                    return Ok(());
                }
                let a = make_f7_words(n_words, seed_a);
                let b = make_f7_words(n_words, seed_b);
                let mut avx_out = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe { crate::x86::bipedal_avx2_packed7::run_add7_batch(&a, &b, &mut avx_out); }
                let mut sc_out = vec![0u64; n_words];
                scalar_add7_batch(&a, &b, &mut sc_out);
                prop_assert_eq!(avx_out, sc_out, "AVX2 add7 proptest");
            }

            /// Cross-check AVX2 sub7 vs scalar on 1000 random packed F_7 word batches.
            #[test]
            fn test_avx2_sub7_proptest((n_words, seed_a, seed_b) in f7_word_batch_strategy()) {
                if !has_avx2_f7() {
                    return Ok(());
                }
                let a = make_f7_words(n_words, seed_a);
                let b = make_f7_words(n_words, seed_b);
                let mut avx_out = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe { crate::x86::bipedal_avx2_packed7::run_sub7_batch(&a, &b, &mut avx_out); }
                let mut sc_out = vec![0u64; n_words];
                scalar_sub7_batch(&a, &b, &mut sc_out);
                prop_assert_eq!(avx_out, sc_out, "AVX2 sub7 proptest");
            }

            /// Cross-check AVX2 mul7 vs scalar on 1000 random packed F_7 word batches.
            #[test]
            fn test_avx2_mul7_proptest((n_words, seed_a, seed_b) in f7_word_batch_strategy()) {
                if !has_avx2_f7() {
                    return Ok(());
                }
                let a = make_f7_words(n_words, seed_a);
                let b = make_f7_words(n_words, seed_b);
                let mut avx_out = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe { crate::x86::bipedal_avx2_packed7::run_mul7_batch(&a, &b, &mut avx_out); }
                let mut sc_out = vec![0u64; n_words];
                scalar_mul7_batch(&a, &b, &mut sc_out);
                prop_assert_eq!(avx_out, sc_out, "AVX2 mul7 proptest");
            }

            /// Cross-check AVX2 neg7 vs scalar on 1000 random packed F_7 word batches.
            #[test]
            fn test_avx2_neg7_proptest((n_words, seed_a, _seed_b) in f7_word_batch_strategy()) {
                if !has_avx2_f7() {
                    return Ok(());
                }
                let a = make_f7_words(n_words, seed_a);
                let mut avx_out = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe { crate::x86::bipedal_avx2_packed7::run_neg7_batch(&a, &mut avx_out); }
                let mut sc_out = vec![0u64; n_words];
                scalar_neg7_batch(&a, &mut sc_out);
                prop_assert_eq!(avx_out, sc_out, "AVX2 neg7 proptest");
            }
        }

        // -- Scalar-fallback test (runs even on non-AVX2 hosts) --

        /// Verify scalar batch is consistent with word-level ops on any host.
        #[test]
        fn test_scalar7_batch_matches_word_ops_on_any_host() {
            let n = 8;
            let a = make_f7_words(n, 0xFACE_CAFE_BABE);
            let b = make_f7_words(n, 0xBEEF_DEAD_C0DE);
            let mut out = vec![0u64; n];
            scalar_add7_batch(&a, &b, &mut out);
            for i in 0..n {
                assert_eq!(
                    out[i],
                    binary7_op_word(a[i], b[i], &ADD7_LUT),
                    "scalar batch add word={i}"
                );
            }
        }
    }

    // -- Proptest cross-check vs gf2-algebra Packed7 scalar reference --

    mod packed7_cross {
        use super::*;
        use gf2_algebra::packed::{Packed7, PackedField};
        use gf2_core::gfp::Fp;

        fn packed7_from_word(w: u64) -> Packed7 {
            let mut p = Packed7::zero();
            for i in 0..16 {
                let v = (w >> (4 * i)) & 0xf;
                p = p.with_lane(i, Fp::<7>::new(v));
            }
            p
        }

        fn word_from_packed7(p: Packed7) -> u64 {
            let mut w = 0u64;
            for i in 0..16 {
                w |= p.lane(i).value() << (4 * i);
            }
            w
        }

        fn f7_word_strategy() -> impl Strategy<Value = u64> {
            prop::collection::vec(0u64..7u64, 16).prop_map(|vals| {
                let mut w = 0u64;
                for (i, &v) in vals.iter().enumerate() {
                    w |= v << (4 * i);
                }
                w
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1000))]

            /// Cross-check binary7_op_word add vs Packed7::add on 1000 inputs.
            #[test]
            fn test_add7_word_vs_packed7(a in f7_word_strategy(), b in f7_word_strategy()) {
                let pa = packed7_from_word(a);
                let pb = packed7_from_word(b);
                let expected = word_from_packed7(pa.add(pb));
                let got = binary7_op_word(a, b, &ADD7_LUT);
                prop_assert_eq!(got, expected, "add7 word mismatch");
            }

            /// Cross-check binary7_op_word sub vs Packed7::sub on 1000 inputs.
            #[test]
            fn test_sub7_word_vs_packed7(a in f7_word_strategy(), b in f7_word_strategy()) {
                let pa = packed7_from_word(a);
                let pb = packed7_from_word(b);
                let expected = word_from_packed7(pa.sub(pb));
                let got = binary7_op_word(a, b, &SUB7_LUT);
                prop_assert_eq!(got, expected, "sub7 word mismatch");
            }

            /// Cross-check binary7_op_word mul vs Packed7::mul on 1000 inputs.
            #[test]
            fn test_mul7_word_vs_packed7(a in f7_word_strategy(), b in f7_word_strategy()) {
                let pa = packed7_from_word(a);
                let pb = packed7_from_word(b);
                let expected = word_from_packed7(pa.mul(pb));
                let got = binary7_op_word(a, b, &MUL7_LUT);
                prop_assert_eq!(got, expected, "mul7 word mismatch");
            }

            /// Cross-check neg7_word vs Packed7::neg on 1000 inputs.
            #[test]
            fn test_neg7_word_vs_packed7(a in f7_word_strategy()) {
                let pa = packed7_from_word(a);
                let expected = word_from_packed7(pa.neg());
                let got = neg7_word(a);
                prop_assert_eq!(got, expected, "neg7 word mismatch");
            }
        }
    }
}
