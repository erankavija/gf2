//! Scalar reference implementations of [`PackedField<Fp<3>>`] and
//! [`PackedFieldVec<Fp<3>>`]: one `Fp<3>` per lane, used as the F_3
//! correctness oracle for `Bipedal3` and [`super::Bipedal3Vec`].

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

/// Scalar reference implementation of [`PackedField<Fp<3>>`]: one `Fp<3>`
/// per lane, with `LANES = 64` to match `Bipedal3` lane for lane.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ScalarPackedFp3 {
    lanes: [Fp<3>; 64],
}

impl fmt::Debug for ScalarPackedFp3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print canonical `value()`s so the output is independent of
        // `Fp<3>`'s storage form.
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
        self.lanes.iter().all(|&x| x == Fp::<3>::new(0))
    }
}

/// Scalar reference implementation of [`PackedFieldVec<Fp<3>>`]: a
/// `Vec<Fp<3>>` with one element per logical position.
///
/// `Self::Element` is [`ScalarPackedFp3`] only to satisfy the trait bound;
/// the storage never materialises an `Element`.
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
        self.elements.iter().all(|&x| x == Fp::<3>::new(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn fp3_strat() -> impl Strategy<Value = Fp<3>> {
        (0u64..3).prop_map(Fp::<3>::new)
    }

    fn packed_strat() -> impl Strategy<Value = ScalarPackedFp3> {
        prop::collection::vec(fp3_strat(), 64).prop_map(|v| {
            let mut p = ScalarPackedFp3::zero();
            for (i, x) in v.into_iter().enumerate() {
                p = p.with_lane(i, x);
            }
            p
        })
    }

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
        let o = <ScalarPackedFp3 as PackedField<Fp<3>>>::one();
        for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
            assert_eq!(o.lane(i), Fp::<3>::new(1));
        }
        assert!(!o.all_zero());

        let two = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        for i in 0..<ScalarPackedFp3 as PackedField<Fp<3>>>::LANES {
            assert_eq!(two.lane(i), Fp::<3>::new(2));
        }

        // In-range word-boundary indices; 64 and 65 are covered by
        // `test_lane_panics_out_of_range_*`.
        let mut v = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for &i in &[0usize, 1, 16, 31, 32, 63] {
            v = v.with_lane(i, Fp::<3>::new(2));
            assert_eq!(v.lane(i), Fp::<3>::new(2));
        }
        for i in 0..64 {
            if ![0usize, 1, 16, 31, 32, 63].contains(&i) {
                assert_eq!(v.lane(i), Fp::<3>::new(0));
            }
        }
    }

    #[test]
    fn test_add_commutative() {
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(7, Fp::<3>::new(1));
        let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(7, Fp::<3>::new(2));
        assert_eq!(a.add(b), b.add(a));

        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let mut b = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..16 {
            a = a.with_lane(i * 4, Fp::<3>::new(1));
            b = b.with_lane(i * 4, Fp::<3>::new(2));
        }
        assert_eq!(a.add(b), b.add(a));

        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
        let mut b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        a = a.with_lane(31, Fp::<3>::new(0));
        b = b.with_lane(31, Fp::<3>::new(0));
        assert_eq!(a.add(b), b.add(a));

        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(1));
        let b = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        assert_eq!(a.add(b), b.add(a));
        assert!(a.add(b).all_zero());
    }

    #[test]
    fn test_sub_self_is_zero() {
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(0, Fp::<3>::new(2));
        assert!(a.sub(a).all_zero());

        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..16 {
            a = a.with_lane(i * 4, Fp::<3>::new((i as u64) % 3));
        }
        assert!(a.sub(a).all_zero());

        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        a = a.with_lane(31, Fp::<3>::new(0));
        assert!(a.sub(a).all_zero());

        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        assert!(a.sub(a).all_zero());
    }

    #[test]
    fn test_mul_zero_absorbs() {
        let z = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        let one = <ScalarPackedFp3 as PackedField<Fp<3>>>::one();
        let two = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));

        assert!(z.mul(z).all_zero());
        assert!(z.mul(one).all_zero());
        assert!(one.mul(z).all_zero());
        assert!(z.mul(two).all_zero());
        assert!(two.mul(z).all_zero());

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
        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero().with_lane(0, Fp::<3>::new(2));
        assert_eq!(a.neg().neg(), a);

        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..16 {
            a = a.with_lane(i * 4, Fp::<3>::new(((i as u64) % 2) + 1));
        }
        assert_eq!(a.neg().neg(), a);

        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        a = a.with_lane(31, Fp::<3>::new(0));
        assert_eq!(a.neg().neg(), a);

        let a = <ScalarPackedFp3 as PackedField<Fp<3>>>::splat(Fp::<3>::new(2));
        assert_eq!(a.neg().neg(), a);
    }

    #[test]
    fn test_with_lane_word_boundary_indices() {
        // In-range word-boundary indices; 64 and 65 are covered by
        // `test_lane_panics_out_of_range_*`.
        for &i in &[0usize, 1, 16, 31, 32, 63] {
            let v = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
            let v = v.with_lane(i, Fp::<3>::new(2));
            assert_eq!(v.lane(i), Fp::<3>::new(2));
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

    #[test]
    fn test_full_lane_round_trip() {
        let mut a = <ScalarPackedFp3 as PackedField<Fp<3>>>::zero();
        for i in 0..64 {
            a = a.with_lane(i, Fp::<3>::new((i as u64) % 3));
        }
        for i in 0..64 {
            let a2 = a.with_lane(i, a.lane(i));
            assert_eq!(a, a2);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        #[test]
        fn test_add_commutativity_proptest(a in packed_strat(), b in packed_strat()) {
            prop_assert_eq!(a.add(b), b.add(a));
        }

        #[test]
        fn test_sub_undoes_add_proptest(a in packed_strat(), b in packed_strat()) {
            prop_assert_eq!(a.add(b).sub(b), a);
        }

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

        #[test]
        fn test_neg_involution_proptest(a in packed_strat()) {
            prop_assert_eq!(a.neg().neg(), a);
        }
    }
}

#[cfg(test)]
mod vec_tests {
    use super::*;
    use proptest::prelude::*;

    fn fp3_strat() -> impl Strategy<Value = Fp<3>> {
        (0u64..3).prop_map(Fp::<3>::new)
    }

    /// Lengths around the 64-element word boundary.
    const REQUIRED_LENGTHS: &[usize] = &[1, 16, 63, 64, 65];

    fn deterministic_vec(len: usize) -> ScalarPackedFp3Vec {
        let xs: Vec<Fp<3>> = (0..len).map(|i| Fp::<3>::new((i as u64) % 3)).collect();
        ScalarPackedFp3Vec::from_field_slice(&xs)
    }

    /// Sets position `i` to `x` by adding a one-position delta; the trait
    /// has no per-position mutator.
    fn set_position(v: &mut ScalarPackedFp3Vec, i: usize, x: Fp<3>) {
        let cur = v.get(i);
        let mut delta = ScalarPackedFp3Vec::zeros(v.len());
        delta.elements[i] = x - cur;
        v.add_assign(&delta);
    }

    #[test]
    fn test_zeros_then_get_returns_zero_at_each_required_length() {
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
        for &len in REQUIRED_LENGTHS {
            let v = ScalarPackedFp3Vec::zeros(len);
            assert!(!v.is_empty(), "len = {}", len);
            assert_eq!(v.is_empty(), v.len() == 0);
        }
    }

    #[test]
    fn test_add_at_len_1() {
        let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
        let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
        a.add_assign(&b);
        assert_eq!(a.get(0), Fp::<3>::new(0));
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

    #[test]
    fn test_sub_at_len_1() {
        let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(0)]);
        let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
        a.sub_assign(&b);
        assert_eq!(a.get(0), Fp::<3>::new(2));
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

    #[test]
    fn test_mul_at_len_1() {
        let mut a = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
        let b = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(2)]);
        a.mul_assign(&b);
        assert_eq!(a.get(0), Fp::<3>::new(1));
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

    #[test]
    fn test_neg_via_zero_minus_self_at_len_1() {
        let v = ScalarPackedFp3Vec::from_field_slice(&[Fp::<3>::new(1)]);
        let mut zero = ScalarPackedFp3Vec::zeros(1);
        zero.sub_assign(&v);
        assert_eq!(zero.get(0), Fp::<3>::new(2));
        assert_eq!(zero.get(0), -Fp::<3>::new(1));
    }

    #[test]
    fn test_neg_via_zero_minus_self_at_each_required_length() {
        for &len in &[1usize, 16, 63, 64, 65] {
            let v = deterministic_vec(len);
            let mut zero = ScalarPackedFp3Vec::zeros(len);
            zero.sub_assign(&v);
            for i in 0..len {
                let expected = -Fp::<3>::new((i as u64) % 3);
                assert_eq!(zero.get(i), expected, "len = {}, i = {}", len, i);
            }
        }
    }

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
        for &len in REQUIRED_LENGTHS {
            let mut v = ScalarPackedFp3Vec::zeros(len);
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

    #[test]
    fn test_all_zero_on_zeros_constructor_at_each_required_length() {
        for &len in &[0usize, 1, 16, 63, 64, 65] {
            let v = ScalarPackedFp3Vec::zeros(len);
            assert!(v.all_zero(), "len = {}", len);
        }
    }

    #[test]
    fn test_all_zero_false_after_setting_one_position_nonzero_at_each_required_length() {
        for &len in REQUIRED_LENGTHS {
            let mut v = ScalarPackedFp3Vec::zeros(len);
            set_position(&mut v, len - 1, Fp::<3>::new(1));
            assert!(!v.all_zero(), "len = {}", len);
        }
    }

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
            let mut bc = b.clone();
            bc.add_assign(&c);
            let mut lhs = a.clone();
            lhs.mul_assign(&bc);
            let mut ab = a.clone();
            ab.mul_assign(&b);
            let mut ac = a.clone();
            ac.mul_assign(&c);
            ab.add_assign(&ac);
            prop_assert_eq!(lhs, ab);
        }
    }
}
