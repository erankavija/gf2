//! F_3 instantiation of the generic [`super::framework::BatchedBipedalLike`]
//! framework. The `(mag, sgn)` encoding follows `@/citation/Scheinerman2024`
//! §2.2: `0 ↔ (0, 0)`, `1 ↔ (1, 0)`, `2 ↔ (1, 1)`; canonical pairs satisfy
//! `sgn & !mag == 0`, and the alt-zero `(0, 1)` also decodes to 0.

use super::framework::BipedalLikeConfig;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use super::lanes::Avx2Lane;
use super::lanes::BipedalLogicalLanes;

/// Returns `true` when the CPU supports AVX2; the result is cached in a
/// `OnceLock`.
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
/// Implements [`BipedalLikeConfig`] with the `@/citation/Scheinerman2024`
/// §2.2 formulas over [`Avx2Lane`].
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
        // Computed as `a + neg(b)` with `bsg = s2 ^ m2`, so the raw words
        // match the scalar reference `Bipedal3::sub_assign` bit-for-bit.
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
            let m_x = Avx2Lane::and(m1, m2);
            let s_x = Avx2Lane::xor(s1, s2);
            (m_x, s_x)
        }
    }

    #[inline(always)]
    unsafe fn neg_lane(m: Self::MagLane, s: Self::SgnLane) -> (Self::MagLane, Self::SgnLane) {
        // SAFETY: hardware feature is the caller's precondition.
        // `sgn ^ mag` flips sgn on nonzero lanes and leaves zero lanes
        // unchanged.
        unsafe {
            let s_neg = Avx2Lane::xor(s, m);
            (m, s_neg)
        }
    }
}

/// F_3 instantiation over [`Avx2Lane`]: 256 logical F_3 lanes per
/// `(mag, sgn)` lane pair.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub type Bipedal3x4 = super::framework::BatchedBipedalLike<Config3>;

#[cfg(test)]
mod tests {
    use super::*;

    use f3_bipedal_prototype::{Bipedal3, F3Encoding};

    fn encode_to_words(canonical: &[u8]) -> (Vec<u64>, Vec<u64>) {
        assert!(
            canonical.len().is_multiple_of(64),
            "test must use 64-aligned lengths"
        );
        let v = Bipedal3::pack(canonical);
        (v.raw_mag().to_vec(), v.raw_sgn().to_vec())
    }

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
        // `Bipedal3` has no `neg_assign`; neg(a) = 0 - a.
        for a in 0u8..3 {
            let zero = Bipedal3::pack(&[0]);
            let mut va = zero.clone();
            va.sub_assign(&Bipedal3::pack(&[a]));
            assert_eq!(va.unpack()[0], (3 - a) % 3, "Bipedal3 neg -{a}");
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    mod simd_parity {
        use super::*;
        use crate::x86::bipedal_avx2 as avx2;

        /// Compares raw `(mag, sgn)` words, so an alt-zero divergence that
        /// decodes to the same F_3 value still fails.
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
        /// (multiple of 64).
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

        // n_elems: 0 = empty, 256 = one AVX2 lane, 1024 = four, 4096 = sixteen.

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

        use proptest::prelude::*;

        /// Canonical F_3 element-pair strategy at lengths whose word count is
        /// a multiple of 4 (one AVX2 lane).
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

    /// Synthetic config with trivial formulas, used only by
    /// `test_framework_is_generic_over_config`.
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

    /// The generic AVX2 entry points monomorphise over a second
    /// `BipedalLikeConfig` impl.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_framework_is_generic_over_config() {
        type _Mock5x4 = super::super::framework::BatchedBipedalLike<MockConfig>;
        type BinaryKernel = unsafe fn(&[u64], &[u64], &[u64], &[u64], &mut [u64], &mut [u64]);
        let _add: BinaryKernel = crate::x86::bipedal_avx2::run_add_batch::<MockConfig>;
        let _sub: BinaryKernel = crate::x86::bipedal_avx2::run_sub_batch::<MockConfig>;
        let _mul: BinaryKernel = crate::x86::bipedal_avx2::run_mul_batch::<MockConfig>;
        assert_eq!(<MockConfig as BipedalLikeConfig>::PRIME, 5);
        assert_eq!(<MockConfig as BipedalLikeConfig>::U64_PER_LANE_PAIR, 4);
        assert_eq!(<Config3 as BipedalLikeConfig>::PRIME, 3);
        assert_eq!(<Config3 as BipedalLikeConfig>::U64_PER_LANE_PAIR, 4);
    }
}
