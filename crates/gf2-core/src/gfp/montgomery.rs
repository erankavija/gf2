//! Montgomery multiplication helpers for GF(p).
//!
//! Internal module providing compile-time constant computation and runtime
//! Montgomery reduction (REDC) for prime field arithmetic. All functions
//! require P > 2 and P odd; the P = 2 special case is handled in the
//! parent module.

/// Compile-time Montgomery constants for a prime modulus P.
///
/// These constants are computed at compile time via const evaluation
/// and enable efficient Montgomery-form arithmetic where R = 2^64.
pub(super) struct MontConsts<const P: u64>;

impl<const P: u64> MontConsts<P> {
    /// R mod P where R = 2^64. This is the Montgomery form of 1.
    pub const R_MOD_P: u64 = compute_r_mod_p(P);

    /// R^2 mod P. Used for converting canonical to Montgomery form.
    pub const R2_MOD_P: u64 = compute_r2_mod_p(P);

    /// -P^{-1} mod 2^64. The REDC reduction constant.
    pub const P_INV: u64 = compute_p_inv(P);
}

/// Compute 2^64 mod p.
///
/// # Complexity
///
/// O(1).
const fn compute_r_mod_p(p: u64) -> u64 {
    ((1u128 << 64) % p as u128) as u64
}

/// Compute R^2 mod p where R = 2^64 mod p.
///
/// # Complexity
///
/// O(1).
const fn compute_r2_mod_p(p: u64) -> u64 {
    let r = compute_r_mod_p(p) as u128;
    ((r * r) % p as u128) as u64
}

/// Compute -P^{-1} mod 2^64 via Hensel lifting.
///
/// Requires P to be odd. Starts with inv = 1 (correct mod 2),
/// then doubles the number of correct bits six times to reach 64.
///
/// # Complexity
///
/// O(1) (6 iterations).
const fn compute_p_inv(p: u64) -> u64 {
    let mut inv: u64 = 1;
    let mut i = 0;
    while i < 6 {
        inv = inv.wrapping_mul(2u64.wrapping_sub(p.wrapping_mul(inv)));
        i += 1;
    }
    // inv = P^{-1} mod 2^64; negate to get -P^{-1} mod 2^64
    inv.wrapping_neg()
}

/// Montgomery reduction: compute t * R^{-1} mod P.
///
/// Input: t < P * R (satisfied for products of Montgomery-form elements).
/// Output: result in [0, P).
///
/// Uses branchless final subtraction for constant-time execution.
///
/// # Complexity
///
/// O(1).
#[inline]
pub(super) fn redc<const P: u64>(t: u128) -> u64 {
    let t_lo = t as u64;
    let m = t_lo.wrapping_mul(MontConsts::<P>::P_INV);
    let mp = m as u128 * P as u128;
    let u = ((t + mp) >> 64) as u64;
    // Branchless: if u >= P then u - P else u
    let (result, borrow) = u.overflowing_sub(P);
    let correction = (borrow as u64).wrapping_neg() & P;
    result.wrapping_add(correction)
}

/// Convert canonical form to Montgomery form: a -> aR mod P.
///
/// # Complexity
///
/// O(1).
#[inline]
pub(super) fn to_mont<const P: u64>(a: u64) -> u64 {
    redc::<P>(a as u128 * MontConsts::<P>::R2_MOD_P as u128)
}

/// Convert Montgomery form to canonical form: aR -> a mod P.
///
/// # Complexity
///
/// O(1).
#[inline]
pub(super) fn from_mont<const P: u64>(a: u64) -> u64 {
    redc::<P>(a as u128)
}

/// Branchless modular addition in [0, P).
///
/// # Complexity
///
/// O(1).
#[inline]
pub(super) fn mont_add<const P: u64>(a: u64, b: u64) -> u64 {
    let sum = a + b;
    let (result, borrow) = sum.overflowing_sub(P);
    let correction = (borrow as u64).wrapping_neg() & P;
    result.wrapping_add(correction)
}

/// Branchless modular subtraction in [0, P).
///
/// # Complexity
///
/// O(1).
#[inline]
pub(super) fn mont_sub<const P: u64>(a: u64, b: u64) -> u64 {
    let (result, borrow) = a.overflowing_sub(b);
    let correction = (borrow as u64).wrapping_neg() & P;
    result.wrapping_add(correction)
}

/// Modular exponentiation in Montgomery domain via square-and-multiply.
///
/// Input: base in Montgomery form, exp in canonical form.
/// Output: base^exp in Montgomery form.
///
/// # Complexity
///
/// O(log exp) Montgomery multiplications.
#[inline]
pub(super) fn mod_pow_mont<const P: u64>(mut base: u64, mut exp: u64) -> u64 {
    let mut result = MontConsts::<P>::R_MOD_P; // Montgomery form of 1
    while exp > 0 {
        if exp & 1 == 1 {
            result = redc::<P>(result as u128 * base as u128);
        }
        exp >>= 1;
        if exp > 0 {
            base = redc::<P>(base as u128 * base as u128);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const MERSENNE_61: u64 = (1u64 << 61) - 1;

    // -----------------------------------------------------------------------
    // Constants verification
    // -----------------------------------------------------------------------

    #[test]
    fn test_r_mod_p_known_values() {
        // 2 ≡ -1 mod 3, so 2^64 ≡ 1 mod 3
        assert_eq!(MontConsts::<3>::R_MOD_P, 1);
        // 2^4 ≡ 1 mod 5, so 2^64 = (2^4)^16 ≡ 1 mod 5
        assert_eq!(MontConsts::<5>::R_MOD_P, 1);
        // 2^3 ≡ 1 mod 7, 2^64 = 2^(3*21+1) ≡ 2 mod 7
        assert_eq!(MontConsts::<7>::R_MOD_P, 2);
        // 2^16 ≡ -1 mod 65537, 2^32 ≡ 1, 2^64 ≡ 1
        assert_eq!(MontConsts::<65537>::R_MOD_P, 1);
        // 2^64 = 8 * 2^61 ≡ 8 mod (2^61 - 1)
        assert_eq!(MontConsts::<MERSENNE_61>::R_MOD_P, 8);
    }

    #[test]
    fn test_r_mod_p_matches_direct_computation() {
        fn check<const P: u64>() {
            let expected = ((1u128 << 64) % P as u128) as u64;
            assert_eq!(MontConsts::<P>::R_MOD_P, expected, "P={P}");
        }
        check::<3>();
        check::<5>();
        check::<7>();
        check::<11>();
        check::<13>();
        check::<65537>();
        check::<MERSENNE_61>();
    }

    #[test]
    fn test_r2_mod_p_known_values() {
        assert_eq!(MontConsts::<3>::R2_MOD_P, 1); // 1^2 mod 3
        assert_eq!(MontConsts::<5>::R2_MOD_P, 1); // 1^2 mod 5
        assert_eq!(MontConsts::<7>::R2_MOD_P, 4); // 2^2 mod 7
        assert_eq!(MontConsts::<65537>::R2_MOD_P, 1); // 1^2 mod 65537
        assert_eq!(MontConsts::<MERSENNE_61>::R2_MOD_P, 64); // 8^2 mod (2^61-1)
    }

    #[test]
    fn test_p_inv_identity() {
        // Verify P * P_INV ≡ -1 mod 2^64 (wrapping product = u64::MAX)
        fn check<const P: u64>() {
            let product = P.wrapping_mul(MontConsts::<P>::P_INV);
            assert_eq!(product, u64::MAX, "P={P}: P * P_INV should be 2^64 - 1");
        }
        check::<3>();
        check::<5>();
        check::<7>();
        check::<11>();
        check::<13>();
        check::<17>();
        check::<65537>();
        check::<MERSENNE_61>();
    }

    // -----------------------------------------------------------------------
    // REDC and conversion
    // -----------------------------------------------------------------------

    #[test]
    fn test_round_trip_small_primes() {
        fn check_all<const P: u64>() {
            for a in 0..P {
                let mont = to_mont::<P>(a);
                let back = from_mont::<P>(mont);
                assert_eq!(back, a, "P={P}, a={a}: round-trip failed");
            }
        }
        check_all::<3>();
        check_all::<5>();
        check_all::<7>();
        check_all::<11>();
        check_all::<13>();
    }

    #[test]
    fn test_to_mont_is_a_times_r_mod_p() {
        fn check_all<const P: u64>() {
            for a in 0..P {
                let mont = to_mont::<P>(a);
                let expected = ((a as u128 * (1u128 << 64)) % P as u128) as u64;
                assert_eq!(mont, expected, "P={P}, a={a}: Montgomery form mismatch");
            }
        }
        check_all::<3>();
        check_all::<5>();
        check_all::<7>();
        check_all::<13>();
    }

    #[test]
    fn test_zero_maps_to_zero() {
        assert_eq!(to_mont::<3>(0), 0);
        assert_eq!(to_mont::<7>(0), 0);
        assert_eq!(to_mont::<65537>(0), 0);
        assert_eq!(to_mont::<MERSENNE_61>(0), 0);
    }

    #[test]
    fn test_one_maps_to_r_mod_p() {
        assert_eq!(to_mont::<3>(1), MontConsts::<3>::R_MOD_P);
        assert_eq!(to_mont::<7>(1), MontConsts::<7>::R_MOD_P);
        assert_eq!(to_mont::<65537>(1), MontConsts::<65537>::R_MOD_P);
        assert_eq!(
            to_mont::<MERSENNE_61>(1),
            MontConsts::<MERSENNE_61>::R_MOD_P
        );
    }

    // -----------------------------------------------------------------------
    // Arithmetic
    // -----------------------------------------------------------------------

    #[test]
    fn test_mont_add_exhaustive() {
        fn check_all<const P: u64>() {
            for a in 0..P {
                for b in 0..P {
                    let result = mont_add::<P>(a, b);
                    let expected = (a + b) % P;
                    assert_eq!(result, expected, "P={P}: {a} + {b}");
                }
            }
        }
        check_all::<3>();
        check_all::<5>();
        check_all::<7>();
    }

    #[test]
    fn test_mont_sub_exhaustive() {
        fn check_all<const P: u64>() {
            for a in 0..P {
                for b in 0..P {
                    let result = mont_sub::<P>(a, b);
                    let expected = (a + P - b) % P;
                    assert_eq!(result, expected, "P={P}: {a} - {b}");
                }
            }
        }
        check_all::<3>();
        check_all::<5>();
        check_all::<7>();
    }

    #[test]
    fn test_mont_mul_exhaustive() {
        fn check_all<const P: u64>() {
            for a in 0..P {
                for b in 0..P {
                    let a_mont = to_mont::<P>(a);
                    let b_mont = to_mont::<P>(b);
                    let product_mont = redc::<P>(a_mont as u128 * b_mont as u128);
                    let product = from_mont::<P>(product_mont);
                    let expected = ((a as u128 * b as u128) % P as u128) as u64;
                    assert_eq!(product, expected, "P={P}: {a} * {b}");
                }
            }
        }
        check_all::<3>();
        check_all::<5>();
        check_all::<7>();
    }

    #[test]
    fn test_mod_pow_mont_basic() {
        // 3^5 mod 7 = 243 mod 7 = 5
        let base = to_mont::<7>(3);
        let result = mod_pow_mont::<7>(base, 5);
        assert_eq!(from_mont::<7>(result), 5);

        // 2^10 mod 1000003 = 1024
        let base = to_mont::<1000003>(2);
        let result = mod_pow_mont::<1000003>(base, 10);
        assert_eq!(from_mont::<1000003>(result), 1024);
    }

    #[test]
    fn test_mod_pow_mont_fermat() {
        // Fermat's little theorem: a^(p-1) ≡ 1 mod p for a != 0
        for a in 1..7u64 {
            let a_mont = to_mont::<7>(a);
            let result = mod_pow_mont::<7>(a_mont, 6);
            assert_eq!(from_mont::<7>(result), 1, "7: {a}^6 should be 1");
        }
    }

    #[test]
    fn test_mod_pow_mont_inversion() {
        // inv(a) = a^(P-2) must satisfy a * inv(a) ≡ 1
        fn check_all<const P: u64>() {
            for a in 1..P {
                let a_mont = to_mont::<P>(a);
                let inv_mont = mod_pow_mont::<P>(a_mont, P - 2);
                let product_mont = redc::<P>(a_mont as u128 * inv_mont as u128);
                assert_eq!(
                    product_mont,
                    MontConsts::<P>::R_MOD_P,
                    "P={P}: {a} * inv({a}) should be 1 in Montgomery form"
                );
            }
        }
        check_all::<3>();
        check_all::<5>();
        check_all::<7>();
        check_all::<11>();
    }

    // -----------------------------------------------------------------------
    // Larger prime tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mersenne_61_round_trip() {
        let values = [0, 1, 2, 1000, MERSENNE_61 - 1, MERSENNE_61 / 2];
        for &a in &values {
            let mont = to_mont::<MERSENNE_61>(a);
            let back = from_mont::<MERSENNE_61>(mont);
            assert_eq!(back, a, "Mersenne-61 round-trip failed for {a}");
        }
    }

    #[test]
    fn test_mersenne_61_mul() {
        let a = 123_456_789u64;
        let b = 987_654_321u64;
        let expected = ((a as u128 * b as u128) % MERSENNE_61 as u128) as u64;
        let a_mont = to_mont::<MERSENNE_61>(a);
        let b_mont = to_mont::<MERSENNE_61>(b);
        let product_mont = redc::<MERSENNE_61>(a_mont as u128 * b_mont as u128);
        assert_eq!(from_mont::<MERSENNE_61>(product_mont), expected);
    }

    #[test]
    fn test_mersenne_61_inversion() {
        let a = 42u64;
        let a_mont = to_mont::<MERSENNE_61>(a);
        let inv_mont = mod_pow_mont::<MERSENNE_61>(a_mont, MERSENNE_61 - 2);
        let product_mont = redc::<MERSENNE_61>(a_mont as u128 * inv_mont as u128);
        assert_eq!(
            product_mont,
            MontConsts::<MERSENNE_61>::R_MOD_P,
            "42 * inv(42) should be 1 in Montgomery form"
        );
    }

    #[test]
    fn test_65537_mul_cross_check() {
        // Cross-check a few multiplications against naive
        let pairs = [(100, 200), (65536, 65536), (1, 65536), (12345, 54321)];
        for (a, b) in pairs {
            let expected = ((a as u128 * b as u128) % 65537u128) as u64;
            let a_mont = to_mont::<65537>(a);
            let b_mont = to_mont::<65537>(b);
            let product_mont = redc::<65537>(a_mont as u128 * b_mont as u128);
            assert_eq!(
                from_mont::<65537>(product_mont),
                expected,
                "{a} * {b} mod 65537"
            );
        }
    }
}
