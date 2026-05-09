//! Scalar reference implementation of [`PackedField<Fp<3>>`].
//!
//! [`ScalarPackedFp3`] is the *correctness oracle* against which the
//! optimised `Bipedal3` (W1-T3) and the future `Bipedal5` / `Bipedal7`
//! impls are cross-checked. The implementation is intentionally
//! one-`Fp<3>`-per-lane: no SIMD, no bit-packing, no popcount tricks.
//! Every method is the literal lane-wise composition of the underlying
//! `Fp<3>` operator.
//!
//! # LANES choice
//!
//! `LANES = 64` matches the bipedal3 lane count fixed in the parent
//! epic design (`dev/plans/gf2_algebra_permanent.md` §7.1) and the
//! D1b §4 stub conformance walk-through. Choosing the same width
//! makes a 1:1 cross-check loop trivially writable: a test routes the
//! same 64-lane input through both [`ScalarPackedFp3`] and the
//! optimised `Bipedal3`, then compares `lane(i)` for `i` in `0..64`.
//!
//! # Boundary against optimised impls
//!
//! [`ScalarPackedFp3`] is **not** a perf path. It exists exclusively
//! to anchor correctness. Production callers (Ryser, the `permanent_*`
//! family) reach for `Bipedal3` once W1-T3 lands; this oracle is only
//! reached through unit tests and `proptest` cross-checks.

use core::fmt;

use gf2_core::gfp::Fp;

use super::PackedField;

/// Scalar reference implementation of [`PackedField<Fp<3>>`] over a
/// fixed `LANES = 64` array of `Fp<3>` elements.
///
/// One `Fp<3>` per lane. No bit-packing, no SIMD, no encoding tricks.
/// This is the correctness oracle for the optimised `Bipedal3` impl
/// (W1-T3) and the F_5 / F_7 impls (W4); all of those are
/// cross-checked against this type via per-lane equality.
///
/// `LANES = 64` is fixed to match the `Bipedal3` lane count from the
/// parent epic design (`dev/plans/gf2_algebra_permanent.md` §7.1) and
/// the D1b §4 conformance walk-through. The choice makes per-lane
/// cross-checks 1:1 with no resampling logic.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedField, ScalarPackedFp3};
/// use gf2_core::gfp::Fp;
///
/// let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
/// let b = a.add(a); // 2 + 2 = 4 mod 3 = 1
/// for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
///     assert_eq!(b.lane(i), Fp::<3>::new(1));
/// }
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ScalarPackedFp3 {
    lanes: [Fp<3>; 64],
}

impl fmt::Debug for ScalarPackedFp3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print lanes in canonical-decode form (the `value()` of each
        // `Fp<3>`) so the output is independent of the underlying
        // Montgomery / canonical storage choice for `Fp<3>`. This keeps
        // `assert_eq!` panic messages stable and human-readable, which
        // matters because this type is the cross-check oracle and
        // mismatches between optimised impls and the oracle are the
        // primary debug surface.
        f.debug_struct("ScalarPackedFp3")
            .field(
                "lanes",
                &core::array::from_fn::<u64, 64, _>(|i| self.lanes[i].value()),
            )
            .finish()
    }
}

impl PackedField<Fp<3>> for ScalarPackedFp3 {
    const LANES: usize = 64;

    fn zero() -> Self {
        Self {
            lanes: [Fp::<3>::new(0); 64],
        }
    }

    fn one() -> Self {
        Self {
            lanes: [Fp::<3>::new(1); 64],
        }
    }

    fn splat(x: Fp<3>) -> Self {
        Self { lanes: [x; 64] }
    }

    fn add(self, rhs: Self) -> Self {
        Self {
            lanes: core::array::from_fn(|i| self.lanes[i] + rhs.lanes[i]),
        }
    }

    fn sub(self, rhs: Self) -> Self {
        Self {
            lanes: core::array::from_fn(|i| self.lanes[i] - rhs.lanes[i]),
        }
    }

    fn neg(self) -> Self {
        Self {
            lanes: core::array::from_fn(|i| -self.lanes[i]),
        }
    }

    fn mul(self, rhs: Self) -> Self {
        Self {
            lanes: core::array::from_fn(|i| self.lanes[i] * rhs.lanes[i]),
        }
    }

    fn lane(self, i: usize) -> Fp<3> {
        assert!(
            i < Self::LANES,
            "ScalarPackedFp3::lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        self.lanes[i]
    }

    fn with_lane(self, i: usize, x: Fp<3>) -> Self {
        assert!(
            i < Self::LANES,
            "ScalarPackedFp3::with_lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let mut out = self.lanes;
        out[i] = x;
        Self { lanes: out }
    }

    fn all_zero(self) -> bool {
        // `Fp::<3>` is `Eq`, so this is canonical-decode equality
        // automatically: every codeword is canonical for `Fp<3>` (no
        // alt-zero redundancy at the scalar level). This satisfies
        // D1b §3.5 trivially.
        self.lanes.iter().all(|&x| x == Fp::<3>::new(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy: a single `Fp<3>` element drawn from `{0, 1, 2}`.
    fn fp3_strat() -> impl Strategy<Value = Fp<3>> {
        (0u64..3).prop_map(Fp::<3>::new)
    }

    /// Strategy: a `ScalarPackedFp3` with every lane independently
    /// drawn from `{0, 1, 2}`.
    fn packed_strat() -> impl Strategy<Value = ScalarPackedFp3> {
        prop::collection::vec(fp3_strat(), 64).prop_map(|v| {
            let mut p = ScalarPackedFp3::zero();
            for (i, x) in v.into_iter().enumerate() {
                p = p.with_lane(i, x);
            }
            p
        })
    }

    // ----------------------------------------------------------------
    // Constants and lane round-trip
    // ----------------------------------------------------------------

    #[test]
    fn test_zero_all_zero() {
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        assert!(z.all_zero());
        for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
            assert_eq!(z.lane(i), Fp::<3>::new(0));
        }
    }

    #[test]
    fn test_one_splat_with_lane_lane_roundtrip() {
        // one(): every lane decodes to 1.
        let o = <ScalarPackedFp3 as PackedField<Fp<3>>>::one();
        for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
            assert_eq!(o.lane(i), Fp::<3>::new(1));
        }
        assert!(!o.all_zero());

        // splat(2): every lane decodes to 2.
        let two = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
            assert_eq!(two.lane(i), Fp::<3>::new(2));
        }

        // with_lane / lane round-trip at word-boundary indices.
        // CLAUDE.md §Testing requires {0, 1, 63, 64, 65}; for a
        // single 64-lane oracle we cover the in-range subset
        // {0, 1, 16, 31, 32, 63} and exercise out-of-range (64, 65)
        // in `test_lane_panics_out_of_range_*`. See test docstrings
        // for the rationale on the substitution.
        let mut v = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for &i in &[0usize, 1, 16, 31, 32, 63] {
            v = v.with_lane(i, Fp::<3>::new(2));
            assert_eq!(v.lane(i), Fp::<3>::new(2));
        }
        // Lanes that were not written remain zero.
        for i in 0..64 {
            if ![0usize, 1, 16, 31, 32, 63].contains(&i) {
                assert_eq!(v.lane(i), Fp::<3>::new(0));
            }
        }
    }

    // ----------------------------------------------------------------
    // Add / sub / mul / neg deterministic checks
    // ----------------------------------------------------------------

    #[test]
    fn test_add_commutative() {
        // 1 non-zero lane.
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(7, Fp::<3>::new(1));
        let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(7, Fp::<3>::new(2));
        assert_eq!(a.add(b), b.add(a));

        // 16 non-zero lanes (every fourth lane).
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let mut b = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..16 {
            a = a.with_lane(i * 4, Fp::<3>::new(1));
            b = b.with_lane(i * 4, Fp::<3>::new(2));
        }
        assert_eq!(a.add(b), b.add(a));

        // 63 non-zero lanes (skip lane 31).
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
        let mut b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        a = a.with_lane(31, Fp::<3>::new(0));
        b = b.with_lane(31, Fp::<3>::new(0));
        assert_eq!(a.add(b), b.add(a));

        // 64 non-zero lanes (all of them).
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
        let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        assert_eq!(a.add(b), b.add(a));
        // 1 + 2 = 0 mod 3: every lane of the sum decodes to zero.
        assert!(a.add(b).all_zero());
    }

    #[test]
    fn test_sub_self_is_zero() {
        // 1 non-zero lane.
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(0, Fp::<3>::new(2));
        assert!(a.sub(a).all_zero());

        // 16 non-zero lanes.
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..16 {
            a = a.with_lane(i * 4, Fp::<3>::new((i as u64) % 3));
        }
        assert!(a.sub(a).all_zero());

        // 63 non-zero lanes.
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        a = a.with_lane(31, Fp::<3>::new(0));
        assert!(a.sub(a).all_zero());

        // 64 non-zero lanes.
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        assert!(a.sub(a).all_zero());
    }

    #[test]
    fn test_mul_zero_absorbs() {
        // Multiplying anything by zero produces all-zero.
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let one = <ScalarPackedFp3 as PackedField<Fp<3>>>::one();
        let two = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));

        assert!(z.mul(z).all_zero());
        assert!(z.mul(one).all_zero());
        assert!(one.mul(z).all_zero());
        assert!(z.mul(two).all_zero());
        assert!(two.mul(z).all_zero());

        // Mixed: a vector with 16 zeros and 48 twos, multiplied by a
        // vector with the complement pattern, yields all-zero.
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        let mut b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        for i in 0..16 {
            a = a.with_lane(i, Fp::<3>::new(0));
        }
        for i in 16..64 {
            b = b.with_lane(i, Fp::<3>::new(0));
        }
        assert!(a.mul(b).all_zero());
    }

    #[test]
    fn test_neg_double_is_identity() {
        // 1 non-zero lane.
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(0, Fp::<3>::new(2));
        assert_eq!(a.neg().neg(), a);

        // 16 non-zero lanes.
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..16 {
            a = a.with_lane(i * 4, Fp::<3>::new(((i as u64) % 2) + 1));
        }
        assert_eq!(a.neg().neg(), a);

        // 63 non-zero lanes.
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        a = a.with_lane(31, Fp::<3>::new(0));
        assert_eq!(a.neg().neg(), a);

        // 64 non-zero lanes.
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        assert_eq!(a.neg().neg(), a);
    }

    // ----------------------------------------------------------------
    // Word-boundary tests on lane indices
    // ----------------------------------------------------------------

    #[test]
    fn test_with_lane_word_boundary_indices() {
        // CLAUDE.md §Testing requires word-boundary cases at
        // {0, 1, 63, 64, 65}. The 64-lane oracle's in-range slice is
        // {0, 1, 16, 31, 32, 63}; 64 and 65 are out-of-range and the
        // panic behaviour is verified in
        // `test_lane_panics_out_of_range_*`. See the issue spec
        // section "Concrete deliverables" item 3 for the rationale.
        for &i in &[0usize, 1, 16, 31, 32, 63] {
            let v = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
            let v = v.with_lane(i, Fp::<3>::new(2));
            assert_eq!(v.lane(i), Fp::<3>::new(2));
            // Round-trip preserves bit-for-bit equality.
            let v2 = v.with_lane(i, v.lane(i));
            assert_eq!(v, v2);
        }
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_64() {
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let _ = z.lane(64);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_65() {
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let _ = z.lane(65);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_64() {
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let _ = z.with_lane(64, Fp::<3>::new(1));
    }

    // ----------------------------------------------------------------
    // Full lane round-trip
    // ----------------------------------------------------------------

    #[test]
    fn test_full_lane_round_trip() {
        // For any value a, with_lane(i, lane(i)) should recover a
        // bit-for-bit at every i.
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..64 {
            a = a.with_lane(i, Fp::<3>::new((i as u64) % 3));
        }
        for i in 0..64 {
            let a2 = a.with_lane(i, a.lane(i));
            assert_eq!(a, a2);
        }
    }

    // ----------------------------------------------------------------
    // Property tests (1000 cases each per issue spec)
    // ----------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        /// Add commutativity: `a + b == b + a` for all `a`, `b`.
        #[test]
        fn test_add_commutativity_proptest(a in packed_strat(), b in packed_strat()) {
            prop_assert_eq!(a.add(b), b.add(a));
        }

        /// Sub undoes add: `(a + b) - b == a` for all `a`, `b`.
        #[test]
        fn test_sub_undoes_add_proptest(a in packed_strat(), b in packed_strat()) {
            prop_assert_eq!(a.add(b).sub(b), a);
        }

        /// Mul distributes over add: `a * (b + c) == a*b + a*c`.
        #[test]
        fn test_mul_distributivity_proptest(
            a in packed_strat(),
            b in packed_strat(),
            c in packed_strat(),
        ) {
            let lhs = a.mul(b.add(c));
            let rhs = a.mul(b).add(a.mul(c));
            prop_assert_eq!(lhs, rhs);
        }

        /// Negation is involutive: `-(-a) == a` for all `a`.
        #[test]
        fn test_neg_involution_proptest(a in packed_strat()) {
            prop_assert_eq!(a.neg().neg(), a);
        }
    }
}
