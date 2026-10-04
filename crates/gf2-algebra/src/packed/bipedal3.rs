//! Fixed-width packed `F_3` element encoding ("bipedal3").
//!
//! [`Bipedal3`] packs 64 independent `F_3` lanes into two `u64` words
//! (`mag` and `sgn`). Arithmetic follows the bitwise formulas of
//! `@/citation/Scheinerman2024` (Theorem 2.1).
//!
//! # Encoding
//!
//! Each `F_3` element `x ∈ {0, 1, 2}` is stored as `(mag_bit, sgn_bit)`:
//!
//! | `x` | `mag` bit | `sgn` bit | note                      |
//! |-----|-----------|-----------|---------------------------|
//! |  0  |     0     |     0     | canonical zero             |
//! |  1  |     1     |     0     |                            |
//! |  2  |     1     |     1     | `≡ −1 (mod 3)`            |
//! |  0  |     0     |     1     | alternative zero           |
//!
//! Bit `s` of `mag` and bit `s` of `sgn` encode lane `s`.

use core::fmt;

use gf2_core::gfp::Fp;

use super::{PackedField, PackedFieldVec};

/// 64 `F_3` lanes in a `(mag, sgn)` `u64` pair; lane `i` occupies bit `i` of
/// each word, encoded as in the [module docs](self).
///
/// `add`, `sub`, and `mul` can produce the alternative-zero codeword
/// `(mag=0, sgn=1)` from canonical inputs; [`Bipedal3::lane`],
/// [`Bipedal3::all_zero`], and `Eq` treat it as zero.
#[derive(Clone, Copy)]
pub struct Bipedal3 {
    mag: u64,
    sgn: u64,
}

impl PartialEq for Bipedal3 {
    /// Canonical-decode equality: two values are equal iff every decoded
    /// lane is equal, regardless of the `sgn` bit on lanes whose `mag`
    /// bit is 0 (alternative-zero lanes).
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.mag != other.mag {
            return false;
        }
        // mag bits equal; check that sgn differs only on lanes where
        // mag == 0 (where sgn is don't-care for canonical-decode).
        (self.sgn ^ other.sgn) & self.mag == 0
    }
}

impl Eq for Bipedal3 {}

impl fmt::Debug for Bipedal3 {
    /// Formats as a 64-element array of decoded lane values in `{0, 1, 2}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bipedal3")
            .field(
                "lanes",
                &core::array::from_fn::<u64, 64, _>(|i| {
                    let m = (self.mag >> i) & 1;
                    let g = (self.sgn >> i) & 1;
                    if m == 0 {
                        0u64
                    } else if g == 0 {
                        1u64
                    } else {
                        2u64
                    }
                }),
            )
            .finish()
    }
}

impl Bipedal3 {
    /// Construct from raw `(mag, sgn)` words; bit `i` of each word is the
    /// corresponding bit of lane `i`.
    #[inline]
    pub fn from_raw(mag: u64, sgn: u64) -> Self {
        Self { mag, sgn }
    }

    /// Raw magnitude word; bit `i` is the `mag` bit of lane `i`.
    #[inline]
    pub fn mag(self) -> u64 {
        self.mag
    }

    /// Raw sign word; bit `i` is the `sgn` bit of lane `i`.
    #[inline]
    pub fn sgn(self) -> u64 {
        self.sgn
    }

    /// Broadcast a single `(mag_bit, sgn_bit)` pair to all 64 lanes; only
    /// bit 0 of each argument is used.
    #[inline]
    pub fn splat_raw(mag_bit: u64, sgn_bit: u64) -> Self {
        Self {
            mag: 0u64.wrapping_sub(mag_bit & 1), // 0 → 0, 1 → u64::MAX
            sgn: 0u64.wrapping_sub(sgn_bit & 1),
        }
    }

    /// Product of the first `n` lanes via the bipedal-multiplication-tree
    /// halving fold (`@/citation/Scheinerman2024` §3.3).
    ///
    /// Lanes `n..64` are padded with the multiplicative identity
    /// (`mag=1`, `sgn=0`) before the six halving steps.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0` or `n > 64`.
    #[inline]
    pub fn fold_mul_first_n(self, n: usize) -> Fp<3> {
        assert!(
            (1..=64).contains(&n),
            "Bipedal3::fold_mul_first_n: n must satisfy 1 <= n <= 64; got n = {n}"
        );
        let used_mask: u64 = if n < 64 { (1u64 << n) - 1 } else { u64::MAX };
        let mut acc_m = self.mag | !used_mask; // set mag=1 for bits n..63
        let mut acc_s = self.sgn & used_mask; // clear sgn for bits n..63
        let mut step: u32 = 32;
        while step > 0 {
            acc_m &= acc_m >> step;
            acc_s ^= acc_s >> step;
            step >>= 1;
        }
        // Bit 0 of (acc_m, acc_s) encodes the product of the n active lanes.
        if acc_m & 1 == 0 {
            Fp::<3>::new(0)
        } else if acc_s & 1 == 0 {
            Fp::<3>::new(1)
        } else {
            Fp::<3>::new(2)
        }
    }

    /// [`PackedField::add`] as an inherent method: a fixed proof target for
    /// `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean`, independent of
    /// trait dispatch.
    #[inline]
    pub fn add_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<3>>>::add(self, rhs)
    }

    /// [`PackedField::sub`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn sub_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<3>>>::sub(self, rhs)
    }

    /// [`PackedField::mul`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn mul_inherent(self, rhs: Self) -> Self {
        <Self as PackedField<Fp<3>>>::mul(self, rhs)
    }

    /// [`PackedField::neg`] as an inherent proof target; see
    /// [`Self::add_inherent`].
    #[inline]
    pub fn neg_inherent(self) -> Self {
        <Self as PackedField<Fp<3>>>::neg(self)
    }
}

impl PackedField<Fp<3>> for Bipedal3 {
    const LANES: usize = 64;

    #[inline]
    fn zero() -> Self {
        Self { mag: 0, sgn: 0 }
    }

    #[inline]
    fn one() -> Self {
        Self {
            mag: u64::MAX,
            sgn: 0,
        }
    }

    #[inline]
    fn splat(x: Fp<3>) -> Self {
        let v = x.value(); // 0, 1, or 2
        let mag_bit = if v != 0 { 1u64 } else { 0u64 };
        let sgn_bit = if v == 2 { 1u64 } else { 0u64 };
        Self::splat_raw(mag_bit, sgn_bit)
    }

    /// `@/citation/Scheinerman2024` Theorem 2.1: 6 word-level operations.
    #[inline]
    fn add(self, rhs: Self) -> Self {
        let am = self.mag;
        let asg = self.sgn;
        let bm = rhs.mag;
        let bsg = rhs.sgn;
        let t = am ^ asg ^ bsg;
        let u = bm & t;
        Self {
            mag: u | (am ^ bm),
            sgn: u ^ asg,
        }
    }

    /// `@/citation/Scheinerman2024` §2.2: 6 word-level operations.
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let am = self.mag;
        let asg = self.sgn;
        let bm = rhs.mag;
        let bsg = rhs.sgn;
        let t = asg ^ bsg;
        let u = am & t;
        Self {
            mag: u | (am ^ bm),
            sgn: u ^ (bm ^ bsg),
        }
    }

    /// `sgn ^= mag` swaps 1 and 2 and leaves every `mag = 0` lane zero.
    #[inline]
    fn neg(self) -> Self {
        Self {
            mag: self.mag,
            sgn: self.sgn ^ self.mag,
        }
    }

    /// `@/citation/Scheinerman2024` product: 2 word-level operations.
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            mag: self.mag & rhs.mag,
            sgn: self.sgn ^ rhs.sgn,
        }
    }

    /// Decodes the alternative-zero codeword `(mag=0, sgn=1)` to 0.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 64`.
    #[inline]
    fn lane(self, i: usize) -> Fp<3> {
        assert!(
            i < Self::LANES,
            "Bipedal3::lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let m = (self.mag >> i) & 1;
        let g = (self.sgn >> i) & 1;
        if m == 0 {
            Fp::<3>::new(0)
        } else if g == 0 {
            Fp::<3>::new(1)
        } else {
            Fp::<3>::new(2)
        }
    }

    /// Writes the canonical codeword for `x`, overwriting an
    /// alternative-zero codeword at lane `i`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= 64`.
    #[inline]
    fn with_lane(self, i: usize, x: Fp<3>) -> Self {
        assert!(
            i < Self::LANES,
            "Bipedal3::with_lane: index {} out of range (LANES = {})",
            i,
            Self::LANES
        );
        let v = x.value(); // 0, 1, or 2
        let mag_bit = if v != 0 { 1u64 } else { 0u64 };
        let sgn_bit = if v == 2 { 1u64 } else { 0u64 };
        let mask = 1u64 << i;
        Self {
            mag: (self.mag & !mask) | (mag_bit << i),
            sgn: (self.sgn & !mask) | (sgn_bit << i),
        }
    }

    /// `mag == 0` suffices: a lane is zero iff its `mag` bit is clear,
    /// whatever its `sgn` bit.
    #[inline]
    fn all_zero(self) -> bool {
        self.mag == 0
    }
}

#[cfg(test)]
mod tests {
    use super::super::ScalarPackedFp3;
    use super::*;
    use proptest::prelude::*;

    fn fp3_strat() -> impl Strategy<Value = Fp<3>> {
        (0u64..3).prop_map(Fp::<3>::new)
    }

    fn bipedal_strat() -> impl Strategy<Value = Bipedal3> {
        prop::collection::vec(fp3_strat(), 64).prop_map(|v| {
            let mut p = Bipedal3::zero();
            for (i, x) in v.into_iter().enumerate() {
                p = p.with_lane(i, x);
            }
            p
        })
    }

    fn bipedal_with_alt_zero_strat() -> impl Strategy<Value = Bipedal3> {
        bipedal_strat().prop_flat_map(|b| {
            (any::<u64>()).prop_map(move |extra_sgn| {
                let zero_lanes = !b.mag;
                Bipedal3 {
                    mag: b.mag,
                    sgn: b.sgn | (extra_sgn & zero_lanes),
                }
            })
        })
    }

    fn scalar_from_bipedal(b: &Bipedal3) -> ScalarPackedFp3 {
        let mut s = ScalarPackedFp3::zero();
        for i in 0..64 {
            s = s.with_lane(i, b.lane(i));
        }
        s
    }

    #[test]
    fn test_lanes_const_is_64() {
        assert_eq!(<Bipedal3 as PackedField<Fp<3>>>::LANES, 64);
    }

    #[test]
    fn test_add_truth_table() {
        let expected: [[u64; 3]; 3] = [[0, 1, 2], [1, 2, 0], [2, 0, 1]];
        for a_v in 0u64..3 {
            for b_v in 0u64..3 {
                let a = Bipedal3::splat(Fp::<3>::new(a_v));
                let b = Bipedal3::splat(Fp::<3>::new(b_v));
                let result = a.add(b);
                let got = result.lane(0).value();
                let exp = expected[a_v as usize][b_v as usize];
                assert_eq!(got, exp, "add({a_v}, {b_v}): expected {exp}, got {got}");
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), exp);
                }
            }
        }
    }

    #[test]
    fn test_sub_truth_table() {
        let expected: [[u64; 3]; 3] = [[0, 2, 1], [1, 0, 2], [2, 1, 0]];
        for a_v in 0u64..3 {
            for b_v in 0u64..3 {
                let a = Bipedal3::splat(Fp::<3>::new(a_v));
                let b = Bipedal3::splat(Fp::<3>::new(b_v));
                let result = a.sub(b);
                let got = result.lane(0).value();
                let exp = expected[a_v as usize][b_v as usize];
                assert_eq!(got, exp, "sub({a_v}, {b_v}): expected {exp}, got {got}");
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), exp);
                }
            }
        }
    }

    #[test]
    fn test_mul_truth_table() {
        let expected: [[u64; 3]; 3] = [[0, 0, 0], [0, 1, 2], [0, 2, 1]];
        for a_v in 0u64..3 {
            for b_v in 0u64..3 {
                let a = Bipedal3::splat(Fp::<3>::new(a_v));
                let b = Bipedal3::splat(Fp::<3>::new(b_v));
                let result = a.mul(b);
                let got = result.lane(0).value();
                let exp = expected[a_v as usize][b_v as usize];
                assert_eq!(got, exp, "mul({a_v}, {b_v}): expected {exp}, got {got}");
                for i in 1..64 {
                    assert_eq!(result.lane(i).value(), exp);
                }
            }
        }
    }

    #[test]
    fn test_add_can_produce_alt_zero_from_canonical_inputs() {
        let two = Bipedal3::splat(Fp::<3>::new(2));
        let one = Bipedal3::splat(Fp::<3>::new(1));
        let result = two.add(one);

        assert_eq!(result.mag(), 0);
        assert_eq!(result.sgn(), u64::MAX);
        assert_eq!(result.lane(0), Fp::<3>::new(0));
    }

    #[test]
    fn test_sub_can_produce_alt_zero_from_canonical_inputs() {
        let one = Bipedal3::splat(Fp::<3>::new(1));
        let result = one.sub(one);

        assert_eq!(result.mag(), 0);
        assert_eq!(result.sgn(), u64::MAX);
        assert_eq!(result.lane(0), Fp::<3>::new(0));
    }

    #[test]
    fn test_mul_can_produce_alt_zero_from_canonical_inputs() {
        let zero = Bipedal3::splat(Fp::<3>::new(0));
        let two = Bipedal3::splat(Fp::<3>::new(2));
        let result = zero.mul(two);

        assert_eq!(result.mag(), 0);
        assert_eq!(result.sgn(), u64::MAX);
        assert_eq!(result.lane(0), Fp::<3>::new(0));
    }

    #[test]
    fn test_neg_truth_table() {
        let expected = [0u64, 2, 1];
        for v in 0u64..3 {
            let a = Bipedal3::splat(Fp::<3>::new(v));
            let result = a.neg();
            let got = result.lane(0).value();
            let exp = expected[v as usize];
            assert_eq!(got, exp, "neg({v}): expected {exp}, got {got}");
            for i in 1..64 {
                assert_eq!(result.lane(i).value(), exp);
            }
        }
    }

    #[test]
    fn test_lane_canonicalises_alt_zero() {
        let v = Bipedal3 {
            mag: 0,
            sgn: 1 << 5,
        };
        assert_eq!(
            v.lane(5),
            Fp::<3>::new(0),
            "alt-zero at lane 5 must decode to 0"
        );
        for i in 0..64 {
            assert_eq!(v.lane(i), Fp::<3>::new(0));
        }
    }

    #[test]
    fn test_eq_alt_zero_equals_canonical_zero() {
        let canon = Bipedal3::zero();
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        assert_eq!(
            canon, alt,
            "canonical zero and all-alt-zero must compare equal"
        );
    }

    #[test]
    fn test_with_lane_canonicalises() {
        let start = Bipedal3 {
            mag: u64::MAX,
            sgn: 0,
        };
        let result = start.with_lane(0, Fp::<3>::new(0));
        assert_eq!(result.mag & 1, 0, "mag bit 0 must be cleared");
        assert_eq!(result.sgn & 1, 0, "sgn bit 0 must be canonical (0)");
        for i in 1..64 {
            assert_eq!(result.lane(i), Fp::<3>::new(1));
        }
    }

    #[test]
    fn test_with_lane_roundtrip() {
        let mut v = Bipedal3::zero();
        for i in 0..64 {
            v = v.with_lane(i, Fp::<3>::new((i as u64) % 3));
        }
        for i in 0..64 {
            let v2 = v.with_lane(i, v.lane(i));
            assert_eq!(v, v2, "round-trip failed at lane {i}");
        }
    }

    #[test]
    fn test_all_zero_canonical_zero() {
        assert!(Bipedal3::zero().all_zero());
    }

    #[test]
    fn test_all_zero_alt_zero() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        assert!(alt.all_zero(), "alt-zero must be reported as all-zero");
    }

    #[test]
    fn test_all_zero_one_nonzero_lane() {
        let v = Bipedal3::zero().with_lane(17, Fp::<3>::new(2));
        assert!(!v.all_zero());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_64() {
        let _ = Bipedal3::zero().lane(64);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_lane_panics_out_of_range_65() {
        let _ = Bipedal3::zero().lane(65);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_64() {
        let _ = Bipedal3::zero().with_lane(64, Fp::<3>::new(1));
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_with_lane_panics_out_of_range_65() {
        let _ = Bipedal3::zero().with_lane(65, Fp::<3>::new(1));
    }

    #[test]
    fn test_alt_zero_through_add() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        let one = Bipedal3::splat(Fp::<3>::new(1));

        let r_alt = alt.add(one);
        let r_can = Bipedal3::zero().add(one);
        assert_eq!(r_alt, r_can, "alt_zero + one != canonical_zero + one");

        let r_alt2 = one.add(alt);
        assert_eq!(r_alt2, r_can, "one + alt_zero != one + canonical_zero");
    }

    #[test]
    fn test_alt_zero_through_sub() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        let two = Bipedal3::splat(Fp::<3>::new(2));

        let r = two.sub(alt);
        assert_eq!(r.lane(0).value(), 2, "2 - alt_zero lane 0 must be 2");

        let r2 = alt.sub(two);
        assert_eq!(r2.lane(0).value(), 1, "alt_zero - 2 lane 0 must be 1");
    }

    #[test]
    fn test_alt_zero_through_mul() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };
        let two = Bipedal3::splat(Fp::<3>::new(2));

        let r = two.mul(alt);
        assert!(r.all_zero(), "2 * alt_zero must be 0");

        let r2 = alt.mul(two);
        assert!(r2.all_zero(), "alt_zero * 2 must be 0");
    }

    #[test]
    fn test_alt_zero_through_neg() {
        let alt = Bipedal3 {
            mag: 0,
            sgn: u64::MAX,
        };

        let r = alt.neg();
        for i in 0..64 {
            assert_eq!(
                r.lane(i),
                Fp::<3>::new(0),
                "neg(alt_zero) lane {i} must be 0"
            );
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 1000, .. ProptestConfig::default() })]

        #[test]
        fn test_proptest_add_matches_scalar(
            a in bipedal_strat(),
            b in bipedal_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.add(b);
            let sr = sa.add(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "add lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_sub_matches_scalar(
            a in bipedal_strat(),
            b in bipedal_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.sub(b);
            let sr = sa.sub(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "sub lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_mul_matches_scalar(
            a in bipedal_strat(),
            b in bipedal_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.mul(b);
            let sr = sa.mul(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "mul lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_neg_matches_scalar(a in bipedal_strat()) {
            let sa = scalar_from_bipedal(&a);
            let br = a.neg();
            let sr = sa.neg();
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "neg lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_add_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
            b in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.add(b);
            let sr = sa.add(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "add (alt-zero) lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_sub_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
            b in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.sub(b);
            let sr = sa.sub(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "sub (alt-zero) lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_mul_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
            b in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let sb = scalar_from_bipedal(&b);
            let br = a.mul(b);
            let sr = sa.mul(sb);
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "mul (alt-zero) lane {} mismatch", i
                );
            }
        }

        #[test]
        fn test_proptest_neg_alt_zero_matches_scalar(
            a in bipedal_with_alt_zero_strat(),
        ) {
            let sa = scalar_from_bipedal(&a);
            let br = a.neg();
            let sr = sa.neg();
            for i in 0..64 {
                prop_assert_eq!(
                    br.lane(i), sr.lane(i),
                    "neg (alt-zero) lane {} mismatch", i
                );
            }
        }
    }

    #[test]
    fn test_fold_mul_first_n_all_twos() {
        for n in 1usize..=8 {
            let v = Bipedal3::splat(Fp::<3>::new(2));
            let expected = if n % 2 == 1 {
                Fp::<3>::new(2)
            } else {
                Fp::<3>::new(1)
            };
            let got = v.fold_mul_first_n(n);
            assert_eq!(
                got, expected,
                "all-2s product at n={n}: expected {expected:?}, got {got:?}"
            );
        }
    }

    #[test]
    fn test_fold_mul_first_n_zero_lane_kills_product() {
        let v = Bipedal3::zero()
            .with_lane(0, Fp::<3>::new(1))
            .with_lane(1, Fp::<3>::new(1))
            .with_lane(2, Fp::<3>::new(0))
            .with_lane(3, Fp::<3>::new(1));
        let got = v.fold_mul_first_n(4);
        assert_eq!(
            got,
            Fp::<3>::new(0),
            "zero-lane product must be 0, got {got:?}"
        );
    }

    #[test]
    fn test_fold_mul_first_n_mixed_pattern() {
        let v = Bipedal3::zero()
            .with_lane(0, Fp::<3>::new(1))
            .with_lane(1, Fp::<3>::new(2))
            .with_lane(2, Fp::<3>::new(1));
        let got = v.fold_mul_first_n(3);
        assert_eq!(got, Fp::<3>::new(2), "1*2*1 must be 2, got {got:?}");
    }

    #[test]
    fn test_fold_mul_first_n_single_lane() {
        for v in 0u64..3 {
            let b = Bipedal3::zero().with_lane(0, Fp::<3>::new(v));
            let got = b.fold_mul_first_n(1);
            assert_eq!(
                got,
                Fp::<3>::new(v),
                "1-lane fold of {v} must return {v}, got {got:?}"
            );
        }
    }

    #[test]
    fn test_fold_mul_first_n_full_64_lanes_all_ones() {
        let v = Bipedal3::splat(Fp::<3>::new(1));
        let got = v.fold_mul_first_n(64);
        assert_eq!(got, Fp::<3>::new(1), "all-1s fold over 64 lanes must be 1");
    }

    #[test]
    fn test_bipedal_mul_tree_matches_scalar_fold() {
        for n in 1usize..=8 {
            let v = Bipedal3::from_raw((1u64 << n) - 1, (1u64 << n) - 1);
            let expected = if n % 2 == 1 {
                Fp::<3>::new(2)
            } else {
                Fp::<3>::new(1)
            };
            let got = v.fold_mul_first_n(n);
            assert_eq!(
                got, expected,
                "all-2s product mismatch at n={n}: got {got:?} want {expected:?}"
            );
        }

        // Lanes 0..4 decode to 1, 2, 0, 2.
        {
            let n = 4usize;
            let v = Bipedal3::from_raw(0b1011, 0b1010);
            let got = v.fold_mul_first_n(n);
            assert_eq!(
                got,
                Fp::<3>::new(0),
                "mixed pattern: zero-lane product should be 0"
            );
        }

        {
            let n = 4usize;
            let v = Bipedal3::from_raw((1u64 << n) - 1, 0);
            let got = v.fold_mul_first_n(n);
            assert_eq!(got, Fp::<3>::new(1), "all-1s product should be 1 for n={n}");
        }
    }

    #[test]
    #[should_panic(expected = "n must satisfy 1 <= n <= 64")]
    fn test_fold_mul_first_n_panics_on_zero() {
        let _ = Bipedal3::zero().fold_mul_first_n(0);
    }

    #[test]
    #[should_panic(expected = "n must satisfy 1 <= n <= 64")]
    fn test_fold_mul_first_n_panics_on_65() {
        let _ = Bipedal3::zero().fold_mul_first_n(65);
    }
}

/// Variable-length packed `F_3` vector: `len_lanes` elements in two parallel
/// `Vec<u64>` planes (`mag` and `sgn`) of `ceil(len_lanes / 64)` words.
///
/// Element `i` lives in word `i >> 6` at bit `i & 63` of both planes, in the
/// [`Bipedal3`] encoding. [`get`][`Bipedal3Vec::get`],
/// [`all_zero`][`PackedFieldVec::all_zero`], and [`PartialEq`] treat the
/// alternative-zero codeword `(mag=0, sgn=1)` as zero.
///
/// # Mask-tail invariant
///
/// Bits beyond `len_lanes` in the last word of both planes are zero; every
/// mutating operation restores this through `Bipedal3Vec::mask_tail`.
///
/// # Complexity
///
/// Lane-wise operations are `O(ceil(len_lanes / 64))`; `get` is `O(1)`.
#[derive(Clone)]
pub struct Bipedal3Vec {
    mag: Vec<u64>,
    sgn: Vec<u64>,
    len_lanes: usize,
}

impl Bipedal3Vec {
    /// Zero all bits beyond `self.len_lanes` in the last word of both planes.
    ///
    /// The word-parallel formulas also operate on padding bits; unmasked
    /// padding corrupts `all_zero` and equality.
    fn mask_tail(&mut self) {
        let n_words = self.mag.len();
        if n_words == 0 {
            return;
        }
        let used = self.len_lanes - 64 * (n_words - 1);
        if used == 64 {
            return; // full word; no padding to mask
        }
        let mask = (1u64 << used) - 1;
        let last = n_words - 1;
        self.mag[last] &= mask;
        self.sgn[last] &= mask;
    }

    /// Product of all `len_lanes` elements via the bipedal multiplication
    /// tree; the empty vector yields `Fp::<3>::new(1)`.
    pub fn fold_mul(&self) -> Fp<3> {
        if self.len_lanes == 0 {
            return Fp::<3>::new(1);
        }
        let n_words = self.mag.len();
        // Identity of the `@/citation/Scheinerman2024` product: (mag=1, sgn=0)
        // decodes to 1.
        let mut acc_mag = u64::MAX;
        let mut acc_sgn = 0u64;

        for w in 0..n_words - 1 {
            acc_mag &= self.mag[w];
            acc_sgn ^= self.sgn[w];
        }

        // Last (possibly partial) word: set padding lanes to identity (mag=1, sgn=0).
        let used = self.len_lanes - 64 * (n_words - 1);
        let used_mask = if used == 64 {
            u64::MAX
        } else {
            (1u64 << used) - 1
        };
        let last_m = self.mag[n_words - 1] | !used_mask;
        let last_s = self.sgn[n_words - 1] & used_mask;
        acc_mag &= last_m;
        acc_sgn ^= last_s;

        let mut result = Fp::<3>::new(1);
        for lane in 0..64u64 {
            let m = (acc_mag >> lane) & 1;
            let s = (acc_sgn >> lane) & 1;
            let v = if m == 0 {
                Fp::<3>::new(0)
            } else if s == 0 {
                Fp::<3>::new(1)
            } else {
                Fp::<3>::new(2)
            };
            result = result * v;
        }
        result
    }

    /// Lane-wise in-place additive inverse: `self[i] = -self[i]` for every `i`.
    pub fn neg_assign(&mut self) {
        for w in 0..self.mag.len() {
            self.sgn[w] ^= self.mag[w];
        }
        self.mask_tail();
    }

    /// Raw `mag` plane: bit `i & 63` of word `i >> 6` is the `mag` bit of
    /// element `i`; bits beyond `self.len()` are zero.
    #[inline]
    pub fn raw_mag(&self) -> &[u64] {
        &self.mag
    }

    /// Raw `sgn` plane: bit `i & 63` of word `i >> 6` is the `sgn` bit of
    /// element `i`; bits beyond `self.len()` are zero.
    #[inline]
    pub fn raw_sgn(&self) -> &[u64] {
        &self.sgn
    }
}

impl PartialEq for Bipedal3Vec {
    /// Canonical-decode equality: two vectors are equal iff they have the
    /// same `len_lanes` and every decoded lane is equal.
    ///
    /// The mask-tail invariant makes the per-word comparison exact.
    fn eq(&self, other: &Self) -> bool {
        if self.len_lanes != other.len_lanes {
            return false;
        }
        for w in 0..self.mag.len() {
            if self.mag[w] != other.mag[w] {
                return false;
            }
            if (self.sgn[w] ^ other.sgn[w]) & self.mag[w] != 0 {
                return false;
            }
        }
        true
    }
}

impl Eq for Bipedal3Vec {}

impl fmt::Debug for Bipedal3Vec {
    /// Formats as the list of decoded lane values in `{0, 1, 2}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lanes: Vec<u64> = (0..self.len_lanes)
            .map(|i| {
                let w = i >> 6;
                let b = i & 63;
                let m = (self.mag[w] >> b) & 1;
                let g = (self.sgn[w] >> b) & 1;
                if m == 0 {
                    0u64
                } else if g == 0 {
                    1u64
                } else {
                    2u64
                }
            })
            .collect();
        f.debug_struct("Bipedal3Vec")
            .field("lanes", &lanes)
            .finish()
    }
}

impl PackedFieldVec<Fp<3>> for Bipedal3Vec {
    type Element = Bipedal3;

    fn zeros(len: usize) -> Self {
        let n_words = len.div_ceil(64);
        Self {
            mag: vec![0u64; n_words],
            sgn: vec![0u64; n_words],
            len_lanes: len,
        }
    }

    fn from_field_slice(xs: &[Fp<3>]) -> Self {
        let len = xs.len();
        let n_words = len.div_ceil(64);
        let mut mag = vec![0u64; n_words];
        let mut sgn = vec![0u64; n_words];
        for (i, &x) in xs.iter().enumerate() {
            let v = x.value();
            let w = i >> 6;
            let b = i & 63;
            if v != 0 {
                mag[w] |= 1u64 << b;
            }
            if v == 2 {
                sgn[w] |= 1u64 << b;
            }
        }
        let mut result = Self {
            mag,
            sgn,
            len_lanes: len,
        };
        result.mask_tail();
        result
    }

    fn len(&self) -> usize {
        self.len_lanes
    }

    /// Decodes the alternative-zero codeword `(mag=0, sgn=1)` to 0.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.len()`.
    fn get(&self, i: usize) -> Fp<3> {
        assert!(
            i < self.len_lanes,
            "Bipedal3Vec::get: index {} out of range (len = {})",
            i,
            self.len_lanes
        );
        let w = i >> 6;
        let b = i & 63;
        let m = (self.mag[w] >> b) & 1;
        let g = (self.sgn[w] >> b) & 1;
        if m == 0 {
            Fp::<3>::new(0)
        } else if g == 0 {
            Fp::<3>::new(1)
        } else {
            Fp::<3>::new(2)
        }
    }

    /// Per-word `@/citation/Scheinerman2024` Theorem 2.1 sum.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    fn add_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Bipedal3Vec::add_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.mag.len() {
            let am = self.mag[w];
            let asg = self.sgn[w];
            let bm = rhs.mag[w];
            let bsg = rhs.sgn[w];
            let t = am ^ asg ^ bsg;
            let u = bm & t;
            self.mag[w] = u | (am ^ bm);
            self.sgn[w] = u ^ asg;
        }
        self.mask_tail();
    }

    /// Per-word `@/citation/Scheinerman2024` §2.2 difference.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    fn sub_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Bipedal3Vec::sub_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.mag.len() {
            let am = self.mag[w];
            let asg = self.sgn[w];
            let bm = rhs.mag[w];
            let bsg = rhs.sgn[w];
            let t = asg ^ bsg;
            let u = am & t;
            self.mag[w] = u | (am ^ bm);
            self.sgn[w] = u ^ (bm ^ bsg);
        }
        self.mask_tail();
    }

    /// Per-word `@/citation/Scheinerman2024` product.
    ///
    /// # Panics
    ///
    /// Panics if `self.len() != rhs.len()`.
    fn mul_assign(&mut self, rhs: &Self) {
        assert_eq!(
            self.len_lanes, rhs.len_lanes,
            "Bipedal3Vec::mul_assign: length mismatch ({} vs {})",
            self.len_lanes, rhs.len_lanes
        );
        for w in 0..self.mag.len() {
            self.mag[w] &= rhs.mag[w];
            self.sgn[w] ^= rhs.sgn[w];
        }
        self.mask_tail();
    }

    /// Tests the `mag` plane only: a lane is zero iff its `mag` bit is clear.
    fn all_zero(&self) -> bool {
        self.mag.iter().all(|&w| w == 0)
    }
}

#[cfg(test)]
mod vec_tests {
    use super::super::ScalarPackedFp3Vec;
    use super::*;
    use proptest::prelude::*;

    macro_rules! test_zeros {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let v = Bipedal3Vec::zeros($len);
                assert_eq!(v.len(), $len, "len mismatch for zeros({})", $len);
                assert!(v.all_zero(), "zeros({}) should be all_zero", $len);
                for i in 0..$len {
                    assert_eq!(v.get(i), Fp::<3>::new(0), "zeros({}).get({}) != 0", $len, i);
                }
            }
        };
    }

    #[test]
    fn test_zeros_0() {
        let v = Bipedal3Vec::zeros(0);
        assert_eq!(v.len(), 0);
        assert!(v.all_zero());
    }
    test_zeros!(test_zeros_1, 1);
    test_zeros!(test_zeros_63, 63);
    test_zeros!(test_zeros_64, 64);
    test_zeros!(test_zeros_65, 65);
    test_zeros!(test_zeros_127, 127);
    test_zeros!(test_zeros_128, 128);
    test_zeros!(test_zeros_129, 129);

    macro_rules! test_from_field_slice {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let xs: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new((i * 7 % 3) as u64))
                    .collect();
                let v = Bipedal3Vec::from_field_slice(&xs);
                assert_eq!(v.len(), $len);
                for i in 0..$len {
                    assert_eq!(
                        v.get(i),
                        xs[i],
                        "from_field_slice({}).get({}) mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_from_field_slice_0() {
        let v = Bipedal3Vec::from_field_slice(&[]);
        assert_eq!(v.len(), 0);
        assert!(v.all_zero());
    }
    test_from_field_slice!(test_from_field_slice_1, 1);
    test_from_field_slice!(test_from_field_slice_63, 63);
    test_from_field_slice!(test_from_field_slice_64, 64);
    test_from_field_slice!(test_from_field_slice_65, 65);
    test_from_field_slice!(test_from_field_slice_127, 127);
    test_from_field_slice!(test_from_field_slice_128, 128);
    test_from_field_slice!(test_from_field_slice_129, 129);

    macro_rules! test_add_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let a_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new((i * 3 % 3) as u64))
                    .collect();
                let b_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i + 1) % 3) as u64))
                    .collect();
                let mut a = Bipedal3Vec::from_field_slice(&a_vals);
                let b = Bipedal3Vec::from_field_slice(&b_vals);
                let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
                let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
                a.add_assign(&b);
                sa.add_assign(&sb);
                for i in 0..$len {
                    assert_eq!(
                        a.get(i),
                        sa.get(i),
                        "add_assign({}) lane {} mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_add_assign_0() {
        let mut a = Bipedal3Vec::zeros(0);
        let b = Bipedal3Vec::zeros(0);
        a.add_assign(&b);
        assert_eq!(a.len(), 0);
    }
    test_add_assign!(test_add_assign_1, 1);
    test_add_assign!(test_add_assign_63, 63);
    test_add_assign!(test_add_assign_64, 64);
    test_add_assign!(test_add_assign_65, 65);
    test_add_assign!(test_add_assign_127, 127);
    test_add_assign!(test_add_assign_128, 128);
    test_add_assign!(test_add_assign_129, 129);

    macro_rules! test_sub_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let a_vals: Vec<Fp<3>> = (0..$len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
                let b_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i + 2) % 3) as u64))
                    .collect();
                let mut a = Bipedal3Vec::from_field_slice(&a_vals);
                let b = Bipedal3Vec::from_field_slice(&b_vals);
                let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
                let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
                a.sub_assign(&b);
                sa.sub_assign(&sb);
                for i in 0..$len {
                    assert_eq!(
                        a.get(i),
                        sa.get(i),
                        "sub_assign({}) lane {} mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_sub_assign_0() {
        let mut a = Bipedal3Vec::zeros(0);
        let b = Bipedal3Vec::zeros(0);
        a.sub_assign(&b);
        assert_eq!(a.len(), 0);
    }
    test_sub_assign!(test_sub_assign_1, 1);
    test_sub_assign!(test_sub_assign_63, 63);
    test_sub_assign!(test_sub_assign_64, 64);
    test_sub_assign!(test_sub_assign_65, 65);
    test_sub_assign!(test_sub_assign_127, 127);
    test_sub_assign!(test_sub_assign_128, 128);
    test_sub_assign!(test_sub_assign_129, 129);

    macro_rules! test_mul_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let a_vals: Vec<Fp<3>> = (0..$len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
                let b_vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i + 1) % 3) as u64))
                    .collect();
                let mut a = Bipedal3Vec::from_field_slice(&a_vals);
                let b = Bipedal3Vec::from_field_slice(&b_vals);
                let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
                let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
                a.mul_assign(&b);
                sa.mul_assign(&sb);
                for i in 0..$len {
                    assert_eq!(
                        a.get(i),
                        sa.get(i),
                        "mul_assign({}) lane {} mismatch",
                        $len,
                        i
                    );
                }
            }
        };
    }

    #[test]
    fn test_mul_assign_0() {
        let mut a = Bipedal3Vec::zeros(0);
        let b = Bipedal3Vec::zeros(0);
        a.mul_assign(&b);
        assert_eq!(a.len(), 0);
    }
    test_mul_assign!(test_mul_assign_1, 1);
    test_mul_assign!(test_mul_assign_63, 63);
    test_mul_assign!(test_mul_assign_64, 64);
    test_mul_assign!(test_mul_assign_65, 65);
    test_mul_assign!(test_mul_assign_127, 127);
    test_mul_assign!(test_mul_assign_128, 128);
    test_mul_assign!(test_mul_assign_129, 129);

    #[test]
    fn test_neg_assign_truth_table() {
        let mut v =
            Bipedal3Vec::from_field_slice(&[Fp::<3>::new(0), Fp::<3>::new(1), Fp::<3>::new(2)]);
        v.neg_assign();
        assert_eq!(v.get(0), Fp::<3>::new(0));
        assert_eq!(v.get(1), Fp::<3>::new(2));
        assert_eq!(v.get(2), Fp::<3>::new(1));
    }

    macro_rules! test_neg_assign {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let vals: Vec<Fp<3>> = (0..$len)
                    .map(|i| Fp::<3>::new(((i * 7 + 3) % 3) as u64))
                    .collect();
                let mut v = Bipedal3Vec::from_field_slice(&vals);
                v.neg_assign();
                for i in 0..$len {
                    let orig = vals[i].value();
                    let expected = if orig == 0 { 0 } else { 3 - orig };
                    assert_eq!(
                        v.get(i).value(),
                        expected,
                        "neg_assign({}) lane {} mismatch (orig={})",
                        $len,
                        i,
                        orig
                    );
                }
            }
        };
    }

    #[test]
    fn test_neg_assign_0() {
        let mut v = Bipedal3Vec::zeros(0);
        v.neg_assign();
        assert_eq!(v.len(), 0);
    }
    test_neg_assign!(test_neg_assign_1, 1);
    test_neg_assign!(test_neg_assign_63, 63);
    test_neg_assign!(test_neg_assign_64, 64);
    test_neg_assign!(test_neg_assign_65, 65);
    test_neg_assign!(test_neg_assign_127, 127);
    test_neg_assign!(test_neg_assign_128, 128);
    test_neg_assign!(test_neg_assign_129, 129);

    fn check_mask_tail(len: usize) {
        assert!(
            !len.is_multiple_of(64),
            "only partial-word lengths have padding"
        );
        let used = len % 64;
        let used_mask: u64 = (1u64 << used) - 1;
        let padding_mask: u64 = !used_mask;

        let xs: Vec<Fp<3>> = (0..len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
        let ys: Vec<Fp<3>> = (0..len)
            .map(|i| Fp::<3>::new(((i + 1) % 3) as u64))
            .collect();

        let v = Bipedal3Vec::from_field_slice(&xs);
        let last = v.mag.len() - 1;
        assert_eq!(
            (v.mag[last] | v.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after from_field_slice (len={len})"
        );

        let mut a = v.clone();
        let b = Bipedal3Vec::from_field_slice(&ys);
        a.add_assign(&b);
        let last = a.mag.len() - 1;
        assert_eq!(
            (a.mag[last] | a.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after add_assign (len={len})"
        );

        let mut c = Bipedal3Vec::from_field_slice(&xs);
        let d = Bipedal3Vec::from_field_slice(&ys);
        c.sub_assign(&d);
        let last = c.mag.len() - 1;
        assert_eq!(
            (c.mag[last] | c.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after sub_assign (len={len})"
        );

        let mut e = Bipedal3Vec::from_field_slice(&xs);
        let f = Bipedal3Vec::from_field_slice(&ys);
        e.mul_assign(&f);
        let last = e.mag.len() - 1;
        assert_eq!(
            (e.mag[last] | e.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after mul_assign (len={len})"
        );

        let mut g = Bipedal3Vec::from_field_slice(&xs);
        g.neg_assign();
        let last = g.mag.len() - 1;
        assert_eq!(
            (g.mag[last] | g.sgn[last]) & padding_mask,
            0,
            "mask_tail violated after neg_assign (len={len})"
        );
    }

    #[test]
    fn test_mask_tail_invariant_1() {
        check_mask_tail(1);
    }
    #[test]
    fn test_mask_tail_invariant_63() {
        check_mask_tail(63);
    }
    #[test]
    fn test_mask_tail_invariant_65() {
        check_mask_tail(65);
    }
    #[test]
    fn test_mask_tail_invariant_127() {
        check_mask_tail(127);
    }
    #[test]
    fn test_mask_tail_invariant_129() {
        check_mask_tail(129);
    }

    #[test]
    fn test_fold_mul_empty() {
        let v = Bipedal3Vec::zeros(0);
        assert_eq!(
            v.fold_mul(),
            Fp::<3>::new(1),
            "fold_mul of empty vec must be 1 (multiplicative identity)"
        );
    }

    macro_rules! test_fold_mul {
        ($name:ident, $len:expr) => {
            #[test]
            fn $name() {
                let xs: Vec<Fp<3>> = (0..$len).map(|i| Fp::<3>::new((i % 3) as u64)).collect();
                let v = Bipedal3Vec::from_field_slice(&xs);
                let expected = (0..$len).fold(Fp::<3>::new(1), |acc, i| acc * v.get(i));
                assert_eq!(v.fold_mul(), expected, "fold_mul({}) mismatch", $len);
            }
        };
    }

    test_fold_mul!(test_fold_mul_1, 1);
    test_fold_mul!(test_fold_mul_7, 7);
    test_fold_mul!(test_fold_mul_64, 64);
    test_fold_mul!(test_fold_mul_100, 100);
    test_fold_mul!(test_fold_mul_200, 200);

    #[test]
    fn test_eq_alt_zero_vs_canonical() {
        let canon = Bipedal3Vec::zeros(5);
        let mut alt = Bipedal3Vec::zeros(5);
        alt.sgn[0] = 1 << 2;
        assert_eq!(canon, alt, "canonical zero and alt-zero must compare equal");
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_0() {
        let v = Bipedal3Vec::zeros(0);
        let _ = v.get(0);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_1() {
        let v = Bipedal3Vec::zeros(1);
        let _ = v.get(1);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_64() {
        let v = Bipedal3Vec::zeros(64);
        let _ = v.get(64);
    }

    #[test]
    #[should_panic(expected = "length mismatch")]
    fn test_add_assign_panics_on_length_mismatch() {
        let mut a = Bipedal3Vec::zeros(3);
        let b = Bipedal3Vec::zeros(4);
        a.add_assign(&b);
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, .. ProptestConfig::default() })]

        #[test]
        fn test_proptest_add_assign_matches_scalar(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a = Bipedal3Vec::from_field_slice(&a_vals);
            let b = Bipedal3Vec::from_field_slice(&b_vals);
            let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
            let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
            a.add_assign(&b);
            sa.add_assign(&sb);
            for i in 0..len {
                prop_assert_eq!(a.get(i), sa.get(i), "add_assign lane {} mismatch (len={})", i, len);
            }
        }

        #[test]
        fn test_proptest_sub_assign_matches_scalar(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a = Bipedal3Vec::from_field_slice(&a_vals);
            let b = Bipedal3Vec::from_field_slice(&b_vals);
            let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
            let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
            a.sub_assign(&b);
            sa.sub_assign(&sb);
            for i in 0..len {
                prop_assert_eq!(a.get(i), sa.get(i), "sub_assign lane {} mismatch (len={})", i, len);
            }
        }

        #[test]
        fn test_proptest_mul_assign_matches_scalar(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a = Bipedal3Vec::from_field_slice(&a_vals);
            let b = Bipedal3Vec::from_field_slice(&b_vals);
            let mut sa = ScalarPackedFp3Vec::from_field_slice(&a_vals);
            let sb = ScalarPackedFp3Vec::from_field_slice(&b_vals);
            a.mul_assign(&b);
            sa.mul_assign(&sb);
            for i in 0..len {
                prop_assert_eq!(a.get(i), sa.get(i), "mul_assign lane {} mismatch (len={})", i, len);
            }
        }

        #[test]
        fn test_proptest_fold_mul_matches_scalar_fold(
            len in 0usize..200,
            vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let vals: Vec<Fp<3>> = vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let v = Bipedal3Vec::from_field_slice(&vals);
            let expected = (0..len).fold(Fp::<3>::new(1), |acc, i| acc * v.get(i));
            prop_assert_eq!(v.fold_mul(), expected, "fold_mul mismatch (len={})", len);
        }
    }

    fn chunks_of(v: &Bipedal3Vec) -> Vec<(Bipedal3, usize)> {
        let n_words = v.mag.len();
        if n_words == 0 {
            return Vec::new();
        }
        let mut chunks = Vec::with_capacity(n_words);
        for w in 0..n_words {
            let used = if w + 1 == n_words {
                v.len_lanes - 64 * w
            } else {
                64
            };
            chunks.push((Bipedal3::from_raw(v.mag[w], v.sgn[w]), used));
        }
        chunks
    }

    fn compose_chunks(chunks: &[(Bipedal3, usize)], total_len: usize) -> Vec<Fp<3>> {
        let mut out = Vec::with_capacity(total_len);
        for (chunk, used) in chunks {
            for lane in 0..*used {
                out.push(chunk.lane(lane));
            }
        }
        debug_assert_eq!(out.len(), total_len);
        out
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, .. ProptestConfig::default() })]

        #[test]
        fn test_proptest_add_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            let b_vec = Bipedal3Vec::from_field_slice(&b_vals);
            a_vec.add_assign(&b_vec);
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let b_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&b_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .zip(b_chunks.into_iter())
                .map(|((ac, used), (bc, _))| (ac.add(bc), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "add chunked vs vec mismatch (len={})", len);
        }

        #[test]
        fn test_proptest_sub_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            let b_vec = Bipedal3Vec::from_field_slice(&b_vals);
            a_vec.sub_assign(&b_vec);
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let b_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&b_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .zip(b_chunks.into_iter())
                .map(|((ac, used), (bc, _))| (ac.sub(bc), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "sub chunked vs vec mismatch (len={})", len);
        }

        #[test]
        fn test_proptest_mul_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
            b_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();
            let b_vals: Vec<Fp<3>> = b_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            let b_vec = Bipedal3Vec::from_field_slice(&b_vals);
            a_vec.mul_assign(&b_vec);
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let b_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&b_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .zip(b_chunks.into_iter())
                .map(|((ac, used), (bc, _))| (ac.mul(bc), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "mul chunked vs vec mismatch (len={})", len);
        }

        #[test]
        fn test_proptest_neg_chunked_matches_vec(
            len in 0usize..200,
            a_vals in prop::collection::vec((0u64..3).prop_map(Fp::<3>::new), 0..200),
        ) {
            let a_vals: Vec<Fp<3>> = a_vals.into_iter().chain(core::iter::repeat(Fp::<3>::new(0))).take(len).collect();

            let mut a_vec = Bipedal3Vec::from_field_slice(&a_vals);
            a_vec.neg_assign();
            let direct: Vec<Fp<3>> = (0..len).map(|i| a_vec.get(i)).collect();

            let a_chunks = chunks_of(&Bipedal3Vec::from_field_slice(&a_vals));
            let chunked_pairs: Vec<(Bipedal3, usize)> = a_chunks
                .into_iter()
                .map(|(c, used)| (c.neg(), used))
                .collect();
            let chunked_decoded = compose_chunks(&chunked_pairs, len);

            prop_assert_eq!(direct, chunked_decoded, "neg chunked vs vec mismatch (len={})", len);
        }
    }
}

/// Rectangular `rows × cols` matrix of packed `F_3` values, stored
/// column-major as one [`Bipedal3Vec`] of length `rows` per column, so the
/// column-iterating permanent kernels borrow a column without copying.
#[derive(Clone)]
pub struct Bipedal3Matrix {
    /// One `Bipedal3Vec` per column, each of length `rows`.
    columns: Vec<Bipedal3Vec>,
    rows: usize,
    cols: usize,
}

impl PartialEq for Bipedal3Matrix {
    /// Shape-equal and per-column canonical-decode equal.
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows && self.cols == other.cols && self.columns == other.columns
    }
}

impl Eq for Bipedal3Matrix {}

impl core::fmt::Debug for Bipedal3Matrix {
    /// Formats as `Bipedal3Matrix { rows, cols, data }` with `data` listed
    /// row by row.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let data: Vec<Vec<u64>> = (0..self.rows)
            .map(|i| {
                (0..self.cols)
                    .map(|j| self.columns[j].get(i).value())
                    .collect()
            })
            .collect();
        f.debug_struct("Bipedal3Matrix")
            .field("rows", &self.rows)
            .field("cols", &self.cols)
            .field("data", &data)
            .finish()
    }
}

impl Bipedal3Matrix {
    /// Construct a matrix from a row-major `Fp<3>` slice.
    ///
    /// The entry at row `i`, column `j` is `data[i * cols + j]`. `rows == 0`
    /// or `cols == 0` is allowed.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != rows * cols`.
    pub fn from_row_major(data: &[Fp<3>], rows: usize, cols: usize) -> Self {
        assert_eq!(
            data.len(),
            rows * cols,
            "Bipedal3Matrix::from_row_major: data.len() ({}) != rows ({}) * cols ({})",
            data.len(),
            rows,
            cols
        );
        let columns: Vec<Bipedal3Vec> = (0..cols)
            .map(|j| {
                let col_data: Vec<Fp<3>> = (0..rows).map(|i| data[i * cols + j]).collect();
                Bipedal3Vec::from_field_slice(&col_data)
            })
            .collect();
        Self {
            columns,
            rows,
            cols,
        }
    }

    /// Inverse of [`from_row_major`][Self::from_row_major]: returns a row-major
    /// decoded `Vec<Fp<3>>` of length `rows * cols`.
    pub fn to_row_major(&self) -> Vec<Fp<3>> {
        let mut out = Vec::with_capacity(self.rows * self.cols);
        for i in 0..self.rows {
            for j in 0..self.cols {
                out.push(self.columns[j].get(i));
            }
        }
        out
    }

    /// Number of rows.
    #[inline]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns.
    #[inline]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Borrow the `j`-th column as a `&Bipedal3Vec` of length `rows`.
    ///
    /// # Panics
    ///
    /// Panics if `j >= self.cols()`.
    #[inline]
    pub fn column(&self, j: usize) -> &Bipedal3Vec {
        assert!(
            j < self.cols,
            "Bipedal3Matrix::column: index {} out of range (cols = {})",
            j,
            self.cols
        );
        &self.columns[j]
    }

    /// Reconstruct the `i`-th row as an owned `Bipedal3Vec` of length `cols`.
    ///
    /// Lane `j` of the result equals `self.column(j).get(i)`; `O(cols)`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()`.
    pub fn row(&self, i: usize) -> Bipedal3Vec {
        assert!(
            i < self.rows,
            "Bipedal3Matrix::row: index {} out of range (rows = {})",
            i,
            self.rows
        );
        let row_data: Vec<Fp<3>> = (0..self.cols).map(|j| self.columns[j].get(i)).collect();
        Bipedal3Vec::from_field_slice(&row_data)
    }

    /// Read the entry at row `i`, column `j`.
    ///
    /// # Panics
    ///
    /// Panics if `i >= self.rows()` or `j >= self.cols()`.
    pub fn get(&self, i: usize, j: usize) -> Fp<3> {
        assert!(
            i < self.rows,
            "Bipedal3Matrix::get: row index {} out of range (rows = {})",
            i,
            self.rows
        );
        assert!(
            j < self.cols,
            "Bipedal3Matrix::get: col index {} out of range (cols = {})",
            j,
            self.cols
        );
        self.columns[j].get(i)
    }

    /// Transpose: returns a `cols × rows` matrix where `transposed.get(j, i) == self.get(i, j)`.
    pub fn transpose(&self) -> Self {
        let rm = self.to_row_major();
        let mut tm = Vec::with_capacity(self.cols * self.rows);
        for j in 0..self.cols {
            for i in 0..self.rows {
                tm.push(rm[i * self.cols + j]);
            }
        }
        Self::from_row_major(&tm, self.cols, self.rows)
    }
}

#[cfg(test)]
mod matrix_tests {
    use super::*;
    use proptest::prelude::*;

    fn det_data(rows: usize, cols: usize) -> Vec<Fp<3>> {
        (0..rows)
            .flat_map(|i| (0..cols).map(move |j| Fp::<3>::new(((i * 7 + j * 11 + 5) as u64) % 3)))
            .collect()
    }

    macro_rules! test_roundtrip {
        ($name:ident, $rows:expr, $cols:expr) => {
            #[test]
            fn $name() {
                let data = det_data($rows, $cols);
                let m = Bipedal3Matrix::from_row_major(&data, $rows, $cols);
                assert_eq!(m.rows(), $rows);
                assert_eq!(m.cols(), $cols);
                let out = m.to_row_major();
                assert_eq!(out, data, "roundtrip mismatch for {}x{}", $rows, $cols);
            }
        };
    }

    #[test]
    fn test_from_row_major_to_row_major_roundtrip_0x0() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 0);
        assert_eq!(m.rows(), 0);
        assert_eq!(m.cols(), 0);
        assert!(m.to_row_major().is_empty());
    }

    #[test]
    fn test_from_row_major_to_row_major_roundtrip_0x5() {
        let m = Bipedal3Matrix::from_row_major(&[], 0, 5);
        assert_eq!(m.rows(), 0);
        assert_eq!(m.cols(), 5);
        assert!(m.to_row_major().is_empty());
    }

    #[test]
    fn test_from_row_major_to_row_major_roundtrip_5x0() {
        let m = Bipedal3Matrix::from_row_major(&[], 5, 0);
        assert_eq!(m.rows(), 5);
        assert_eq!(m.cols(), 0);
        assert!(m.to_row_major().is_empty());
    }

    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_1x1, 1, 1);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_1x64, 1, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x1, 64, 1);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_63x63, 63, 63);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_63x64, 63, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x63, 64, 63);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x64, 64, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_64x65, 64, 65);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_65x64, 65, 64);
    test_roundtrip!(test_from_row_major_to_row_major_roundtrip_65x65, 65, 65);

    macro_rules! test_get {
        ($name:ident, $rows:expr, $cols:expr) => {
            #[test]
            fn $name() {
                let data = det_data($rows, $cols);
                let m = Bipedal3Matrix::from_row_major(&data, $rows, $cols);
                for i in 0..$rows {
                    for j in 0..$cols {
                        assert_eq!(
                            m.get(i, j),
                            data[i * $cols + j],
                            "get({},{}) mismatch for {}x{}",
                            i,
                            j,
                            $rows,
                            $cols
                        );
                    }
                }
            }
        };
    }

    test_get!(test_get_1x1, 1, 1);
    test_get!(test_get_1x64, 1, 64);
    test_get!(test_get_64x1, 64, 1);
    test_get!(test_get_63x63, 63, 63);
    test_get!(test_get_63x64, 63, 64);
    test_get!(test_get_64x63, 64, 63);
    test_get!(test_get_64x64, 64, 64);
    test_get!(test_get_65x65, 65, 65);

    #[test]
    fn test_column_returns_correct_vec() {
        let data = det_data(5, 3);
        let m = Bipedal3Matrix::from_row_major(&data, 5, 3);
        for j in 0..3 {
            for i in 0..5 {
                assert_eq!(
                    m.column(j).get(i),
                    m.get(i, j),
                    "column({}).get({}) != get({}, {})",
                    j,
                    i,
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_row_returns_correct_vec() {
        let data = det_data(5, 3);
        let m = Bipedal3Matrix::from_row_major(&data, 5, 3);
        for i in 0..5 {
            let row_vec = m.row(i);
            for j in 0..3 {
                assert_eq!(
                    row_vec.get(j),
                    m.get(i, j),
                    "row({}).get({}) != get({}, {})",
                    i,
                    j,
                    i,
                    j
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_row() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.get(3, 0);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_get_panics_out_of_range_col() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.get(0, 4);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_column_panics_out_of_range() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.column(4);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn test_row_panics_out_of_range() {
        let m = Bipedal3Matrix::from_row_major(&det_data(3, 4), 3, 4);
        let _ = m.row(3);
    }

    #[test]
    #[should_panic(expected = "rows")]
    fn test_from_row_major_panics_on_length_mismatch() {
        let data: Vec<Fp<3>> = (0..5u64).map(|v| Fp::<3>::new(v % 3)).collect();
        let _ = Bipedal3Matrix::from_row_major(&data, 2, 3);
    }

    macro_rules! test_transpose_roundtrip {
        ($name:ident, $rows:expr, $cols:expr) => {
            #[test]
            fn $name() {
                let data = det_data($rows, $cols);
                let m = Bipedal3Matrix::from_row_major(&data, $rows, $cols);
                let tt = m.transpose().transpose();
                assert_eq!(
                    m, tt,
                    "transpose().transpose() != self for {}x{}",
                    $rows, $cols
                );
            }
        };
    }

    test_transpose_roundtrip!(test_transpose_roundtrip_1x1, 1, 1);
    test_transpose_roundtrip!(test_transpose_roundtrip_5x7, 5, 7);
    test_transpose_roundtrip!(test_transpose_roundtrip_63x65, 63, 65);
    test_transpose_roundtrip!(test_transpose_roundtrip_64x64, 64, 64);
    test_transpose_roundtrip!(test_transpose_roundtrip_64x100, 64, 100);
    test_transpose_roundtrip!(test_transpose_roundtrip_130x17, 130, 17);

    #[test]
    fn test_transpose_value_check_5x3() {
        let data = det_data(5, 3);
        let m = Bipedal3Matrix::from_row_major(&data, 5, 3);
        let t = m.transpose();
        assert_eq!(t.rows(), 3);
        assert_eq!(t.cols(), 5);
        for i in 0..5 {
            for j in 0..3 {
                assert_eq!(
                    t.get(j, i),
                    m.get(i, j),
                    "transpose value mismatch at (i={}, j={}): t.get({},{})={:?}, m.get({},{})={:?}",
                    i, j, j, i, t.get(j, i), i, j, m.get(i, j)
                );
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn test_proptest_transpose_roundtrip_random_shapes(
            rows in 0usize..130,
            cols in 0usize..130,
            seed in 0u64..u64::MAX,
        ) {
            let n = rows * cols;
            let data: Vec<Fp<3>> = (0..n)
                .map(|k| {
                    let h = seed.wrapping_mul(6364136223846793005)
                        .wrapping_add(k as u64)
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    Fp::<3>::new(h % 3)
                })
                .collect();
            let m = Bipedal3Matrix::from_row_major(&data, rows, cols);
            let tt = m.transpose().transpose();
            prop_assert_eq!(m, tt, "transpose roundtrip failed for {}x{}", rows, cols);
        }
    }
}
