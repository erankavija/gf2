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
//! For F_3 the magnitude and sign lane shapes coincide (both are
//! [`super::lanes::Avx2Lane`]); the per-prime config selects this via the
//! `MagLane` / `SgnLane` associated types on
//! [`super::framework::BipedalLikeConfig`]. Future F_5 D-bit-sliced (W4)
//! is expected to pick a wider magnitude lane and a narrower sign lane
//! through the same trait — no framework-body changes will be needed.
//!
//! The actual AVX2 batch entry points (`run_add_batch`, etc.) live in
//! [`crate::x86::bipedal_avx2`] so the asm-artefact-present gate fires on
//! source changes — see the W4 wave plan and `dev/plans/r4_simd_batching_decision.md`.

use super::framework::BipedalLikeConfig;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use super::lanes::Avx2Lane;
use super::lanes::BipedalLogicalLanes;

/// Returns `true` when the CPU supports AVX2, `false` otherwise.
///
/// The result is cached in a `OnceLock<bool>` so CPUID is queried at most
/// once per process — matching the project's `simd::maybe_simd()` pattern
/// from `gf2-core` (CLAUDE.md §Architecture point 3). Callers in this module
/// use this instead of bare `is_x86_feature_detected!("avx2")` to make the
/// caching visible and auditable.
///
/// # Complexity
///
/// `O(1)` after the first call (CPUID result is cached).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn has_avx2() -> bool {
    use std::sync::OnceLock;
    static AVX2: OnceLock<bool> = OnceLock::new();
    *AVX2.get_or_init(|| {
        use std::arch::is_x86_feature_detected;
        is_x86_feature_detected!("avx2")
    })
}

/// F_3 arithmetic recipe for the generic bipedal-like framework.
///
/// Implements [`BipedalLikeConfig`] using the Scheinerman 2024 §2.2
/// formulas. The associated types `MagLane` and `SgnLane` both pick
/// [`Avx2Lane`] (the only lane shape currently wired); each `*_lane`
/// method is `#[inline(always)]` so it inlines cleanly into the
/// AVX2-feature-enabled batch entry points (`run_*_batch`) defined in
/// [`crate::x86::bipedal_avx2`].
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
/// // U64_PER_LANE_PAIR is 4 (256-bit Avx2Lane = 4 × u64).
/// assert_eq!(<Config3 as BipedalLikeConfig>::U64_PER_LANE_PAIR, 4);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct Config3;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl BipedalLikeConfig for Config3 {
    type MagLane = Avx2Lane;
    type SgnLane = Avx2Lane;
    const PRIME: u64 = 3;
    const U64_PER_LANE_PAIR: usize = 4;

    #[inline(always)]
    unsafe fn add_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane) {
        // SAFETY: hardware feature is the caller's precondition.
        unsafe {
            // F_3 add: t = m1^s1^s2; u = m2&t; m_+ = u | (m1^m2); s_+ = u^s1
            let t = Avx2Lane::xor(Avx2Lane::xor(m1, s1), s2);
            let u = Avx2Lane::and(m2, t);
            let m_plus = Avx2Lane::or(u, Avx2Lane::xor(m1, m2));
            let s_plus = Avx2Lane::xor(u, s1);
            (m_plus, s_plus)
        }
    }

    #[inline(always)]
    unsafe fn sub_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane) {
        // SAFETY: hardware feature is the caller's precondition.
        // F_3 sub computed as `a + neg(b)`, matching the scalar reference
        // (`dev/research/f3_bipedal::Bipedal3::sub_assign`) bit-for-bit so
        // the SIMD parity tests can assert raw-word equality, not just
        // canonical-decoded equality. The 7-op sequence is the same paper
        // Algorithm 2 add formula applied with `bsg = s2 ^ m2` (neg(b)).
        unsafe {
            let bsg = Avx2Lane::xor(s2, m2);
            let t = Avx2Lane::xor(Avx2Lane::xor(m1, s1), bsg);
            let u = Avx2Lane::and(m2, t);
            let m_minus = Avx2Lane::or(u, Avx2Lane::xor(m1, m2));
            let s_minus = Avx2Lane::xor(u, s1);
            (m_minus, s_minus)
        }
    }

    #[inline(always)]
    unsafe fn mul_lane(
        m1: Self::MagLane,
        s1: Self::SgnLane,
        m2: Self::MagLane,
        s2: Self::SgnLane,
    ) -> (Self::MagLane, Self::SgnLane) {
        // SAFETY: hardware feature is the caller's precondition.
        unsafe {
            // F_3 mul: m_x = m1 & m2; s_x = s1 ^ s2
            let m_x = Avx2Lane::and(m1, m2);
            let s_x = Avx2Lane::xor(s1, s2);
            (m_x, s_x)
        }
    }

    #[inline(always)]
    unsafe fn neg_lane(m: Self::MagLane, s: Self::SgnLane) -> (Self::MagLane, Self::SgnLane) {
        // SAFETY: hardware feature is the caller's precondition.
        // F_3 canonical-form invariant `sgn & !mag == 0` => `sgn ^ mag`
        // flips sgn on nonzero lanes, leaves zero lanes invariant.
        // Equivalent to `sub(0, x)` but cheaper (1 op vs 6).
        unsafe {
            let s_neg = Avx2Lane::xor(s, m);
            (m, s_neg)
        }
    }
}

/// Concrete F_3 instantiation: 256-lane batched AVX2 over [`Avx2Lane`].
///
/// 4 × `u64` × 64 bits = 256 logical F_3 lanes per `(mag, sgn)` word-pair.
/// Both magnitude and sign use the AVX2 256-bit lane via the
/// [`Config3::MagLane`] / [`Config3::SgnLane`] associated types.
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
pub type Bipedal3x4 = super::framework::BatchedBipedalLike<Config3>;

// =============================================================================
// Tests: SIMD-vs-scalar parity against the SSOT `Bipedal3` reference
// (dev/research/f3_bipedal::Bipedal3) and a synthetic non-F_3 config for
// genericity demonstration.
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // The canonical scalar reference is the standalone `Bipedal3` prototype
    // at dev/research/f3_bipedal::Bipedal3 (paper Theorem 2.1, six-op
    // formula). Per b17bec62 success criterion 3 and the SSOT rule
    // ("no custom implementations of what already exists"), the AVX2
    // parity tests below cross-check against that one reference rather
    // than carrying a duplicate inline oracle.
    use f3_bipedal_prototype::{Bipedal3, F3Encoding};

    // ---- Encoding helper: derive packed (mag, sgn) word streams via SSOT ----
    //
    // Per the SSOT rule we route every encoding through `Bipedal3::pack`
    // (paper Theorem 2.1 reference) and read its raw word arrays back via
    // `raw_mag()` / `raw_sgn()` rather than carrying a duplicate packing loop
    // here. The tests below use 64-aligned lengths so the AVX2 mod-4-words
    // contract is trivially satisfied without any sub-word bookkeeping.
    fn encode_to_words(canonical: &[u8]) -> (Vec<u64>, Vec<u64>) {
        assert!(
            canonical.len().is_multiple_of(64),
            "test must use 64-aligned lengths"
        );
        let v = Bipedal3::pack(canonical);
        (v.raw_mag().to_vec(), v.raw_sgn().to_vec())
    }

    // ---- Truth-table sanity checks (3x3 grid) against `Bipedal3` ----
    //
    // Each test packs a single-element vector with the canonical reference,
    // applies the op, and asserts the unpacked result matches the F_3
    // ground-truth table. These run quickly and serve as smoke tests
    // independent of the SIMD path.

    #[test]
    fn test_bipedal3_reference_add_truth_table() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.add_assign(&vb);
                assert_eq!(va.unpack()[0], (a + b) % 3, "Bipedal3 add {a} + {b}");
            }
        }
    }

    #[test]
    fn test_bipedal3_reference_sub_truth_table() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.sub_assign(&vb);
                assert_eq!(va.unpack()[0], (a + 3 - b) % 3, "Bipedal3 sub {a} - {b}");
            }
        }
    }

    #[test]
    fn test_bipedal3_reference_mul_truth_table() {
        for a in 0u8..3 {
            for b in 0u8..3 {
                let mut va = Bipedal3::pack(&[a]);
                let vb = Bipedal3::pack(&[b]);
                va.mul_assign(&vb);
                assert_eq!(va.unpack()[0], (a * b) % 3, "Bipedal3 mul {a} * {b}");
            }
        }
    }

    #[test]
    fn test_bipedal3_reference_neg_truth_table() {
        // Bipedal3 has no neg_assign; the framework's neg(a) = sub(0, a)
        // by definition. Use sub against zero to exercise neg semantics.
        for a in 0u8..3 {
            let zero = Bipedal3::pack(&[0]);
            let mut va = zero.clone();
            va.sub_assign(&Bipedal3::pack(&[a]));
            assert_eq!(va.unpack()[0], (3 - a) % 3, "Bipedal3 neg -{a}");
        }
    }

    // ---- AVX2 SIMD parity tests vs the SSOT `Bipedal3` reference ----

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    mod simd_parity {
        use super::*;
        use crate::x86::bipedal_avx2 as avx2;

        /// Run AVX2 add on the bit-packed encoding of `a`/`b` and assert
        /// bitwise equality of the resulting `(mag, sgn)` word streams
        /// against `Bipedal3::pack(a).add_assign(b)`'s internal raw words.
        ///
        /// Raw-word comparison is required: comparing only canonical-decoded
        /// outputs would let alt-zero divergences (`(mag=0, sgn=1)` vs
        /// `(mag=0, sgn=0)`) slip through even though they decode to the
        /// same F_3 value. Both implementations follow paper Algorithm 2
        /// (same six XOR/AND/OR sequence), so the raw `(mag, sgn)` buffers
        /// must agree word-for-word.
        fn run_parity_add(a: &[u8], b: &[u8]) {
            assert_eq!(a.len(), b.len());
            let n_elems = a.len();
            let n_words = n_elems / 64;
            assert_eq!(
                n_words % 4,
                0,
                "n_words must be a multiple of 4 (AVX2 lane)"
            );

            let (m1, s1) = encode_to_words(a);
            let (m2, s2) = encode_to_words(b);
            let mut out_m = vec![0u64; n_words];
            let mut out_s = vec![0u64; n_words];
            // SAFETY: AVX2 verified by the calling test; lengths all equal
            // n_words and divisible by 4.
            unsafe {
                avx2::run_add_batch::<Config3>(&m1, &s1, &m2, &s2, &mut out_m, &mut out_s);
            }

            let mut va = Bipedal3::pack(a);
            va.add_assign(&Bipedal3::pack(b));

            assert_eq!(
                out_m.as_slice(),
                va.raw_mag(),
                "AVX2 add diverged from Bipedal3 scalar reference (mag, n_elems={n_elems})"
            );
            assert_eq!(
                out_s.as_slice(),
                va.raw_sgn(),
                "AVX2 add diverged from Bipedal3 scalar reference (sgn, n_elems={n_elems})"
            );
        }

        fn run_parity_sub(a: &[u8], b: &[u8]) {
            assert_eq!(a.len(), b.len());
            let n_elems = a.len();
            let n_words = n_elems / 64;
            assert_eq!(n_words % 4, 0);

            let (m1, s1) = encode_to_words(a);
            let (m2, s2) = encode_to_words(b);
            let mut out_m = vec![0u64; n_words];
            let mut out_s = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths all equal n_words and divisible by 4.
            unsafe {
                avx2::run_sub_batch::<Config3>(&m1, &s1, &m2, &s2, &mut out_m, &mut out_s);
            }

            let mut va = Bipedal3::pack(a);
            va.sub_assign(&Bipedal3::pack(b));

            assert_eq!(
                out_m.as_slice(),
                va.raw_mag(),
                "AVX2 sub diverged from Bipedal3 scalar reference (mag, n_elems={n_elems})"
            );
            assert_eq!(
                out_s.as_slice(),
                va.raw_sgn(),
                "AVX2 sub diverged from Bipedal3 scalar reference (sgn, n_elems={n_elems})"
            );
        }

        fn run_parity_mul(a: &[u8], b: &[u8]) {
            assert_eq!(a.len(), b.len());
            let n_elems = a.len();
            let n_words = n_elems / 64;
            assert_eq!(n_words % 4, 0);

            let (m1, s1) = encode_to_words(a);
            let (m2, s2) = encode_to_words(b);
            let mut out_m = vec![0u64; n_words];
            let mut out_s = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths all equal n_words and divisible by 4.
            unsafe {
                avx2::run_mul_batch::<Config3>(&m1, &s1, &m2, &s2, &mut out_m, &mut out_s);
            }

            let mut va = Bipedal3::pack(a);
            va.mul_assign(&Bipedal3::pack(b));

            assert_eq!(
                out_m.as_slice(),
                va.raw_mag(),
                "AVX2 mul diverged from Bipedal3 scalar reference (mag, n_elems={n_elems})"
            );
            assert_eq!(
                out_s.as_slice(),
                va.raw_sgn(),
                "AVX2 mul diverged from Bipedal3 scalar reference (sgn, n_elems={n_elems})"
            );
        }

        fn run_parity_neg(a: &[u8]) {
            let n_elems = a.len();
            let n_words = n_elems / 64;
            assert_eq!(n_words % 4, 0);

            let (m, s) = encode_to_words(a);
            let mut out_m = vec![0u64; n_words];
            let mut out_s = vec![0u64; n_words];
            // SAFETY: AVX2 verified; lengths all equal n_words and divisible by 4.
            unsafe {
                avx2::run_neg_batch::<Config3>(&m, &s, &mut out_m, &mut out_s);
            }

            // `Bipedal3` has no public `neg_assign`; neg(a) = 0 - a.
            let zero = vec![0u8; n_elems];
            let mut va = Bipedal3::pack(&zero);
            va.sub_assign(&Bipedal3::pack(a));

            assert_eq!(
                out_m.as_slice(),
                va.raw_mag(),
                "AVX2 neg diverged from Bipedal3 scalar reference (mag, n_elems={n_elems})"
            );
            assert_eq!(
                out_s.as_slice(),
                va.raw_sgn(),
                "AVX2 neg diverged from Bipedal3 scalar reference (sgn, n_elems={n_elems})"
            );
        }

        /// Deterministic LCG-driven canonical F_3 vector of length `n_elems`
        /// (multiple of 64). Uses two-bit rejection sampling on a 64-bit
        /// LCG state to draw uniform 0..=2.
        fn make_canonical_vec(n_elems: usize, seed: u64) -> Vec<u8> {
            assert_eq!(n_elems % 64, 0);
            let mut state = seed;
            let mut out = Vec::with_capacity(n_elems);
            while out.len() < n_elems {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let mut x = state;
                for _ in 0..30 {
                    let r = (x & 0x3) as u8;
                    x >>= 2;
                    if r < 3 && out.len() < n_elems {
                        out.push(r);
                    }
                }
            }
            out
        }

        // ---- Word-boundary explicit tests at n_elems = {0, 256, 1024, 4096} ----
        //
        // 0 = empty, 256 = 4 words = one AVX2 lane, 1024 = 16 words = four
        // AVX2 lanes, 4096 = 64 words = sixteen AVX2 lanes (loop iteration
        // coverage). All multiples of 64 (one Bipedal3 word).

        #[test]
        fn test_bipedal3_avx2_add_matches_reference_l0() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(0, 0xDEAD_BEEF);
            let b = make_canonical_vec(0, 0xCAFE_F00D);
            run_parity_add(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_reference_l0() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(0, 0xDEAD_BEEF);
            let b = make_canonical_vec(0, 0xCAFE_F00D);
            run_parity_sub(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_reference_l0() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(0, 0xDEAD_BEEF);
            let b = make_canonical_vec(0, 0xCAFE_F00D);
            run_parity_mul(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_reference_l0() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(0, 0xDEAD_BEEF);
            run_parity_neg(&a);
        }

        #[test]
        fn test_bipedal3_avx2_add_matches_reference_l256() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(256, 1);
            let b = make_canonical_vec(256, 2);
            run_parity_add(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_reference_l256() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(256, 3);
            let b = make_canonical_vec(256, 4);
            run_parity_sub(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_reference_l256() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(256, 5);
            let b = make_canonical_vec(256, 6);
            run_parity_mul(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_reference_l256() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(256, 7);
            run_parity_neg(&a);
        }

        #[test]
        fn test_bipedal3_avx2_add_matches_reference_l1024() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(1024, 11);
            let b = make_canonical_vec(1024, 12);
            run_parity_add(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_reference_l1024() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(1024, 13);
            let b = make_canonical_vec(1024, 14);
            run_parity_sub(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_reference_l1024() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(1024, 15);
            let b = make_canonical_vec(1024, 16);
            run_parity_mul(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_reference_l1024() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(1024, 17);
            run_parity_neg(&a);
        }

        #[test]
        fn test_bipedal3_avx2_add_matches_reference_l4096() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(4096, 21);
            let b = make_canonical_vec(4096, 22);
            run_parity_add(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_sub_matches_reference_l4096() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(4096, 23);
            let b = make_canonical_vec(4096, 24);
            run_parity_sub(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_mul_matches_reference_l4096() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(4096, 25);
            let b = make_canonical_vec(4096, 26);
            run_parity_mul(&a, &b);
        }

        #[test]
        fn test_bipedal3_avx2_neg_matches_reference_l4096() {
            if !has_avx2() {
                return;
            }
            let a = make_canonical_vec(4096, 27);
            run_parity_neg(&a);
        }

        // ---- Proptest cross-checks (1000 cases per op) vs `Bipedal3` ----

        use proptest::prelude::*;

        /// Canonical F_3 element-pair strategy at SIMD-aligned lengths.
        ///
        /// `n_elems` is one of `{0, 256, 512, 1024, 2048}`; all multiples
        /// of 64 (= one Bipedal3 word) AND yield a `n_words` that is a
        /// multiple of 4 (one AVX2 lane). `0` exercises the empty-input
        /// boundary. Length stays small so 1000 cases run well under the
        /// 5 s per-test limit.
        fn canonical_pair_strategy() -> impl Strategy<Value = (Vec<u8>, Vec<u8>)> {
            (
                prop_oneof![Just(0usize), Just(256), Just(512), Just(1024), Just(2048),],
                any::<u64>(),
                any::<u64>(),
            )
                .prop_map(|(n, seed_a, seed_b)| {
                    (make_canonical_vec(n, seed_a), make_canonical_vec(n, seed_b))
                })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(1000))]

            /// Cross-check AVX2 add against `dev/research/f3_bipedal::Bipedal3`
            /// on 1000 random canonical-form F_3 vectors of varying length.
            #[test]
            fn test_bipedal3_avx2_add_matches_reference_proptest(
                pair in canonical_pair_strategy(),
            ) {
                if !has_avx2() {
                    return Ok(());
                }
                let (a, b) = pair;
                run_parity_add(&a, &b);
            }

            /// Cross-check AVX2 sub against `dev/research/f3_bipedal::Bipedal3`
            /// on 1000 random canonical-form F_3 vectors.
            #[test]
            fn test_bipedal3_avx2_sub_matches_reference_proptest(
                pair in canonical_pair_strategy(),
            ) {
                if !has_avx2() {
                    return Ok(());
                }
                let (a, b) = pair;
                run_parity_sub(&a, &b);
            }

            /// Cross-check AVX2 mul against `dev/research/f3_bipedal::Bipedal3`
            /// on 1000 random canonical-form F_3 vectors.
            #[test]
            fn test_bipedal3_avx2_mul_matches_reference_proptest(
                pair in canonical_pair_strategy(),
            ) {
                if !has_avx2() {
                    return Ok(());
                }
                let (a, b) = pair;
                run_parity_mul(&a, &b);
            }

            /// Cross-check AVX2 neg against `dev/research/f3_bipedal::Bipedal3`
            /// on 1000 random canonical-form F_3 vectors.
            #[test]
            fn test_bipedal3_avx2_neg_matches_reference_proptest(
                pair in canonical_pair_strategy(),
            ) {
                if !has_avx2() {
                    return Ok(());
                }
                let (a, _) = pair;
                run_parity_neg(&a);
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
    ///
    /// Picks the same lane shape as F_3 (`Avx2Lane` for both `MagLane` and
    /// `SgnLane`) so the existing AVX2 entry points monomorphise without
    /// any per-config kernel code; a real F_5 / F_7 config can pick a
    /// different shape via the same associated-type machinery.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[derive(Clone, Copy, Debug, Default)]
    struct MockConfig;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    impl BipedalLikeConfig for MockConfig {
        type MagLane = Avx2Lane;
        type SgnLane = Avx2Lane;
        const PRIME: u64 = 5;
        const U64_PER_LANE_PAIR: usize = 4;

        #[inline(always)]
        unsafe fn add_lane(
            m1: Self::MagLane,
            s1: Self::SgnLane,
            _m2: Self::MagLane,
            _s2: Self::SgnLane,
        ) -> (Self::MagLane, Self::SgnLane) {
            (m1, s1)
        }

        #[inline(always)]
        unsafe fn sub_lane(
            m1: Self::MagLane,
            s1: Self::SgnLane,
            _m2: Self::MagLane,
            _s2: Self::SgnLane,
        ) -> (Self::MagLane, Self::SgnLane) {
            (m1, s1)
        }

        #[inline(always)]
        unsafe fn mul_lane(
            m1: Self::MagLane,
            s1: Self::SgnLane,
            _m2: Self::MagLane,
            _s2: Self::SgnLane,
        ) -> (Self::MagLane, Self::SgnLane) {
            (m1, s1)
        }

        #[inline(always)]
        unsafe fn neg_lane(m: Self::MagLane, s: Self::SgnLane) -> (Self::MagLane, Self::SgnLane) {
            (m, s)
        }
    }

    /// Demonstrates that `BatchedBipedalLike` is generic over the config
    /// parameter — instantiating with a fresh `BipedalLikeConfig` impl
    /// requires zero kernel-code changes (success criterion 4). The
    /// generic AVX2 entry points `run_*_batch::<C>` in
    /// `crate::x86::bipedal_avx2` accept the new config without
    /// modification; the type-level check below proves the trait machinery
    /// resolves end-to-end.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_framework_is_generic_over_config() {
        type _Mock5x4 = super::super::framework::BatchedBipedalLike<MockConfig>;
        // Reference the generic entry points monomorphised over the new
        // config to prove they accept it without source changes. The
        // `unsafe fn` pointer cast below is sound because the AVX2 entry
        // points share a uniform shape across configs whose `MagLane` and
        // `SgnLane` resolve to the same lane type (`Avx2Lane` here).
        type BinaryKernel = unsafe fn(&[u64], &[u64], &[u64], &[u64], &mut [u64], &mut [u64]);
        let _add: BinaryKernel = crate::x86::bipedal_avx2::run_add_batch::<MockConfig>;
        let _sub: BinaryKernel = crate::x86::bipedal_avx2::run_sub_batch::<MockConfig>;
        let _mul: BinaryKernel = crate::x86::bipedal_avx2::run_mul_batch::<MockConfig>;
        // The fact that the type alias and the generic-fn pointers above
        // resolved already proves genericity; the assertions here just tie
        // the constants through.
        assert_eq!(<MockConfig as BipedalLikeConfig>::PRIME, 5);
        assert_eq!(<MockConfig as BipedalLikeConfig>::U64_PER_LANE_PAIR, 4);
        assert_eq!(<Config3 as BipedalLikeConfig>::PRIME, 3);
        assert_eq!(<Config3 as BipedalLikeConfig>::U64_PER_LANE_PAIR, 4);
    }
}
