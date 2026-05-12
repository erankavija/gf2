//! Scalar reference implementations of [`PackedField<Fp<3>>`] and
//! [`PackedFieldVec<Fp<3>>`].
//!
//! [`ScalarPackedFp3`] is the F_3 *correctness oracle* against which the
//! optimised `Bipedal3` impl is cross-checked. (F_5 and F_7 packed types
//! `Packed5` / `Packed7` use their own scalar `Fp<5>` / `Fp<7>` oracles
//! per-lane rather than going through `ScalarPackedFp3`, which is F_3-
//! specific by name and trait signature.) The implementation is intentionally
//! one-`Fp<3>`-per-lane: no SIMD, no bit-packing, no popcount tricks.
//! Every method is the literal lane-wise composition of the underlying
//! `Fp<3>` operator.
//!
//! [`ScalarPackedFp3Vec`] is the matching variable-length oracle for
//! [`PackedFieldVec<Fp<3>>`]: a `Vec<Fp<3>>` with one `Fp<3>` per
//! logical position. It is used by W1-T3's `Bipedal3Vec` cross-check
//! tests in the same way `ScalarPackedFp3` is used for the fixed-width
//! `Bipedal3` element. Both types satisfy the literal-element-wise
//! semantics of the trait, which is why they are useful as oracles.
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

use super::{PackedField, PackedFieldVec};

/// Scalar reference implementation of [`PackedField<Fp<3>>`] over a
/// fixed `LANES = 64` array of `Fp<3>` elements.
///
/// One `Fp<3>` per lane. No bit-packing, no SIMD, no encoding tricks.
/// This is the F_3 correctness oracle for the optimised `Bipedal3`
/// impl; it is cross-checked via per-lane equality. F_5 / F_7 impls
/// (`Packed5` / `Packed7`) are F_3-incompatible by type and cross-
/// check against their own scalar `Fp<5>` / `Fp<7>` per-lane oracles,
/// not against this type.
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

/// Scalar reference implementation of [`PackedFieldVec<Fp<3>>`] over
/// a `Vec<Fp<3>>` with one `Fp<3>` per logical position.
///
/// This is the variable-length companion of [`ScalarPackedFp3`]:
/// where the fixed-width oracle anchors `PackedField` correctness for
/// SIMD-batched packed types, this variable-length oracle anchors
/// `PackedFieldVec` correctness for sequence-shaped impls such as the
/// future `Bipedal3Vec` (W1-T3). The storage is the simplest possible
/// representation — no bit-packing, no SIMD, no chunking — so that
/// cross-check tests can route the same input through both impls and
/// compare element-by-element via [`PackedFieldVec::get`].
///
/// `Self::Element` is set to [`ScalarPackedFp3`] purely to satisfy
/// the trait's `type Element: PackedField<Fp<3>>` bound; the storage
/// is `Vec<Fp<3>>` directly and never materialises an `Element`
/// internally. Optimised impls (e.g. `Bipedal3Vec`) will store
/// `Vec<Bipedal3>` chunks and use the associated type seriously.
///
/// # Examples
///
/// ```
/// use gf2_algebra::packed::{PackedFieldVec, ScalarPackedFp3Vec};
/// use gf2_core::gfp::Fp;
///
/// let xs = [Fp::<3>::new(1), Fp::<3>::new(2), Fp::<3>::new(0)];
/// let v = ScalarPackedFp3Vec::from_field_slice(&xs);
/// assert_eq!(v.len(), 3);
/// for i in 0..3 {
///     assert_eq!(v.get(i), xs[i]);
/// }
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScalarPackedFp3Vec {
    elements: Vec<Fp<3>>,
}

impl PackedFieldVec<Fp<3>> for ScalarPackedFp3Vec {
    type Element = ScalarPackedFp3;

    fn zeros(len: usize) -> Self {
        Self {
            elements: vec![Fp::<3>::new(0); len],
        }
    }

    fn from_field_slice(xs: &[Fp<3>]) -> Self {
        Self {
            elements: xs.to_vec(),
        }
    }

    fn len(&self) -> usize {
        self.elements.len()
    }

    fn get(&self, i: usize) -> Fp<3> {
        assert!(
            i < self.elements.len(),
            "ScalarPackedFp3Vec::get: index {} out of range (len = {})",
            i,
            self.elements.len()
        );
        self.elements[i]
    }

    fn add_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.elements.len(),
            rhs.elements.len(),
            "ScalarPackedFp3Vec::add_assign: length mismatch ({} vs {})",
            self.elements.len(),
            rhs.elements.len()
        );
        for (lhs, &r) in self.elements.iter_mut().zip(rhs.elements.iter()) {
            *lhs += r;
        }
    }

    fn sub_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.elements.len(),
            rhs.elements.len(),
            "ScalarPackedFp3Vec::sub_assign: length mismatch ({} vs {})",
            self.elements.len(),
            rhs.elements.len()
        );
        for (lhs, &r) in self.elements.iter_mut().zip(rhs.elements.iter()) {
            *lhs = *lhs - r;
        }
    }

    fn mul_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.elements.len(),
            rhs.elements.len(),
            "ScalarPackedFp3Vec::mul_assign: length mismatch ({} vs {})",
            self.elements.len(),
            rhs.elements.len()
        );
        for (lhs, &r) in self.elements.iter_mut().zip(rhs.elements.iter()) {
            *lhs = *lhs * r;
        }
    }

    fn all_zero(&self) -> bool {
        // `Fp::<3>` is `Eq` and canonical, so this is canonical-decode
        // equality (D1b §3.5 trivially). Empty vectors answer `true`
        // because `Iterator::all` on an empty iterator returns `true`,
        // matching the documented contract on `PackedFieldVec::all_zero`.
        self.elements.iter().all(|&x| x == Fp::<3>::new(0))
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

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_65() {
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let _ = z.with_lane(65, Fp::<3>::new(1));
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

#[cfg(test)]
mod vec_tests {
    //! `ScalarPackedFp3Vec` test surface covering the issue criterion's
    //! explicit `{1, 16, 63, 64, 65}`-element lengths.
    //!
    //! The fixed-width [`super::ScalarPackedFp3`] caps at 64 lanes, so
    //! the literal "65 elements" requirement of the success criterion
    //! is covered by the variable-length [`super::ScalarPackedFp3Vec`]
    //! per the user resolution recorded in the rework dispatch (Option
    //! C, 2026-05-09): the criterion targets `PackedFieldVec` length
    //! semantics, where 65 elements is naturally representable.
    use super::*;
    use proptest::prelude::*;

    /// Strategy: a single `Fp<3>` element drawn from `{0, 1, 2}`.
    fn fp3_strat() -> impl Strategy<Value = Fp<3>> {
        (0u64..3).prop_map(Fp::<3>::new)
    }

    /// The five lengths required by the issue criterion's
    /// `{1, 16, 63, 64, 65}` boundary set.
    const REQUIRED_LENGTHS: &[usize] = &[1, 16, 63, 64, 65];

    /// Build a vector of the given length whose i-th element is
    /// `Fp::<3>::new((i as u64) % 3)`. Deterministic so tests can
    /// recompute expected values.
    fn deterministic_vec(len: usize) -> ScalarPackedFp3Vec {
        let xs: Vec<Fp<3>> = (0..len).map(|i| Fp::<3>::new((i as u64) % 3)).collect();
        ScalarPackedFp3Vec::from_field_slice(&xs)
    }

    /// In-place set of position `i` to value `x`. Convenience helper
    /// because the trait does not expose a public mutator analogue of
    /// `with_lane`; we use `add_assign` with a one-position delta.
    fn set_position(v: &mut ScalarPackedFp3Vec, i: usize, x: Fp<3>) {
        let cur = v.get(i);
        let mut delta = ScalarPackedFp3Vec::zeros(v.len());
        // Need: cur + d == x, so d = x - cur.
        delta.elements[i] = x - cur;
        v.add_assign(&delta);
    }

    // ---------------------------------------------------------------
    // Constructors and basic accessors
    // ---------------------------------------------------------------

    #[test]
    fn test_zeros_then_get_returns_zero_at_each_required_length() {
        // Zero-length and the criterion's five required lengths.
        for &len in &[0, 1, 16, 63, 64, 65] {
            let v = ScalarPackedFp3Vec::zeros(len);
            assert_eq!(v.len(), len);
            for i in 0..len {
                assert_eq!(v.get(i), Fp::<3>::new(0), "len = {}, i = {}", len, i);
            }
        }
    }

    #[test]
    fn test_from_field_slice_then_get_returns_input_at_each_required_length() {
        for &len in REQUIRED_LENGTHS {
            let xs: Vec<Fp<3>> = (0..len).map(|i| Fp::<3>::new((i as u64) % 3)).collect();
            let v = ScalarPackedFp3Vec::from_field_slice(&xs);
            assert_eq!(v.len(), len);
            for (i, &expected) in xs.iter().enumerate() {
                assert_eq!(v.get(i), expected, "len = {}, i = {}", len, i);
            }
        }
    }

    #[test]
    fn test_is_empty_on_zero_length() {
        let v = ScalarPackedFp3Vec::zeros(0);
        assert!(v.is_empty());
        assert_eq!(v.len(), 0);
    }

    #[test]
    fn test_is_empty_default_impl_calls_len() {
        // Non-empty constructions must not be empty. The trait's
        // default `is_empty` is `self.len() == 0`; this asserts that
        // the default does not get accidentally overridden.
        for &len in REQUIRED_LENGTHS {
            let v = ScalarPackedFp3Vec::zeros(len);
            assert!(!v.is_empty(), "len = {}", len);
            assert_eq!(v.is_empty(), v.len() == 0);
        }
    }

    // ---------------------------------------------------------------
    // add_assign at each required length
    // ---------------------------------------------------------------

    #[test]
    fn test_add_at_len_1() {
        let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
        let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
        a.add_assign(&b);
        assert_eq!(a.get(0), Fp::<3>::new(0)); // 1 + 2 == 0 mod 3
    }

    #[test]
    fn test_add_at_len_16() {
        let mut a = deterministic_vec(16);
        let b = deterministic_vec(16);
        a.add_assign(&b);
        for i in 0..16 {
            let expected = Fp::<3>::new((2 * (i as u64)) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    #[test]
    fn test_add_at_len_63() {
        let mut a = deterministic_vec(63);
        let b = deterministic_vec(63);
        a.add_assign(&b);
        for i in 0..63 {
            let expected = Fp::<3>::new((2 * (i as u64)) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    #[test]
    fn test_add_at_len_64() {
        let mut a = deterministic_vec(64);
        let b = deterministic_vec(64);
        a.add_assign(&b);
        for i in 0..64 {
            let expected = Fp::<3>::new((2 * (i as u64)) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    #[test]
    fn test_add_at_len_65() {
        let mut a = deterministic_vec(65);
        let b = deterministic_vec(65);
        a.add_assign(&b);
        for i in 0..65 {
            let expected = Fp::<3>::new((2 * (i as u64)) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    // ---------------------------------------------------------------
    // sub_assign at each required length
    // ---------------------------------------------------------------

    #[test]
    fn test_sub_at_len_1() {
        let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(0)]);
        let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
        a.sub_assign(&b);
        assert_eq!(a.get(0), Fp::<3>::new(2)); // 0 - 1 == 2 mod 3
    }

    #[test]
    fn test_sub_at_len_16() {
        let mut a = deterministic_vec(16);
        let b = a.clone();
        a.sub_assign(&b);
        assert!(a.all_zero());
    }

    #[test]
    fn test_sub_at_len_63() {
        let mut a = deterministic_vec(63);
        let b = a.clone();
        a.sub_assign(&b);
        assert!(a.all_zero());
    }

    #[test]
    fn test_sub_at_len_64() {
        let mut a = deterministic_vec(64);
        let b = a.clone();
        a.sub_assign(&b);
        assert!(a.all_zero());
    }

    #[test]
    fn test_sub_at_len_65() {
        let mut a = deterministic_vec(65);
        let b = a.clone();
        a.sub_assign(&b);
        assert!(a.all_zero());
    }

    // ---------------------------------------------------------------
    // mul_assign at each required length
    // ---------------------------------------------------------------

    #[test]
    fn test_mul_at_len_1() {
        let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
        let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
        a.mul_assign(&b);
        assert_eq!(a.get(0), Fp::<3>::new(1)); // 2 * 2 == 1 mod 3
    }

    #[test]
    fn test_mul_at_len_16() {
        let mut a = deterministic_vec(16);
        let b = ScalarPackedFp3Vec::zeros(16);
        a.mul_assign(&b);
        assert!(a.all_zero());
    }

    #[test]
    fn test_mul_at_len_63() {
        let mut a = deterministic_vec(63);
        let b = deterministic_vec(63);
        a.mul_assign(&b);
        for i in 0..63 {
            let v = (i as u64) % 3;
            let expected = Fp::<3>::new((v * v) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    #[test]
    fn test_mul_at_len_64() {
        let mut a = deterministic_vec(64);
        let b = deterministic_vec(64);
        a.mul_assign(&b);
        for i in 0..64 {
            let v = (i as u64) % 3;
            let expected = Fp::<3>::new((v * v) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    #[test]
    fn test_mul_at_len_65() {
        let mut a = deterministic_vec(65);
        let b = deterministic_vec(65);
        a.mul_assign(&b);
        for i in 0..65 {
            let v = (i as u64) % 3;
            let expected = Fp::<3>::new((v * v) % 3);
            assert_eq!(a.get(i), expected, "i = {}", i);
        }
    }

    // ---------------------------------------------------------------
    // neg via sub-from-zero at each required length
    //
    // `PackedFieldVec` does not expose `neg`, but the issue criterion
    // names `neg` alongside the vec ops. The user-resolution dispatch
    // says: derive neg as `0 - self` and verify it matches per-element
    // `Fp<3>::neg`. This covers the spirit of the criterion at lengths
    // {1, 16, 63, 64, 65}.
    // ---------------------------------------------------------------

    #[test]
    fn test_neg_via_zero_minus_self_at_len_1() {
        let v = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
        let mut zero = ScalarPackedFp3Vec::zeros(1);
        zero.sub_assign(&v);
        // -1 mod 3 == 2
        assert_eq!(zero.get(0), Fp::<3>::new(2));
        assert_eq!(zero.get(0), -Fp::<3>::new(1));
    }

    #[test]
    fn test_neg_via_zero_minus_self_at_each_required_length() {
        for &len in &[1usize, 16, 63, 64, 65] {
            let v = deterministic_vec(len);
            let mut zero = ScalarPackedFp3Vec::zeros(len);
            zero.sub_assign(&v);
            // Expected: per-element -Fp<3>::new(i % 3).
            for i in 0..len {
                let expected = -Fp::<3>::new((i as u64) % 3);
                assert_eq!(zero.get(i), expected, "len = {}, i = {}", len, i);
            }
        }
    }

    // ---------------------------------------------------------------
    // splat (constructed via from_field_slice with a constant slice)
    // and "with_lane" (covered by set_position helper using the
    // add_assign-by-delta technique). The criterion lists splat and
    // with_lane on the trait; PackedFieldVec exposes neither directly,
    // but both are exercisable through the public surface and we
    // cover them at every required length.
    // ---------------------------------------------------------------

    #[test]
    fn test_splat_via_from_field_slice_at_each_required_length() {
        for &len in REQUIRED_LENGTHS {
            let xs: Vec<Fp<3>> = vec![Fp::<3>::new(2); len];
            let v = ScalarPackedFp3Vec::from_field_slice(&xs);
            assert_eq!(v.len(), len);
            for i in 0..len {
                assert_eq!(v.get(i), Fp::<3>::new(2), "len = {}, i = {}", len, i);
            }
        }
    }

    #[test]
    fn test_with_lane_via_add_assign_at_each_required_length() {
        // The criterion includes `with_lane` at lengths {1,16,63,64,65}.
        // PackedFieldVec doesn't expose `with_lane` directly, but the
        // same effect is built from the public surface using
        // add-by-delta. We verify the round-trip at each required
        // length and at the boundary positions inside that length.
        for &len in REQUIRED_LENGTHS {
            let mut v = ScalarPackedFp3Vec::zeros(len);
            // Hit a representative set of positions: first, last, and
            // (for len >= 16) a couple of interior boundaries.
            let mut probes: Vec<usize> = vec![0, len - 1];
            if len >= 16 {
                probes.push(len / 2);
                probes.push(len - 2);
            }
            probes.sort();
            probes.dedup();
            for &i in &probes {
                set_position(&mut v, i, Fp::<3>::new(2));
                assert_eq!(v.get(i), Fp::<3>::new(2), "len = {}, i = {}", len, i);
            }
        }
    }

    // ---------------------------------------------------------------
    // round-trip (lane / get round-trip equivalent at vec scale)
    // ---------------------------------------------------------------

    #[test]
    fn test_round_trip_from_field_slice_to_get_at_each_required_length() {
        for &len in REQUIRED_LENGTHS {
            let xs: Vec<Fp<3>> = (0..len)
                .map(|i| Fp::<3>::new((i as u64).wrapping_mul(7) % 3))
                .collect();
            let v = ScalarPackedFp3Vec::from_field_slice(&xs);
            for (i, &expected) in xs.iter().enumerate() {
                assert_eq!(v.get(i), expected, "len = {}, i = {}", len, i);
            }
        }
    }

    // ---------------------------------------------------------------
    // all_zero at each required length
    // ---------------------------------------------------------------

    #[test]
    fn test_all_zero_on_zeros_constructor_at_each_required_length() {
        // Zero-length included for completeness — empty vec is all-zero.
        for &len in &[0usize, 1, 16, 63, 64, 65] {
            let v = ScalarPackedFp3Vec::zeros(len);
            assert!(v.all_zero(), "len = {}", len);
        }
    }

    #[test]
    fn test_all_zero_false_after_setting_one_position_nonzero_at_each_required_length() {
        for &len in REQUIRED_LENGTHS {
            // For every required length, set a different boundary
            // position to a non-zero value and assert all_zero is false.
            let mut v = ScalarPackedFp3Vec::zeros(len);
            set_position(&mut v, len - 1, Fp::<3>::new(1));
            assert!(!v.all_zero(), "len = {}", len);
        }
    }

    // ---------------------------------------------------------------
    // length-mismatch panics
    // ---------------------------------------------------------------

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_add_assign_mismatched_lengths_panics() {
        let mut a = ScalarPackedFp3Vec::zeros(64);
        let b = ScalarPackedFp3Vec::zeros(65);
        a.add_assign(&b);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_sub_assign_mismatched_lengths_panics() {
        let mut a = ScalarPackedFp3Vec::zeros(64);
        let b = ScalarPackedFp3Vec::zeros(65);
        a.sub_assign(&b);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_mul_assign_mismatched_lengths_panics() {
        let mut a = ScalarPackedFp3Vec::zeros(64);
        let b = ScalarPackedFp3Vec::zeros(65);
        a.mul_assign(&b);
    }

    // ---------------------------------------------------------------
    // get out-of-range panic at the 65-boundary
    // ---------------------------------------------------------------

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_out_of_range_at_len_65_panics() {
        let v = ScalarPackedFp3Vec::zeros(65);
        let _ = v.get(65);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_out_of_range_at_len_64_panics() {
        let v = ScalarPackedFp3Vec::zeros(64);
        let _ = v.get(64);
    }

    // ---------------------------------------------------------------
    // proptest cross-check: arithmetic distributes / inverts
    // exactly like per-element Fp<3>, at the criterion lengths
    // ---------------------------------------------------------------

    fn vec_strat(len: usize) -> impl Strategy<Value = ScalarPackedFp3Vec> {
        prop::collection::vec(fp3_strat(), len)
            .prop_map(|xs| ScalarPackedFp3Vec::from_field_slice(&xs))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn test_add_commutativity_proptest_at_len_65(
            a in vec_strat(65),
            b in vec_strat(65),
        ) {
            let mut lhs = a.clone();
            lhs.add_assign(&b);
            let mut rhs = b.clone();
            rhs.add_assign(&a);
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn test_sub_undoes_add_proptest_at_len_65(
            a in vec_strat(65),
            b in vec_strat(65),
        ) {
            let mut work = a.clone();
            work.add_assign(&b);
            work.sub_assign(&b);
            prop_assert_eq!(work, a);
        }

        #[test]
        fn test_mul_distributivity_proptest_at_len_65(
            a in vec_strat(65),
            b in vec_strat(65),
            c in vec_strat(65),
        ) {
            // lhs = a * (b + c)
            let mut bc = b.clone();
            bc.add_assign(&c);
            let mut lhs = a.clone();
            lhs.mul_assign(&bc);
            // rhs = a*b + a*c
            let mut ab = a.clone();
            ab.mul_assign(&b);
            let mut ac = a.clone();
            ac.mul_assign(&c);
            ab.add_assign(&ac);
            prop_assert_eq!(lhs, ab);
        }
    }
}
