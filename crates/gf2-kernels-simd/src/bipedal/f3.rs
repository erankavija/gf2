//! F_3 instantiation of the generic [`super::framework::BatchedBipedalLike`]
//! framework.
//!
//! The F_3 (mag, sgn) encoding follows Scheinerman 2024 §2.2:
//!
//! - `0` ↔ `(mag=0, sgn=0)` (canonical zero)
//! - `1` ↔ `(mag=1, sgn=0)`
//! - `2` ↔ `(mag=1, sgn=1)`
//! - `(mag=0, sgn=1)` is an "alt-zero" — meaningless sgn bit on a
//!   zero magnitude lane. Production paths produce only canonical
//!   `(mag, sgn)` pairs satisfying `sgn & !mag == 0`.
//!
//! Add/sub/mul/neg formulas (paper Theorem 2.1):
//!
//! - add: `t = m1^s1^s2; u = m2&t; m_+ = u | (m1^m2); s_+ = u ^ s1`  (6 ops)
//! - sub: `t = s1^s2; u = m1&t; m_- = u | (m1^m2); s_- = u ^ (m2^s2)`  (6 ops)
//! - mul: `m_x = m1 & m2; s_x = s1 ^ s2`  (2 ops)
//! - neg: `(m', s') = (m, s ^ m)`  (1 op; flips sgn on every nonzero lane)
//!
//! The actual AVX2 batch entry points (`run_add_batch`, etc.) live in
//! [`crate::x86::bipedal_avx2`] so the asm-artefact-present gate fires on
//! source changes — see the W4 wave plan and `dev/plans/r4_simd_batching_decision.md`.

use super::framework::BipedalLikeConfig;
use super::lanes::BipedalLogicalLanes;

/// F_3 arithmetic recipe for the generic bipedal-like framework.
///
/// Implements [`BipedalLikeConfig`] using the Scheinerman 2024 §2.2
/// formulas. Each `*_lane` method is `#[inline(always)]` so it inlines
/// cleanly into the AVX2-feature-enabled batch entry points
/// (`run_*_batch`) defined in [`crate::x86::bipedal_avx2`].
///
/// This struct is zero-sized — it is a type-level tag only.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::framework::BipedalLikeConfig;
/// use gf2_kernels_simd::bipedal::Config3;
/// // PRIME is 3 for F_3.
/// assert_eq!(<Config3 as BipedalLikeConfig>::PRIME, 3);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct Config3;

impl BipedalLikeConfig for Config3 {
    const PRIME: u64 = 3;

    #[inline(always)]
    unsafe fn add_lane<Mag, Sgn>(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes,
    {
        // SAFETY: hardware feature is the caller's precondition.
        // The framework is shape-uniform: for the F_3 instantiation
        // `Mag = Sgn = Avx2Lane`, so the byte-level transmute is a
        // strict no-op (size_of and align_of match by construction).
        // For future encodings (F_5 D-bit-sliced, F_7 LUT-A) the lane
        // types may differ and the per-prime config will pick its own
        // shape — the `transmute_lane` indirection is what makes the F_3
        // body type-naturally generic.
        unsafe {
            let s1_as_m: Mag = transmute_lane(s1);
            let s2_as_m: Mag = transmute_lane(s2);
            let t = Mag::xor(Mag::xor(m1, s1_as_m), s2_as_m);
            let u = Mag::and(m2, t);
            let m_plus = Mag::or(u, Mag::xor(m1, m2));
            let s_plus_m: Mag = Mag::xor(u, s1_as_m);
            let s_plus: Sgn = transmute_lane(s_plus_m);
            (m_plus, s_plus)
        }
    }

    #[inline(always)]
    unsafe fn sub_lane<Mag, Sgn>(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes,
    {
        // SAFETY: hardware feature is the caller's precondition.
        unsafe {
            let s1_as_m: Mag = transmute_lane(s1);
            let s2_as_m: Mag = transmute_lane(s2);
            let m2_as_s: Sgn = transmute_lane(m2);
            let t_m = Mag::xor(s1_as_m, s2_as_m);
            let u = Mag::and(m1, t_m);
            let m_minus = Mag::or(u, Mag::xor(m1, m2));
            // s_- = u ^ (m2 ^ s2) — done in the Sgn type to mirror the
            // per-prime kernel's instruction order (R4 reference).
            let m2_xor_s2: Sgn = Sgn::xor(m2_as_s, s2);
            let u_as_s: Sgn = transmute_lane(u);
            let s_minus = Sgn::xor(u_as_s, m2_xor_s2);
            (m_minus, s_minus)
        }
    }

    #[inline(always)]
    unsafe fn mul_lane<Mag, Sgn>(m1: Mag, s1: Sgn, m2: Mag, s2: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes,
    {
        // SAFETY: hardware feature is the caller's precondition.
        unsafe {
            let m_x = Mag::and(m1, m2);
            let s_x = Sgn::xor(s1, s2);
            (m_x, s_x)
        }
    }

    #[inline(always)]
    unsafe fn neg_lane<Mag, Sgn>(m: Mag, s: Sgn) -> (Mag, Sgn)
    where
        Mag: BipedalLogicalLanes,
        Sgn: BipedalLogicalLanes,
    {
        // SAFETY: hardware feature is the caller's precondition.
        // F_3 canonical-form invariant `sgn & !mag == 0` => `sgn ^ mag`
        // flips sgn on nonzero lanes, leaves zero lanes invariant.
        // Equivalent to `sub(0, x)` but cheaper (1 op vs 6).
        unsafe {
            let m_as_s: Sgn = transmute_lane(m);
            let s_neg = Sgn::xor(s, m_as_s);
            (m, s_neg)
        }
    }
}

/// Concrete F_3 instantiation: 256-lane batched AVX2 over [`crate::bipedal::Avx2Lane`].
///
/// 4 × `u64` × 64 bits = 256 logical F_3 lanes per `(mag, sgn)` word-pair.
/// Both magnitude and sign use the AVX2 256-bit lane.
///
/// The associated batch entry points (`run_add_batch`, `run_sub_batch`,
/// `run_mul_batch`, `run_neg_batch`) live in [`crate::x86::bipedal_avx2`]
/// so the asm-artefact-present gate fires on changes to them. Call those
/// directly — they are re-exported via [`crate::bipedal`] for convenience.
///
/// # Examples
///
/// ```no_run
/// use gf2_kernels_simd::bipedal::Bipedal3x4;
/// // The type alias spells out the framework instantiation. The actual
/// // batch entry points live in `crate::x86::bipedal_avx2` and are
/// // re-exported from the module root for ergonomics.
/// // SAFETY: caller verifies AVX2 + slice lengths before invoking.
/// let _: fn() = || {
///     let _ = std::any::type_name::<Bipedal3x4>();
/// };
/// ```
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type Bipedal3x4 =
    super::framework::BatchedBipedalLike<Config3, super::lanes::Avx2Lane, super::lanes::Avx2Lane>;

/// Reinterpret one lane type as another of the same byte size.
///
/// For the F_3 instantiation `Mag = Sgn = Avx2Lane`, so this is a no-op.
/// In a future F_5 framework where `Mag != Sgn` the framework would
/// supply a domain-specific conversion instead; this helper exists
/// to keep the F_3 code path generic-looking.
///
/// # Safety
///
/// `From` and `To` must have the same memory layout. For the only
/// instantiated call site (`Mag = Sgn = Avx2Lane`) this is trivially true.
/// `debug_assert_eq!` on size_of/align_of guards against future misuse.
#[inline(always)]
unsafe fn transmute_lane<From: Copy, To: Copy>(x: From) -> To {
    debug_assert_eq!(core::mem::size_of::<From>(), core::mem::size_of::<To>());
    debug_assert_eq!(core::mem::align_of::<From>(), core::mem::align_of::<To>());
    // SAFETY: caller asserts From and To have the same layout. For the
    // only instantiated call site (Mag = Sgn = Avx2Lane) this is a true
    // no-op; the debug_asserts above check at runtime in debug builds.
    unsafe { core::mem::transmute_copy::<From, To>(&x) }
}

// =============================================================================
// Tests: SIMD-vs-scalar parity and a synthetic non-F_3 config for genericity
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Scalar oracle (test-only reference; T3 will land production scalar
    //      Bipedal3 in gf2-algebra; this oracle covers SIMD parity until then) ----

    /// Word-wise scalar bipedal F_3 add. One u64 word = 64 lanes; each lane
    /// applies the paper Theorem 2.1 6-op formula.
    ///
    /// The formulas operate on `u64`s directly because each F_3 lane is one
    /// bit, so the bitwise ops apply to all 64 lanes in parallel without any
    /// loop. The output `(mag, sgn)` satisfies the canonical invariant
    /// `sgn & !mag == 0` provided the inputs do.
    fn bipedal3_scalar_add(m1: u64, s1: u64, m2: u64, s2: u64) -> (u64, u64) {
        let t = m1 ^ s1 ^ s2;
        let u = m2 & t;
        let m_plus = u | (m1 ^ m2);
        let s_plus = u ^ s1;
        (m_plus, s_plus)
    }

    /// Word-wise scalar bipedal F_3 sub. See [`bipedal3_scalar_add`] for shape.
    fn bipedal3_scalar_sub(m1: u64, s1: u64, m2: u64, s2: u64) -> (u64, u64) {
        let t = s1 ^ s2;
        let u = m1 & t;
        let m_minus = u | (m1 ^ m2);
        let s_minus = u ^ (m2 ^ s2);
        (m_minus, s_minus)
    }

    /// Word-wise scalar bipedal F_3 mul.
    fn bipedal3_scalar_mul(m1: u64, s1: u64, m2: u64, s2: u64) -> (u64, u64) {
        let m_x = m1 & m2;
        let s_x = s1 ^ s2;
        (m_x, s_x)
    }

    /// Word-wise scalar bipedal F_3 neg.
    fn bipedal3_scalar_neg(m: u64, s: u64) -> (u64, u64) {
        (m, s ^ m)
    }

    // ---- Decode helper for sanity-check truth tables ----

    /// Decode a single bit pair to its canonical 0/1/2 value.
    fn decode_lane(mag: u8, sgn: u8) -> u8 {
        if mag == 0 {
            0
        } else if sgn == 0 {
            1
        } else {
            2
        }
    }

    /// Truth-table sanity check (3x3 grid) for the scalar add oracle.
    #[test]
    fn test_bipedal3_scalar_add_truth_table() {
        let enc = [(0u64, 0u64), (1, 0), (1, 1)];
        for a in 0..3 {
            for b in 0..3 {
                let (m1, s1) = enc[a];
                let (m2, s2) = enc[b];
                let (m, s) = bipedal3_scalar_add(m1, s1, m2, s2);
                assert_eq!(
                    decode_lane(m as u8, s as u8),
                    ((a as u8) + (b as u8)) % 3,
                    "scalar add {a} + {b}"
                );
            }
        }
    }

    /// Truth-table sanity check for the scalar sub oracle.
    #[test]
    fn test_bipedal3_scalar_sub_truth_table() {
        let enc = [(0u64, 0u64), (1, 0), (1, 1)];
        for a in 0..3 {
            for b in 0..3 {
                let (m1, s1) = enc[a];
                let (m2, s2) = enc[b];
                let (m, s) = bipedal3_scalar_sub(m1, s1, m2, s2);
                assert_eq!(
                    decode_lane(m as u8, s as u8),
                    ((a as u8) + 3 - (b as u8)) % 3,
                    "scalar sub {a} - {b}"
                );
            }
        }
    }

    /// Truth-table sanity check for the scalar mul oracle.
    #[test]
    fn test_bipedal3_scalar_mul_truth_table() {
        let enc = [(0u64, 0u64), (1, 0), (1, 1)];
        for a in 0..3 {
            for b in 0..3 {
                let (m1, s1) = enc[a];
                let (m2, s2) = enc[b];
                let (m, s) = bipedal3_scalar_mul(m1, s1, m2, s2);
                assert_eq!(
                    decode_lane(m as u8, s as u8),
                    ((a as u8) * (b as u8)) % 3,
                    "scalar mul {a} * {b}"
                );
            }
        }
    }

    /// Truth-table sanity check for the scalar neg oracle.
    #[test]
    fn test_bipedal3_scalar_neg_truth_table() {
        let enc = [(0u64, 0u64), (1, 0), (1, 1)];
        for (a, &(m, s)) in enc.iter().enumerate() {
            let (mn, sn) = bipedal3_scalar_neg(m, s);
            assert_eq!(
                decode_lane(mn as u8, sn as u8),
                (3 - (a as u8)) % 3,
                "scalar neg -{a}"
            );
        }
    }

    // ---- AVX2 SIMD parity tests vs scalar oracle ----

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    mod simd_parity {
        use super::*;
        use crate::x86::bipedal_avx2 as avx2;

        /// Helper: alloc same-size streams and run an op via the AVX2 batch
        /// entry point, then run the same op via the scalar oracle and assert
        /// pointwise equality.
        ///
        /// `n_words` is the number of u64 words per stream; must be a multiple
        /// of 4 (one AVX2 lane = 4 u64). `n_words = 0` is also allowed and
        /// must not segfault.
        fn run_parity_add(m1: &[u64], s1: &[u64], m2: &[u64], s2: &[u64]) {
            let n = m1.len();
            assert_eq!(n % 4, 0);
            let mut out_m = vec![0u64; n];
            let mut out_s = vec![0u64; n];
            // SAFETY: AVX2 verified by the calling test; lengths all equal n
            // and divisible by 4.
            unsafe {
                avx2::run_add_batch(m1, s1, m2, s2, &mut out_m, &mut out_s);
            }
            for i in 0..n {
                let (em, es) = bipedal3_scalar_add(m1[i], s1[i], m2[i], s2[i]);
                assert_eq!(out_m[i], em, "add mag mismatch at i={i}");
                assert_eq!(out_s[i], es, "add sgn mismatch at i={i}");
            }
        }

        fn run_parity_sub(m1: &[u64], s1: &[u64], m2: &[u64], s2: &[u64]) {
            let n = m1.len();
            assert_eq!(n % 4, 0);
            let mut out_m = vec![0u64; n];
            let mut out_s = vec![0u64; n];
            // SAFETY: AVX2 verified by the calling test; lengths all equal n.
            unsafe {
                avx2::run_sub_batch(m1, s1, m2, s2, &mut out_m, &mut out_s);
            }
            for i in 0..n {
                let (em, es) = bipedal3_scalar_sub(m1[i], s1[i], m2[i], s2[i]);
                assert_eq!(out_m[i], em, "sub mag mismatch at i={i}");
                assert_eq!(out_s[i], es, "sub sgn mismatch at i={i}");
            }
        }

        fn run_parity_mul(m1: &[u64], s1: &[u64], m2: &[u64], s2: &[u64]) {
            let n = m1.len();
            assert_eq!(n % 4, 0);
            let mut out_m = vec![0u64; n];
            let mut out_s = vec![0u64; n];
            // SAFETY: AVX2 verified by the calling test; lengths all equal n.
            unsafe {
                avx2::run_mul_batch(m1, s1, m2, s2, &mut out_m, &mut out_s);
            }
            for i in 0..n {
                let (em, es) = bipedal3_scalar_mul(m1[i], s1[i], m2[i], s2[i]);
                assert_eq!(out_m[i], em, "mul mag mismatch at i={i}");
                assert_eq!(out_s[i], es, "mul sgn mismatch at i={i}");
            }
        }

        fn run_parity_neg(m: &[u64], s: &[u64]) {
            let n = m.len();
            assert_eq!(n % 4, 0);
            let mut out_m = vec![0u64; n];
            let mut out_s = vec![0u64; n];
            // SAFETY: AVX2 verified by the calling test; lengths all equal n.
            unsafe {
                avx2::run_neg_batch(m, s, &mut out_m, &mut out_s);
            }
            for i in 0..n {
                let (em, es) = bipedal3_scalar_neg(m[i], s[i]);
                assert_eq!(out_m[i], em, "neg mag mismatch at i={i}");
                assert_eq!(out_s[i], es, "neg sgn mismatch at i={i}");
            }
        }

        /// Generate a canonical (mag, sgn) word-pair from a deterministic LCG
        /// seeded by the supplied state. The canonical invariant
        /// `sgn & !mag == 0` is enforced by AND'ing sgn with mag.
        fn lcg_canonical_pair(state: &mut u64) -> (u64, u64) {
            // 64-bit LCG (numerical recipes Knuth).
            *state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mag = *state;
            *state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let sgn_raw = *state;
            (mag, sgn_raw & mag)
        }

        fn make_canonical_streams(
            n_words: usize,
            seed: u64,
        ) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
            let mut state = seed;
            let mut m1 = Vec::with_capacity(n_words);
            let mut s1 = Vec::with_capacity(n_words);
            let mut m2 = Vec::with_capacity(n_words);
            let mut s2 = Vec::with_capacity(n_words);
            for _ in 0..n_words {
                let (a, b) = lcg_canonical_pair(&mut state);
                m1.push(a);
                s1.push(b);
                let (a, b) = lcg_canonical_pair(&mut state);
                m2.push(a);
                s2.push(b);
            }
            (m1, s1, m2, s2)
        }

        // ---- Word-boundary explicit tests at L = {0, 1, 4, 16, 64} ----

        /// L = 0: empty streams must not segfault.
        #[test]
        fn test_bipedal3_avx2_add_matches_scalar_l0() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(0, 0xDEAD_BEEF);
            run_parity_add(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_scalar_l0() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(0, 0xDEAD_BEEF);
            run_parity_sub(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_scalar_l0() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(0, 0xDEAD_BEEF);
            run_parity_mul(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_scalar_l0() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, _, _) = make_canonical_streams(0, 0xDEAD_BEEF);
            run_parity_neg(&m1, &s1);
        }

        /// L = 4 (one AVX2 lane).
        #[test]
        fn test_bipedal3_avx2_add_matches_scalar_l4() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(4, 1);
            run_parity_add(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_scalar_l4() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(4, 2);
            run_parity_sub(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_scalar_l4() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(4, 3);
            run_parity_mul(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_scalar_l4() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, _, _) = make_canonical_streams(4, 4);
            run_parity_neg(&m1, &s1);
        }

        /// L = 16 (four AVX2 lanes).
        #[test]
        fn test_bipedal3_avx2_add_matches_scalar_l16() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(16, 5);
            run_parity_add(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_scalar_l16() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(16, 6);
            run_parity_sub(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_scalar_l16() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(16, 7);
            run_parity_mul(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_scalar_l16() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, _, _) = make_canonical_streams(16, 8);
            run_parity_neg(&m1, &s1);
        }

        /// L = 64 (sixteen AVX2 lanes — covers loop iteration count).
        #[test]
        fn test_bipedal3_avx2_add_matches_scalar_l64() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(64, 9);
            run_parity_add(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_scalar_l64() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(64, 10);
            run_parity_sub(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_scalar_l64() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, m2, s2) = make_canonical_streams(64, 11);
            run_parity_mul(&m1, &s1, &m2, &s2);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_scalar_l64() {
            if !is_x86_feature_detected!("avx2") {
                return;
            }
            let (m1, s1, _, _) = make_canonical_streams(64, 12);
            run_parity_neg(&m1, &s1);
        }

        // ---- Proptest cross-checks (1000 cases per op, per spec) ----

        use proptest::prelude::*;

        /// Strategy: fixed-width canonical (mag, sgn) word streams. The
        /// `n_words` stays small so 1000 cases run well under the 5 s
        /// per-test limit.
        fn canonical_streams_strategy(
        ) -> impl Strategy<Value = (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>)> {
            // n_words ∈ {0, 4, 8, 16, 32}; multiple of 4 honours the AVX2 lane
            // contract. `0` exercises the empty-input boundary.
            (
                prop_oneof![Just(0usize), Just(4), Just(8), Just(16), Just(32)],
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
                any::<u64>(),
            )
                .prop_map(|(n_words, seed_a, seed_b, seed_c, seed_d)| {
                    let mut sa = seed_a;
                    let mut sb = seed_b;
                    let mut sc = seed_c;
                    let mut sd = seed_d;
                    let mut m1 = Vec::with_capacity(n_words);
                    let mut s1 = Vec::with_capacity(n_words);
                    let mut m2 = Vec::with_capacity(n_words);
                    let mut s2 = Vec::with_capacity(n_words);
                    for _ in 0..n_words {
                        // Each lane's mag & sgn pulled from independent LCGs;
                        // canonical invariant enforced by `sgn & mag`.
                        sa = sa.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                        sb = sb.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                        sc = sc.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                        sd = sd.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                        m1.push(sa);
                        s1.push(sb & sa);
                        m2.push(sc);
                        s2.push(sd & sc);
                    }
                    (m1, s1, m2, s2)
                })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1000))]

            /// Cross-check AVX2 add against the scalar oracle on 1000 random
            /// canonical-form (mag, sgn) word streams of varying length.
            #[test]
            fn test_bipedal3_avx2_add_matches_scalar_proptest(
                streams in canonical_streams_strategy(),
            ) {
                if !is_x86_feature_detected!("avx2") {
                    return Ok(());
                }
                let (m1, s1, m2, s2) = streams;
                run_parity_add(&m1, &s1, &m2, &s2);
            }

            /// Cross-check AVX2 sub against the scalar oracle on 1000 random
            /// canonical-form (mag, sgn) word streams.
            #[test]
            fn test_bipedal3_avx2_sub_matches_scalar_proptest(
                streams in canonical_streams_strategy(),
            ) {
                if !is_x86_feature_detected!("avx2") {
                    return Ok(());
                }
                let (m1, s1, m2, s2) = streams;
                run_parity_sub(&m1, &s1, &m2, &s2);
            }

            /// Cross-check AVX2 mul against the scalar oracle on 1000 random
            /// canonical-form (mag, sgn) word streams.
            #[test]
            fn test_bipedal3_avx2_mul_matches_scalar_proptest(
                streams in canonical_streams_strategy(),
            ) {
                if !is_x86_feature_detected!("avx2") {
                    return Ok(());
                }
                let (m1, s1, m2, s2) = streams;
                run_parity_mul(&m1, &s1, &m2, &s2);
            }

            /// Cross-check AVX2 neg against the scalar oracle on 1000 random
            /// canonical-form (mag, sgn) word streams.
            #[test]
            fn test_bipedal3_avx2_neg_matches_scalar_proptest(
                streams in canonical_streams_strategy(),
            ) {
                if !is_x86_feature_detected!("avx2") {
                    return Ok(());
                }
                let (m1, s1, _, _) = streams;
                run_parity_neg(&m1, &s1);
            }
        }
    }

    // ---- Genericity demonstration: a synthetic non-F_3 config ----

    /// Synthetic config used only by `test_framework_is_generic_over_config`.
    /// Implements `BipedalLikeConfig` with trivial formulas to demonstrate
    /// that adding a new prime requires only a new config impl, no new
    /// kernel code (success criterion 4).
    ///
    /// The "arithmetic" here is intentionally trivial — `add_lane` returns
    /// the first operand unchanged; we are only checking that the trait
    /// bound machinery resolves and the body type-checks.
    #[derive(Clone, Copy, Debug, Default)]
    struct MockConfig;

    impl BipedalLikeConfig for MockConfig {
        const PRIME: u64 = 5;

        #[inline(always)]
        unsafe fn add_lane<Mag, Sgn>(m1: Mag, s1: Sgn, _m2: Mag, _s2: Sgn) -> (Mag, Sgn)
        where
            Mag: BipedalLogicalLanes,
            Sgn: BipedalLogicalLanes,
        {
            (m1, s1)
        }

        #[inline(always)]
        unsafe fn sub_lane<Mag, Sgn>(m1: Mag, s1: Sgn, _m2: Mag, _s2: Sgn) -> (Mag, Sgn)
        where
            Mag: BipedalLogicalLanes,
            Sgn: BipedalLogicalLanes,
        {
            (m1, s1)
        }

        #[inline(always)]
        unsafe fn mul_lane<Mag, Sgn>(m1: Mag, s1: Sgn, _m2: Mag, _s2: Sgn) -> (Mag, Sgn)
        where
            Mag: BipedalLogicalLanes,
            Sgn: BipedalLogicalLanes,
        {
            (m1, s1)
        }

        #[inline(always)]
        unsafe fn neg_lane<Mag, Sgn>(m: Mag, s: Sgn) -> (Mag, Sgn)
        where
            Mag: BipedalLogicalLanes,
            Sgn: BipedalLogicalLanes,
        {
            (m, s)
        }
    }

    /// Demonstrates that `BatchedBipedalLike` is generic over the config
    /// parameter — instantiating with a fresh `BipedalLikeConfig` impl
    /// requires zero kernel-code changes (success criterion 4).
    ///
    /// We only need the type to resolve and the const PRIME to be
    /// reachable; we don't actually issue an AVX2 op.
    #[test]
    fn test_framework_is_generic_over_config() {
        type _Mock5x4 = super::super::framework::BatchedBipedalLike<
            MockConfig,
            super::super::lanes::Avx2Lane,
            super::super::lanes::Avx2Lane,
        >;
        // The fact that the type alias above resolved already proves
        // genericity; the assertion here just ties the const through.
        assert_eq!(<MockConfig as BipedalLikeConfig>::PRIME, 5);
        assert_eq!(<Config3 as BipedalLikeConfig>::PRIME, 3);
    }
}
