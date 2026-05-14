//! F_5 SIMD batch kernels (R1 Candidate D 3-plane bit-sliced).
//!
//! The F_5 encoding follows R1 Candidate D (`dev/plans/r1_f5_encoding_decision.md`):
//! each `F_5` element is stored as a 3-bit canonical value in three parallel
//! bit-planes `(b0, b1, b2)`:
//!
//! | `x` | `b2` bit | `b1` bit | `b0` bit |
//! |-----|----------|----------|----------|
//! |  0  |    0     |    0     |    0     |
//! |  1  |    0     |    0     |    1     |
//! |  2  |    0     |    1     |    0     |
//! |  3  |    0     |    1     |    1     |
//! |  4  |    1     |    0     |    0     |
//!
//! One `u64` per plane = 64 F_5 lanes. One "word unit" = 3 u64s (one per plane).
//!
//! # SIMD strategy
//!
//! AVX2 vectorises over the 64-lane word units: one AVX2 register (256 bits =
//! 4 × u64) processes 4 independent 64-lane word units simultaneously, giving
//! 256 F_5 lanes per register. Binary ops require 3 registers per operand
//! (one per plane), so the batch entry points use a 6-input / 3-output stream
//! API. Custom AVX2 entry points live in [`crate::x86::bipedal_avx2_packed5`].
//!
//! # No `BipedalLikeConfig` integration
//!
//! The generic [`super::framework::BipedalLikeConfig`] trait imposes a 2-stream
//! `(mag, sgn)` shape per operand, which cannot losslessly represent F_5
//! value 4 (which needs `b2 = 1`). F_5 therefore ships *only* via the
//! dedicated 3-plane AVX2 entry points in [`crate::x86::bipedal_avx2_packed5`],
//! the runtime-detection bundle [`F5AvxFns`], and the scalar fallbacks below.
//! See JIT issue `1f769232`'s `## Amendment 2026-05-14` for the rationale.
//!
//! # Runtime detection
//!
//! [`has_avx2_f5`] caches the CPUID result in a `OnceLock<bool>` (same pattern
//! as [`super::bipedal3::has_avx2`]). [`detect_avx2_f5`] returns a
//! [`F5AvxFns`] bundle when the host supports AVX2.

// ---------------------------------------------------------------------------
// Scalar word-level helpers (shared with AVX2 entry points via re-export)
// ---------------------------------------------------------------------------

/// Decode a single `(b0, b1, b2)` operand word-triple into five mutually-
/// exclusive one-hot selectors `[e0, e1, e2, e3, e4]`.
///
/// A lane carries `e[i] = 1` iff that lane's canonical value equals `i`.
/// Codepoints 5..=7 produce all-zero selectors (treated as 0).
///
/// Transliterated from `gf2_algebra::packed::packed5::decode5`.
///
/// # Complexity
///
/// `O(1)`: 3 NOTs + 8 ANDs = 11 word-level operations.
#[inline(always)]
pub(crate) fn decode5_word(b0: u64, b1: u64, b2: u64) -> [u64; 5] {
    let n0 = !b0;
    let n1 = !b1;
    let n2 = !b2;
    let n2n1 = n2 & n1;
    let n2_1 = n2 & b1;
    let n1n0 = n1 & n0;
    let e0 = n2n1 & n0;
    let e1 = n2n1 & b0;
    let e2 = n2_1 & n0;
    let e3 = n2_1 & b0;
    let e4 = b2 & n1n0;
    [e0, e1, e2, e3, e4]
}

/// Encode five per-result selectors `r[0..5]` into output bit-planes
/// `(c0, c1, c2)`.
///
/// Encoding per the 3-bit canonical mapping:
/// - `c0 = r[1] | r[3]` (b0 bit set for values 1 and 3)
/// - `c1 = r[2] | r[3]` (b1 bit set for values 2 and 3)
/// - `c2 = r[4]`        (b2 bit set for value 4)
///
/// Transliterated from `gf2_algebra::packed::packed5::encode5`.
///
/// # Complexity
///
/// `O(1)`: 3 OR operations.
#[inline(always)]
pub(crate) fn encode5_word(r: [u64; 5]) -> (u64, u64, u64) {
    let c0 = r[1] | r[3];
    let c1 = r[2] | r[3];
    let c2 = r[4];
    (c0, c1, c2)
}

/// Scalar F_5 add on a single word-triple `(b0a, b1a, b2a) + (b0b, b1b, b2b)`.
///
/// Cross-product cells `(i + j) mod 5 == k` for k in 1..=4.
///
/// # Complexity
///
/// `O(1)`: 60 word-level bitwise operations.
#[inline(always)]
pub(crate) fn add5_word(
    b0a: u64,
    b1a: u64,
    b2a: u64,
    b0b: u64,
    b1b: u64,
    b2b: u64,
) -> (u64, u64, u64) {
    let ea = decode5_word(b0a, b1a, b2a);
    let eb = decode5_word(b0b, b1b, b2b);
    let r1 =
        (ea[0] & eb[1]) | (ea[1] & eb[0]) | (ea[2] & eb[4]) | (ea[3] & eb[3]) | (ea[4] & eb[2]);
    let r2 =
        (ea[0] & eb[2]) | (ea[1] & eb[1]) | (ea[2] & eb[0]) | (ea[3] & eb[4]) | (ea[4] & eb[3]);
    let r3 =
        (ea[0] & eb[3]) | (ea[1] & eb[2]) | (ea[2] & eb[1]) | (ea[3] & eb[0]) | (ea[4] & eb[4]);
    let r4 =
        (ea[0] & eb[4]) | (ea[1] & eb[3]) | (ea[2] & eb[2]) | (ea[3] & eb[1]) | (ea[4] & eb[0]);
    encode5_word([0, r1, r2, r3, r4])
}

/// Scalar F_5 sub on a single word-triple.
///
/// Cross-product cells `(i - j + 5) mod 5 == k` for k in 1..=4.
///
/// # Complexity
///
/// `O(1)`: 60 word-level bitwise operations.
#[inline(always)]
pub(crate) fn sub5_word(
    b0a: u64,
    b1a: u64,
    b2a: u64,
    b0b: u64,
    b1b: u64,
    b2b: u64,
) -> (u64, u64, u64) {
    let ea = decode5_word(b0a, b1a, b2a);
    let eb = decode5_word(b0b, b1b, b2b);
    let r1 =
        (ea[0] & eb[4]) | (ea[1] & eb[0]) | (ea[2] & eb[1]) | (ea[3] & eb[2]) | (ea[4] & eb[3]);
    let r2 =
        (ea[0] & eb[3]) | (ea[1] & eb[4]) | (ea[2] & eb[0]) | (ea[3] & eb[1]) | (ea[4] & eb[2]);
    let r3 =
        (ea[0] & eb[2]) | (ea[1] & eb[3]) | (ea[2] & eb[4]) | (ea[3] & eb[0]) | (ea[4] & eb[1]);
    let r4 =
        (ea[0] & eb[1]) | (ea[1] & eb[2]) | (ea[2] & eb[3]) | (ea[3] & eb[4]) | (ea[4] & eb[0]);
    encode5_word([0, r1, r2, r3, r4])
}

/// Scalar F_5 mul on a single word-triple.
///
/// Cross-product cells `(i * j) mod 5 == k` for k in 1..=4
/// (cells with i=0 or j=0 always yield 0).
///
/// # Complexity
///
/// `O(1)`: 52 word-level bitwise operations.
#[inline(always)]
pub(crate) fn mul5_word(
    b0a: u64,
    b1a: u64,
    b2a: u64,
    b0b: u64,
    b1b: u64,
    b2b: u64,
) -> (u64, u64, u64) {
    let ea = decode5_word(b0a, b1a, b2a);
    let eb = decode5_word(b0b, b1b, b2b);
    let r1 = (ea[1] & eb[1]) | (ea[2] & eb[3]) | (ea[3] & eb[2]) | (ea[4] & eb[4]);
    let r2 = (ea[1] & eb[2]) | (ea[2] & eb[1]) | (ea[3] & eb[4]) | (ea[4] & eb[3]);
    let r3 = (ea[1] & eb[3]) | (ea[2] & eb[4]) | (ea[3] & eb[1]) | (ea[4] & eb[2]);
    let r4 = (ea[1] & eb[4]) | (ea[2] & eb[2]) | (ea[3] & eb[3]) | (ea[4] & eb[1]);
    encode5_word([0, r1, r2, r3, r4])
}

/// Scalar F_5 neg on a single word-triple.
///
/// Negation remap: `neg(x) = (5 - x) mod 5`, i.e. selectors permuted as
/// `(e0, e1, e2, e3, e4) -> (e0, e4, e3, e2, e1)`.
///
/// # Complexity
///
/// `O(1)`: 11 decode + 2 encode = 13 word-level operations.
#[inline(always)]
pub(crate) fn neg5_word(b0: u64, b1: u64, b2: u64) -> (u64, u64, u64) {
    let e = decode5_word(b0, b1, b2);
    encode5_word([e[0], e[4], e[3], e[2], e[1]])
}

// ---------------------------------------------------------------------------
// Scalar batch entry points (used as scalar fallback)
// ---------------------------------------------------------------------------

/// Scalar fallback batch add for F_5.
///
/// Processes `n_words` word-triples (one per 64 F_5 lanes), applying
/// `add5_word` per triple. Caller ensures all slices have length `n_words`.
///
/// # Arguments
///
/// * `b0a, b1a, b2a` — first operand planes (each length `n_words`).
/// * `b0b, b1b, b2b` — second operand planes.
/// * `out_b0, out_b1, out_b2` — output planes.
///
/// # Panics
///
/// Panics in debug mode if slice lengths are inconsistent.
///
/// # Complexity
///
/// `O(n_words)`.
#[allow(clippy::too_many_arguments)]
pub fn scalar_add5_batch(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    let n = b0a.len();
    debug_assert_eq!(n, b1a.len());
    debug_assert_eq!(n, b2a.len());
    debug_assert_eq!(n, b0b.len());
    debug_assert_eq!(n, b1b.len());
    debug_assert_eq!(n, b2b.len());
    debug_assert_eq!(n, out_b0.len());
    debug_assert_eq!(n, out_b1.len());
    debug_assert_eq!(n, out_b2.len());
    for i in 0..n {
        let (c0, c1, c2) = add5_word(b0a[i], b1a[i], b2a[i], b0b[i], b1b[i], b2b[i]);
        out_b0[i] = c0;
        out_b1[i] = c1;
        out_b2[i] = c2;
    }
}

/// Scalar fallback batch sub for F_5.
///
/// # Arguments / Complexity
///
/// Same contract as [`scalar_add5_batch`].
#[allow(clippy::too_many_arguments)]
pub fn scalar_sub5_batch(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    let n = b0a.len();
    debug_assert_eq!(n, b1a.len());
    debug_assert_eq!(n, b2a.len());
    debug_assert_eq!(n, b0b.len());
    debug_assert_eq!(n, b1b.len());
    debug_assert_eq!(n, b2b.len());
    debug_assert_eq!(n, out_b0.len());
    debug_assert_eq!(n, out_b1.len());
    debug_assert_eq!(n, out_b2.len());
    for i in 0..n {
        let (c0, c1, c2) = sub5_word(b0a[i], b1a[i], b2a[i], b0b[i], b1b[i], b2b[i]);
        out_b0[i] = c0;
        out_b1[i] = c1;
        out_b2[i] = c2;
    }
}

/// Scalar fallback batch mul for F_5.
///
/// # Arguments / Complexity
///
/// Same contract as [`scalar_add5_batch`].
#[allow(clippy::too_many_arguments)]
pub fn scalar_mul5_batch(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    let n = b0a.len();
    debug_assert_eq!(n, b1a.len());
    debug_assert_eq!(n, b2a.len());
    debug_assert_eq!(n, b0b.len());
    debug_assert_eq!(n, b1b.len());
    debug_assert_eq!(n, b2b.len());
    debug_assert_eq!(n, out_b0.len());
    debug_assert_eq!(n, out_b1.len());
    debug_assert_eq!(n, out_b2.len());
    for i in 0..n {
        let (c0, c1, c2) = mul5_word(b0a[i], b1a[i], b2a[i], b0b[i], b1b[i], b2b[i]);
        out_b0[i] = c0;
        out_b1[i] = c1;
        out_b2[i] = c2;
    }
}

/// Scalar fallback batch neg for F_5.
///
/// # Arguments
///
/// * `b0, b1, b2` — input planes (each length `n_words`).
/// * `out_b0, out_b1, out_b2` — output planes.
///
/// # Complexity
///
/// `O(n_words)`.
pub fn scalar_neg5_batch(
    b0: &[u64],
    b1: &[u64],
    b2: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    let n = b0.len();
    debug_assert_eq!(n, b1.len());
    debug_assert_eq!(n, b2.len());
    debug_assert_eq!(n, out_b0.len());
    debug_assert_eq!(n, out_b1.len());
    debug_assert_eq!(n, out_b2.len());
    for i in 0..n {
        let (c0, c1, c2) = neg5_word(b0[i], b1[i], b2[i]);
        out_b0[i] = c0;
        out_b1[i] = c1;
        out_b2[i] = c2;
    }
}

// ---------------------------------------------------------------------------
// Runtime AVX2 detection for F_5
// ---------------------------------------------------------------------------

/// Returns `true` when the CPU supports AVX2, `false` otherwise.
///
/// The result is cached in a `OnceLock<bool>` so CPUID is queried at most
/// once per process — matching the [`super::bipedal3::has_avx2`] pattern.
///
/// # Complexity
///
/// `O(1)` after the first call (CPUID result is cached).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn has_avx2_f5() -> bool {
    use std::sync::OnceLock;
    static AVX2: OnceLock<bool> = OnceLock::new();
    *AVX2.get_or_init(|| {
        use std::arch::is_x86_feature_detected;
        is_x86_feature_detected!("avx2")
    })
}

/// Binary batch kernel for F_5: `(b0a,b1a,b2a) op (b0b,b1b,b2b) -> (out0,out1,out2)`.
///
/// All nine slices must have the same length (a multiple of 4 for the AVX2 path).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type F5BinaryKernelFn =
    fn(&[u64], &[u64], &[u64], &[u64], &[u64], &[u64], &mut [u64], &mut [u64], &mut [u64]);

/// Unary batch kernel for F_5: `(b0, b1, b2) -> (out0, out1, out2)`.
///
/// All six slices must have the same length.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type F5UnaryKernelFn = fn(&[u64], &[u64], &[u64], &mut [u64], &mut [u64], &mut [u64]);

/// Function-pointer bundle for the F_5 AVX2 batch kernels.
///
/// Returned by [`detect_avx2_f5`] when the host supports AVX2. The
/// function-pointer fields are safe to call — safety preconditions were
/// discharged by the detection step.
///
/// All binary ops take three input plane pairs and three output planes; all
/// slices must share the same length (a multiple of 4 for the AVX2 path).
/// `neg` takes one input triple and one output triple.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::bipedal::packed5::detect_avx2_f5;
/// let maybe_fns = detect_avx2_f5();
/// // `maybe_fns.is_some()` on any AVX2-capable x86_64 host.
/// let _ = maybe_fns;
/// ```
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[derive(Copy, Clone)]
pub struct F5AvxFns {
    /// Apply F_5 add: `(a_planes) + (b_planes) -> out_planes`.
    pub add_fn: F5BinaryKernelFn,
    /// Apply F_5 sub: `(a_planes) - (b_planes) -> out_planes`.
    pub sub_fn: F5BinaryKernelFn,
    /// Apply F_5 mul: `(a_planes) * (b_planes) -> out_planes`.
    pub mul_fn: F5BinaryKernelFn,
    /// Apply F_5 neg: `-(a_planes) -> out_planes`.
    pub neg_fn: F5UnaryKernelFn,
}

/// Detect AVX2 at runtime and return an [`F5AvxFns`] bundle if available.
///
/// Returns `None` on non-x86 targets or when the runtime CPU lacks AVX2.
/// Callers must then fall back to the scalar batch functions.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::bipedal::packed5::detect_avx2_f5;
/// let maybe_fns = detect_avx2_f5();
/// let _ = maybe_fns;
/// ```
///
/// # Complexity
///
/// `O(1)`; CPUID is cached via `OnceLock`.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn detect_avx2_f5() -> Option<F5AvxFns> {
    use std::sync::OnceLock;
    static FNS: OnceLock<Option<F5AvxFns>> = OnceLock::new();
    *FNS.get_or_init(detect_avx2_f5_uncached)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn detect_avx2_f5_uncached() -> Option<F5AvxFns> {
    use std::arch::is_x86_feature_detected;
    if is_x86_feature_detected!("avx2") {
        Some(F5AvxFns {
            add_fn: add5_safe,
            sub_fn: sub5_safe,
            mul_fn: mul5_safe,
            neg_fn: neg5_safe,
        })
    } else {
        None
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(clippy::too_many_arguments)]
fn add5_safe(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: `detect_avx2_f5` only sets this fn ptr when AVX2 is available.
    unsafe {
        crate::x86::bipedal_avx2_packed5::run_add5_batch(
            b0a, b1a, b2a, b0b, b1b, b2b, out_b0, out_b1, out_b2,
        )
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(clippy::too_many_arguments)]
fn sub5_safe(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: `detect_avx2_f5` only sets this fn ptr when AVX2 is available.
    unsafe {
        crate::x86::bipedal_avx2_packed5::run_sub5_batch(
            b0a, b1a, b2a, b0b, b1b, b2b, out_b0, out_b1, out_b2,
        )
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(clippy::too_many_arguments)]
fn mul5_safe(
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: `detect_avx2_f5` only sets this fn ptr when AVX2 is available.
    unsafe {
        crate::x86::bipedal_avx2_packed5::run_mul5_batch(
            b0a, b1a, b2a, b0b, b1b, b2b, out_b0, out_b1, out_b2,
        )
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn neg5_safe(
    b0: &[u64],
    b1: &[u64],
    b2: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    // SAFETY: `detect_avx2_f5` only sets this fn ptr when AVX2 is available.
    unsafe { crate::x86::bipedal_avx2_packed5::run_neg5_batch(b0, b1, b2, out_b0, out_b1, out_b2) }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ------ Encoding helpers ------

    /// Encode a canonical F_5 value into the 3-plane bit representation.
    fn encode_val(x: u8) -> (u64, u64, u64) {
        let v = x as u64;
        let b0 = v & 1;
        let b1 = (v >> 1) & 1;
        let b2 = (v >> 2) & 1;
        (b0, b1, b2)
    }

    /// Decode lane `i` from a `(b0, b1, b2)` word-triple.
    fn decode_lane(b0: u64, b1: u64, b2: u64, i: usize) -> u64 {
        let bit0 = (b0 >> i) & 1;
        let bit1 = (b1 >> i) & 1;
        let bit2 = (b2 >> i) & 1;
        bit0 | (bit1 << 1) | (bit2 << 2)
    }

    /// Build a packed word-triple where every lane holds the same value `v`.
    fn splat_word(v: u8) -> (u64, u64, u64) {
        let (b0_bit, b1_bit, b2_bit) = encode_val(v);
        let b0 = if b0_bit != 0 { u64::MAX } else { 0 };
        let b1 = if b1_bit != 0 { u64::MAX } else { 0 };
        let b2 = if b2_bit != 0 { u64::MAX } else { 0 };
        (b0, b1, b2)
    }

    // ------ Scalar reference ops ------

    fn scalar_f5_add(a: u64, b: u64) -> u64 {
        (a + b) % 5
    }
    fn scalar_f5_sub(a: u64, b: u64) -> u64 {
        (a + 5 - b) % 5
    }
    fn scalar_f5_mul(a: u64, b: u64) -> u64 {
        (a * b) % 5
    }
    fn scalar_f5_neg(a: u64) -> u64 {
        (5 - a) % 5
    }

    // ------ Truth-table smoke tests (5×5 grid per binary op) ------

    #[test]
    fn test_add5_word_truth_table() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let (b0a, b1a, b2a) = splat_word(a);
                let (b0b, b1b, b2b) = splat_word(b);
                let (c0, c1, c2) = add5_word(b0a, b1a, b2a, b0b, b1b, b2b);
                let expected = scalar_f5_add(a as u64, b as u64);
                for lane in 0..64 {
                    assert_eq!(
                        decode_lane(c0, c1, c2, lane),
                        expected,
                        "add5_word({a},{b}) lane {lane}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_sub5_word_truth_table() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let (b0a, b1a, b2a) = splat_word(a);
                let (b0b, b1b, b2b) = splat_word(b);
                let (c0, c1, c2) = sub5_word(b0a, b1a, b2a, b0b, b1b, b2b);
                let expected = scalar_f5_sub(a as u64, b as u64);
                for lane in 0..64 {
                    assert_eq!(
                        decode_lane(c0, c1, c2, lane),
                        expected,
                        "sub5_word({a},{b}) lane {lane}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_mul5_word_truth_table() {
        for a in 0u8..5 {
            for b in 0u8..5 {
                let (b0a, b1a, b2a) = splat_word(a);
                let (b0b, b1b, b2b) = splat_word(b);
                let (c0, c1, c2) = mul5_word(b0a, b1a, b2a, b0b, b1b, b2b);
                let expected = scalar_f5_mul(a as u64, b as u64);
                for lane in 0..64 {
                    assert_eq!(
                        decode_lane(c0, c1, c2, lane),
                        expected,
                        "mul5_word({a},{b}) lane {lane}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_neg5_word_truth_table() {
        for a in 0u8..5 {
            let (b0, b1, b2) = splat_word(a);
            let (c0, c1, c2) = neg5_word(b0, b1, b2);
            let expected = scalar_f5_neg(a as u64);
            for lane in 0..64 {
                assert_eq!(
                    decode_lane(c0, c1, c2, lane),
                    expected,
                    "neg5_word({a}) lane {lane}"
                );
            }
        }
    }

    // ------ Word-boundary tests for scalar batch ops ------

    /// Build n-word plane slices with deterministic LCG values (canonical only).
    fn make_f5_words(n_words: usize, seed: u64) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
        let mut b0 = vec![0u64; n_words];
        let mut b1 = vec![0u64; n_words];
        let mut b2 = vec![0u64; n_words];
        let mut state = seed;
        for i in 0..n_words {
            // Fill each word with 64 canonical F_5 lanes (0..=4) via LCG.
            let mut w0 = 0u64;
            let mut w1 = 0u64;
            let mut w2 = 0u64;
            let mut bits_left = 64usize;
            while bits_left > 0 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let mut x = state;
                while bits_left > 0 {
                    let r = x & 0x7;
                    x >>= 3;
                    if r < 5 {
                        let bit = 64 - bits_left;
                        if (r & 1) != 0 {
                            w0 |= 1u64 << bit;
                        }
                        if (r & 2) != 0 {
                            w1 |= 1u64 << bit;
                        }
                        if (r & 4) != 0 {
                            w2 |= 1u64 << bit;
                        }
                        bits_left -= 1;
                        if x == 0 {
                            break;
                        }
                    }
                }
            }
            b0[i] = w0;
            b1[i] = w1;
            b2[i] = w2;
        }
        (b0, b1, b2)
    }

    fn check_scalar_batch_add(n_words: usize) {
        let (b0a, b1a, b2a) = make_f5_words(n_words, 0xDEAD_BEEF_1234);
        let (b0b, b1b, b2b) = make_f5_words(n_words, 0xCAFE_F00D_5678);
        let mut out0 = vec![0u64; n_words];
        let mut out1 = vec![0u64; n_words];
        let mut out2 = vec![0u64; n_words];
        scalar_add5_batch(
            &b0a, &b1a, &b2a, &b0b, &b1b, &b2b, &mut out0, &mut out1, &mut out2,
        );
        for wi in 0..n_words {
            for li in 0..64 {
                let a = decode_lane(b0a[wi], b1a[wi], b2a[wi], li);
                let b = decode_lane(b0b[wi], b1b[wi], b2b[wi], li);
                let got = decode_lane(out0[wi], out1[wi], out2[wi], li);
                assert_eq!(got, scalar_f5_add(a, b), "scalar_add5 word={wi} lane={li}");
            }
        }
    }

    fn check_scalar_batch_neg(n_words: usize) {
        let (b0, b1, b2) = make_f5_words(n_words, 0xABCD_EF01_2345);
        let mut out0 = vec![0u64; n_words];
        let mut out1 = vec![0u64; n_words];
        let mut out2 = vec![0u64; n_words];
        scalar_neg5_batch(&b0, &b1, &b2, &mut out0, &mut out1, &mut out2);
        for wi in 0..n_words {
            for li in 0..64 {
                let a = decode_lane(b0[wi], b1[wi], b2[wi], li);
                let got = decode_lane(out0[wi], out1[wi], out2[wi], li);
                assert_eq!(got, scalar_f5_neg(a), "scalar_neg5 word={wi} lane={li}");
            }
        }
    }

    #[test]
    fn test_scalar5_batch_word_boundaries() {
        for n in [0usize, 1, 4, 64, 65] {
            check_scalar_batch_add(n);
            check_scalar_batch_neg(n);
        }
    }

    // ------ AVX2 parity tests ------

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    mod avx2_parity {
        use super::*;
        use crate::x86::bipedal_avx2_packed5 as avx2;

        fn check_avx2_add(n_words: usize) {
            // n_words must be multiple of 4 for AVX2 path
            assert_eq!(n_words % 4, 0);
            let (b0a, b1a, b2a) = make_f5_words(n_words, 0x1111_2222_3333);
            let (b0b, b1b, b2b) = make_f5_words(n_words, 0x4444_5555_6666);
            let mut avx_out0 = vec![0u64; n_words];
            let mut avx_out1 = vec![0u64; n_words];
            let mut avx_out2 = vec![0u64; n_words];
            // SAFETY: AVX2 verified by calling test; lengths are multiple of 4.
            unsafe {
                avx2::run_add5_batch(
                    &b0a,
                    &b1a,
                    &b2a,
                    &b0b,
                    &b1b,
                    &b2b,
                    &mut avx_out0,
                    &mut avx_out1,
                    &mut avx_out2,
                );
            }
            let mut sc_out0 = vec![0u64; n_words];
            let mut sc_out1 = vec![0u64; n_words];
            let mut sc_out2 = vec![0u64; n_words];
            scalar_add5_batch(
                &b0a,
                &b1a,
                &b2a,
                &b0b,
                &b1b,
                &b2b,
                &mut sc_out0,
                &mut sc_out1,
                &mut sc_out2,
            );
            assert_eq!(
                avx_out0.as_slice(),
                sc_out0.as_slice(),
                "AVX2 add5 b0 mismatch (n_words={n_words})"
            );
            assert_eq!(
                avx_out1.as_slice(),
                sc_out1.as_slice(),
                "AVX2 add5 b1 mismatch (n_words={n_words})"
            );
            assert_eq!(
                avx_out2.as_slice(),
                sc_out2.as_slice(),
                "AVX2 add5 b2 mismatch (n_words={n_words})"
            );
        }

        fn check_avx2_sub(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let (b0a, b1a, b2a) = make_f5_words(n_words, 0xAAAA_BBBB_CCCC);
            let (b0b, b1b, b2b) = make_f5_words(n_words, 0xDDDD_EEEE_FFFF);
            let mut avx_out0 = vec![0u64; n_words];
            let mut avx_out1 = vec![0u64; n_words];
            let mut avx_out2 = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths are multiple of 4.
            unsafe {
                avx2::run_sub5_batch(
                    &b0a,
                    &b1a,
                    &b2a,
                    &b0b,
                    &b1b,
                    &b2b,
                    &mut avx_out0,
                    &mut avx_out1,
                    &mut avx_out2,
                );
            }
            let mut sc_out0 = vec![0u64; n_words];
            let mut sc_out1 = vec![0u64; n_words];
            let mut sc_out2 = vec![0u64; n_words];
            scalar_sub5_batch(
                &b0a,
                &b1a,
                &b2a,
                &b0b,
                &b1b,
                &b2b,
                &mut sc_out0,
                &mut sc_out1,
                &mut sc_out2,
            );
            assert_eq!(avx_out0.as_slice(), sc_out0.as_slice(), "AVX2 sub5 b0");
            assert_eq!(avx_out1.as_slice(), sc_out1.as_slice(), "AVX2 sub5 b1");
            assert_eq!(avx_out2.as_slice(), sc_out2.as_slice(), "AVX2 sub5 b2");
        }

        fn check_avx2_mul(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let (b0a, b1a, b2a) = make_f5_words(n_words, 0x1234_5678_9ABC);
            let (b0b, b1b, b2b) = make_f5_words(n_words, 0xDEF0_1234_5678);
            let mut avx_out0 = vec![0u64; n_words];
            let mut avx_out1 = vec![0u64; n_words];
            let mut avx_out2 = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths are multiple of 4.
            unsafe {
                avx2::run_mul5_batch(
                    &b0a,
                    &b1a,
                    &b2a,
                    &b0b,
                    &b1b,
                    &b2b,
                    &mut avx_out0,
                    &mut avx_out1,
                    &mut avx_out2,
                );
            }
            let mut sc_out0 = vec![0u64; n_words];
            let mut sc_out1 = vec![0u64; n_words];
            let mut sc_out2 = vec![0u64; n_words];
            scalar_mul5_batch(
                &b0a,
                &b1a,
                &b2a,
                &b0b,
                &b1b,
                &b2b,
                &mut sc_out0,
                &mut sc_out1,
                &mut sc_out2,
            );
            assert_eq!(avx_out0.as_slice(), sc_out0.as_slice(), "AVX2 mul5 b0");
            assert_eq!(avx_out1.as_slice(), sc_out1.as_slice(), "AVX2 mul5 b1");
            assert_eq!(avx_out2.as_slice(), sc_out2.as_slice(), "AVX2 mul5 b2");
        }

        fn check_avx2_neg(n_words: usize) {
            assert_eq!(n_words % 4, 0);
            let (b0, b1, b2) = make_f5_words(n_words, 0xFEDC_BA98_7654);
            let mut avx_out0 = vec![0u64; n_words];
            let mut avx_out1 = vec![0u64; n_words];
            let mut avx_out2 = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths are multiple of 4.
            unsafe {
                avx2::run_neg5_batch(&b0, &b1, &b2, &mut avx_out0, &mut avx_out1, &mut avx_out2);
            }
            let mut sc_out0 = vec![0u64; n_words];
            let mut sc_out1 = vec![0u64; n_words];
            let mut sc_out2 = vec![0u64; n_words];
            scalar_neg5_batch(&b0, &b1, &b2, &mut sc_out0, &mut sc_out1, &mut sc_out2);
            assert_eq!(avx_out0.as_slice(), sc_out0.as_slice(), "AVX2 neg5 b0");
            assert_eq!(avx_out1.as_slice(), sc_out1.as_slice(), "AVX2 neg5 b1");
            assert_eq!(avx_out2.as_slice(), sc_out2.as_slice(), "AVX2 neg5 b2");
        }

        // Word-boundary tests at n_words = {0, 4, 16, 64}.
        // 0 = empty; 4 = one AVX2 lane; 16 = four AVX2 lanes; 64 = sixteen.

        #[test]
        fn test_avx2_add5_matches_scalar_l0() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_add(0);
        }

        #[test]
        fn test_avx2_sub5_matches_scalar_l0() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_sub(0);
        }

        #[test]
        fn test_avx2_mul5_matches_scalar_l0() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_mul(0);
        }

        #[test]
        fn test_avx2_neg5_matches_scalar_l0() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_neg(0);
        }

        #[test]
        fn test_avx2_add5_matches_scalar_l4() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_add(4);
        }

        #[test]
        fn test_avx2_sub5_matches_scalar_l4() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_sub(4);
        }

        #[test]
        fn test_avx2_mul5_matches_scalar_l4() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_mul(4);
        }

        #[test]
        fn test_avx2_neg5_matches_scalar_l4() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_neg(4);
        }

        #[test]
        fn test_avx2_add5_matches_scalar_l16() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_add(16);
        }

        #[test]
        fn test_avx2_sub5_matches_scalar_l16() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_sub(16);
        }

        #[test]
        fn test_avx2_mul5_matches_scalar_l16() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_mul(16);
        }

        #[test]
        fn test_avx2_neg5_matches_scalar_l16() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_neg(16);
        }

        #[test]
        fn test_avx2_add5_matches_scalar_l64() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_add(64);
        }

        #[test]
        fn test_avx2_sub5_matches_scalar_l64() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_sub(64);
        }

        #[test]
        fn test_avx2_mul5_matches_scalar_l64() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_mul(64);
        }

        #[test]
        fn test_avx2_neg5_matches_scalar_l64() {
            if !has_avx2_f5() {
                return;
            }
            check_avx2_neg(64);
        }

        // -- Proptest cross-checks (1000 cases per op) vs scalar --

        /// Strategy: pick n_words from {0, 4, 8, 16} (multiples of 4) and two seeds.
        fn f5_word_batch_strategy() -> impl Strategy<Value = (usize, u64, u64)> {
            (
                prop_oneof![Just(0usize), Just(4), Just(8), Just(16)],
                any::<u64>(),
                any::<u64>(),
            )
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1000))]

            /// Cross-check AVX2 add vs scalar on 1000 random F_5 plane batches.
            #[test]
            fn test_avx2_add5_proptest((n_words, seed_a, seed_b) in f5_word_batch_strategy()) {
                if !has_avx2_f5() {
                    return Ok(());
                }
                let (b0a, b1a, b2a) = make_f5_words(n_words, seed_a);
                let (b0b, b1b, b2b) = make_f5_words(n_words, seed_b);
                let mut avx_out0 = vec![0u64; n_words];
                let mut avx_out1 = vec![0u64; n_words];
                let mut avx_out2 = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe {
                    crate::x86::bipedal_avx2_packed5::run_add5_batch(
                        &b0a, &b1a, &b2a, &b0b, &b1b, &b2b,
                        &mut avx_out0, &mut avx_out1, &mut avx_out2,
                    );
                }
                let mut sc_out0 = vec![0u64; n_words];
                let mut sc_out1 = vec![0u64; n_words];
                let mut sc_out2 = vec![0u64; n_words];
                scalar_add5_batch(
                    &b0a, &b1a, &b2a, &b0b, &b1b, &b2b,
                    &mut sc_out0, &mut sc_out1, &mut sc_out2,
                );
                prop_assert_eq!(avx_out0, sc_out0, "AVX2 add5 b0 proptest");
                prop_assert_eq!(avx_out1, sc_out1, "AVX2 add5 b1 proptest");
                prop_assert_eq!(avx_out2, sc_out2, "AVX2 add5 b2 proptest");
            }

            /// Cross-check AVX2 sub vs scalar on 1000 random F_5 plane batches.
            #[test]
            fn test_avx2_sub5_proptest((n_words, seed_a, seed_b) in f5_word_batch_strategy()) {
                if !has_avx2_f5() {
                    return Ok(());
                }
                let (b0a, b1a, b2a) = make_f5_words(n_words, seed_a);
                let (b0b, b1b, b2b) = make_f5_words(n_words, seed_b);
                let mut avx_out0 = vec![0u64; n_words];
                let mut avx_out1 = vec![0u64; n_words];
                let mut avx_out2 = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe {
                    crate::x86::bipedal_avx2_packed5::run_sub5_batch(
                        &b0a, &b1a, &b2a, &b0b, &b1b, &b2b,
                        &mut avx_out0, &mut avx_out1, &mut avx_out2,
                    );
                }
                let mut sc_out0 = vec![0u64; n_words];
                let mut sc_out1 = vec![0u64; n_words];
                let mut sc_out2 = vec![0u64; n_words];
                scalar_sub5_batch(
                    &b0a, &b1a, &b2a, &b0b, &b1b, &b2b,
                    &mut sc_out0, &mut sc_out1, &mut sc_out2,
                );
                prop_assert_eq!(avx_out0, sc_out0, "AVX2 sub5 b0 proptest");
                prop_assert_eq!(avx_out1, sc_out1, "AVX2 sub5 b1 proptest");
                prop_assert_eq!(avx_out2, sc_out2, "AVX2 sub5 b2 proptest");
            }

            /// Cross-check AVX2 mul vs scalar on 1000 random F_5 plane batches.
            #[test]
            fn test_avx2_mul5_proptest((n_words, seed_a, seed_b) in f5_word_batch_strategy()) {
                if !has_avx2_f5() {
                    return Ok(());
                }
                let (b0a, b1a, b2a) = make_f5_words(n_words, seed_a);
                let (b0b, b1b, b2b) = make_f5_words(n_words, seed_b);
                let mut avx_out0 = vec![0u64; n_words];
                let mut avx_out1 = vec![0u64; n_words];
                let mut avx_out2 = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe {
                    crate::x86::bipedal_avx2_packed5::run_mul5_batch(
                        &b0a, &b1a, &b2a, &b0b, &b1b, &b2b,
                        &mut avx_out0, &mut avx_out1, &mut avx_out2,
                    );
                }
                let mut sc_out0 = vec![0u64; n_words];
                let mut sc_out1 = vec![0u64; n_words];
                let mut sc_out2 = vec![0u64; n_words];
                scalar_mul5_batch(
                    &b0a, &b1a, &b2a, &b0b, &b1b, &b2b,
                    &mut sc_out0, &mut sc_out1, &mut sc_out2,
                );
                prop_assert_eq!(avx_out0, sc_out0, "AVX2 mul5 b0 proptest");
                prop_assert_eq!(avx_out1, sc_out1, "AVX2 mul5 b1 proptest");
                prop_assert_eq!(avx_out2, sc_out2, "AVX2 mul5 b2 proptest");
            }

            /// Cross-check AVX2 neg vs scalar on 1000 random F_5 plane batches.
            #[test]
            fn test_avx2_neg5_proptest((n_words, seed_a, _seed_b) in f5_word_batch_strategy()) {
                if !has_avx2_f5() {
                    return Ok(());
                }
                let (b0, b1, b2) = make_f5_words(n_words, seed_a);
                let mut avx_out0 = vec![0u64; n_words];
                let mut avx_out1 = vec![0u64; n_words];
                let mut avx_out2 = vec![0u64; n_words];
                // SAFETY: AVX2 verified; n_words is multiple of 4.
                unsafe {
                    crate::x86::bipedal_avx2_packed5::run_neg5_batch(
                        &b0, &b1, &b2,
                        &mut avx_out0, &mut avx_out1, &mut avx_out2,
                    );
                }
                let mut sc_out0 = vec![0u64; n_words];
                let mut sc_out1 = vec![0u64; n_words];
                let mut sc_out2 = vec![0u64; n_words];
                scalar_neg5_batch(&b0, &b1, &b2, &mut sc_out0, &mut sc_out1, &mut sc_out2);
                prop_assert_eq!(avx_out0, sc_out0, "AVX2 neg5 b0 proptest");
                prop_assert_eq!(avx_out1, sc_out1, "AVX2 neg5 b1 proptest");
                prop_assert_eq!(avx_out2, sc_out2, "AVX2 neg5 b2 proptest");
            }
        }

        // -- Scalar-fallback test (runs even on non-AVX2 hosts) --

        /// Verify scalar batch ops are consistent with word-level ops on any host.
        #[test]
        fn test_scalar5_batch_matches_word_ops_on_any_host() {
            let n_words = 8;
            let (b0a, b1a, b2a) = make_f5_words(n_words, 0xFACE_CAFE_BABE);
            let (b0b, b1b, b2b) = make_f5_words(n_words, 0xBEEF_DEAD_C0DE);
            let mut out0 = vec![0u64; n_words];
            let mut out1 = vec![0u64; n_words];
            let mut out2 = vec![0u64; n_words];
            scalar_add5_batch(
                &b0a, &b1a, &b2a, &b0b, &b1b, &b2b, &mut out0, &mut out1, &mut out2,
            );
            // Compare to per-word oracle.
            for wi in 0..n_words {
                let (e0, e1, e2) = add5_word(b0a[wi], b1a[wi], b2a[wi], b0b[wi], b1b[wi], b2b[wi]);
                assert_eq!(out0[wi], e0, "scalar batch add b0 word={wi}");
                assert_eq!(out1[wi], e1, "scalar batch add b1 word={wi}");
                assert_eq!(out2[wi], e2, "scalar batch add b2 word={wi}");
            }
        }
    }

    // -- Proptest cross-check vs gf2-algebra Packed5 scalar reference --
    // Cross-checks the word-level ops (both scalar and AVX2) against
    // gf2_algebra::packed::Packed5 per-lane decoded values.

    #[cfg(test)]
    mod packed5_cross {
        use super::*;
        use gf2_algebra::packed::{Packed5, PackedField};
        use gf2_core::gfp::Fp;

        /// Build a `Packed5` from a word-triple `(b0, b1, b2)` by encoding
        /// each of the 64 lanes from the bit-plane representation.
        fn packed5_from_words(b0: u64, b1: u64, b2: u64) -> Packed5 {
            let mut p = Packed5::zero();
            for i in 0..64usize {
                let val = ((b0 >> i) & 1) | (((b1 >> i) & 1) << 1) | (((b2 >> i) & 1) << 2);
                p = p.with_lane(i, Fp::<5>::new(val));
            }
            p
        }

        /// Recover `(b0, b1, b2)` words from a `Packed5` by reading per-lane values.
        fn words_from_packed5(p: Packed5) -> (u64, u64, u64) {
            let mut b0 = 0u64;
            let mut b1 = 0u64;
            let mut b2 = 0u64;
            for i in 0..64usize {
                let v = p.lane(i).value();
                if (v & 1) != 0 {
                    b0 |= 1u64 << i;
                }
                if (v & 2) != 0 {
                    b1 |= 1u64 << i;
                }
                if (v & 4) != 0 {
                    b2 |= 1u64 << i;
                }
            }
            (b0, b1, b2)
        }

        /// Strategy producing one canonical word-triple.
        fn word_triple_strategy() -> impl Strategy<Value = (u64, u64, u64)> {
            // Build a canonical packed word by generating 64 random F_5 values.
            prop::collection::vec(0u64..5u64, 64).prop_map(|vals| {
                let mut b0 = 0u64;
                let mut b1 = 0u64;
                let mut b2 = 0u64;
                for (i, &v) in vals.iter().enumerate() {
                    if (v & 1) != 0 {
                        b0 |= 1u64 << i;
                    }
                    if (v & 2) != 0 {
                        b1 |= 1u64 << i;
                    }
                    if (v & 4) != 0 {
                        b2 |= 1u64 << i;
                    }
                }
                (b0, b1, b2)
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1000))]

            /// Cross-check word-level add5_word against Packed5::add on 1000 inputs.
            #[test]
            fn test_add5_word_vs_packed5(
                (b0a, b1a, b2a) in word_triple_strategy(),
                (b0b, b1b, b2b) in word_triple_strategy(),
            ) {
                let pa = packed5_from_words(b0a, b1a, b2a);
                let pb = packed5_from_words(b0b, b1b, b2b);
                let expected = pa.add(pb);
                let (c0, c1, c2) = add5_word(b0a, b1a, b2a, b0b, b1b, b2b);
                let (ex_b0, ex_b1, ex_b2) = words_from_packed5(expected);
                prop_assert_eq!(c0, ex_b0, "add5 b0 mismatch");
                prop_assert_eq!(c1, ex_b1, "add5 b1 mismatch");
                prop_assert_eq!(c2, ex_b2, "add5 b2 mismatch");
            }

            /// Cross-check word-level sub5_word against Packed5::sub on 1000 inputs.
            #[test]
            fn test_sub5_word_vs_packed5(
                (b0a, b1a, b2a) in word_triple_strategy(),
                (b0b, b1b, b2b) in word_triple_strategy(),
            ) {
                let pa = packed5_from_words(b0a, b1a, b2a);
                let pb = packed5_from_words(b0b, b1b, b2b);
                let expected = pa.sub(pb);
                let (c0, c1, c2) = sub5_word(b0a, b1a, b2a, b0b, b1b, b2b);
                let (ex_b0, ex_b1, ex_b2) = words_from_packed5(expected);
                prop_assert_eq!(c0, ex_b0, "sub5 b0 mismatch");
                prop_assert_eq!(c1, ex_b1, "sub5 b1 mismatch");
                prop_assert_eq!(c2, ex_b2, "sub5 b2 mismatch");
            }

            /// Cross-check word-level mul5_word against Packed5::mul on 1000 inputs.
            #[test]
            fn test_mul5_word_vs_packed5(
                (b0a, b1a, b2a) in word_triple_strategy(),
                (b0b, b1b, b2b) in word_triple_strategy(),
            ) {
                let pa = packed5_from_words(b0a, b1a, b2a);
                let pb = packed5_from_words(b0b, b1b, b2b);
                let expected = pa.mul(pb);
                let (c0, c1, c2) = mul5_word(b0a, b1a, b2a, b0b, b1b, b2b);
                let (ex_b0, ex_b1, ex_b2) = words_from_packed5(expected);
                prop_assert_eq!(c0, ex_b0, "mul5 b0 mismatch");
                prop_assert_eq!(c1, ex_b1, "mul5 b1 mismatch");
                prop_assert_eq!(c2, ex_b2, "mul5 b2 mismatch");
            }

            /// Cross-check word-level neg5_word against Packed5::neg on 1000 inputs.
            #[test]
            fn test_neg5_word_vs_packed5(
                (b0, b1, b2) in word_triple_strategy(),
            ) {
                let pa = packed5_from_words(b0, b1, b2);
                let expected = pa.neg();
                let (c0, c1, c2) = neg5_word(b0, b1, b2);
                let (ex_b0, ex_b1, ex_b2) = words_from_packed5(expected);
                prop_assert_eq!(c0, ex_b0, "neg5 b0 mismatch");
                prop_assert_eq!(c1, ex_b1, "neg5 b1 mismatch");
                prop_assert_eq!(c2, ex_b2, "neg5 b2 mismatch");
            }
        }
    }
}
