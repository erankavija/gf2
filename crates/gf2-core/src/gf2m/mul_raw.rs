/// Schoolbook GF(2^m) multiplication: `a * b mod primitive_poly`.
///
/// Pure function operating on `u64` values — no allocations, no trait dispatch,
/// no `self`. Monomorphized to `u64` for formal verification via Charon/Aeneas.
///
/// # Arguments
///
/// * `a` - First operand, must be < 2^m
/// * `b` - Second operand, must be < 2^m
/// * `m` - Extension degree (1..=63)
/// * `primitive_poly` - The primitive polynomial (degree-m term included)
///
/// # Panics
///
/// Panics if `m = 0` (subtraction underflow on `m - 1`) or
/// `m ≥ 64` (shift overflow on `1u64 << m`). These overflow panics
/// serve as runtime guards enforcing the valid range `1..=63`.
///
/// # Complexity
///
/// O(m) bitwise operations.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::mul_raw::gf2m_mul_raw;
///
/// // GF(2^4) with primitive polynomial x^4 + x + 1 = 0b10011
/// let result = gf2m_mul_raw(0b0011, 0b0101, 4, 0b10011);
/// assert_eq!(result, 0b1111); // (x+1) * (x^2+1) = x^3+x^2+x+1 in GF(2^4)
/// ```
pub fn gf2m_mul_raw(a: u64, b: u64, m: usize, primitive_poly: u64) -> u64 {
    if a == 0 || b == 0 {
        return 0;
    }

    let mut result: u64 = 0;
    let mut temp: u64 = a;
    let mut i: usize = 0;

    while i < m {
        if (b >> i) & 1 != 0 {
            result ^= temp;
        }

        let will_overflow = (temp >> (m - 1)) & 1 != 0;
        temp <<= 1;

        if will_overflow {
            temp ^= primitive_poly;
        }

        i += 1;
    }

    result & ((1u64 << m) - 1)
}

/// GF(2^m) addition: `a + b` in GF(2^m) is simply bitwise XOR.
///
/// In fields of characteristic 2, addition is XOR. No reduction is needed:
/// XOR of two m-bit values is at most m bits.
///
/// # Arguments
///
/// * `a` - First operand
/// * `b` - Second operand
///
/// # Complexity
///
/// O(1) — single XOR instruction.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::mul_raw::gf2m_add_raw;
///
/// // In GF(2^m), addition is XOR regardless of m
/// assert_eq!(gf2m_add_raw(0b1010, 0b0110), 0b1100);
/// assert_eq!(gf2m_add_raw(0b1111, 0b1111), 0); // a + a = 0
/// ```
pub fn gf2m_add_raw(a: u64, b: u64) -> u64 {
    a ^ b
}

/// Square-and-multiply exponentiation in GF(2^m).
///
/// Computes `base^exp mod primitive_poly` using repeated squaring.
///
/// # Arguments
///
/// * `base` - Base element, must be < 2^m
/// * `exp` - Exponent (arbitrary u64)
/// * `m` - Extension degree (1..=63)
/// * `primitive_poly` - The primitive polynomial (degree-m term included)
///
/// # Panics
///
/// Panics if `m ≥ 64` (delegated to `gf2m_mul_raw`).
///
/// # Complexity
///
/// O(m · log(exp)) bitwise operations.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::mul_raw::gf2m_pow_raw;
///
/// // GF(2^4) with p(x) = x^4 + x + 1
/// let alpha = 0b0010; // x (primitive element)
/// assert_eq!(gf2m_pow_raw(alpha, 0, 4, 0b10011), 1); // x^0 = 1
/// assert_eq!(gf2m_pow_raw(alpha, 1, 4, 0b10011), alpha); // x^1 = x
/// assert_eq!(gf2m_pow_raw(alpha, 15, 4, 0b10011), 1); // x^15 = 1 (order of GF(16)*)
/// ```
pub fn gf2m_pow_raw(mut base: u64, mut exp: u64, m: usize, primitive_poly: u64) -> u64 {
    let mut result: u64 = 1;
    while exp > 0 {
        if exp & 1 != 0 {
            result = gf2m_mul_raw(result, base, m, primitive_poly);
        }
        exp >>= 1;
        if exp > 0 {
            base = gf2m_mul_raw(base, base, m, primitive_poly);
        }
    }
    result
}

/// Multiplicative inverse in GF(2^m) via Fermat's little theorem.
///
/// Computes `a^(-1) = a^(2^m - 2)` since `a^(2^m - 1) = 1` for all
/// nonzero elements of GF(2^m). Returns 0 for zero input (which has
/// no multiplicative inverse).
///
/// # Arguments
///
/// * `a` - Element to invert, must be < 2^m
/// * `m` - Extension degree (1..=63)
/// * `primitive_poly` - The primitive polynomial (degree-m term included)
///
/// # Panics
///
/// Panics if `m ≥ 64` (delegated to `gf2m_pow_raw`).
///
/// # Complexity
///
/// O(m³) bitwise operations (m squarings of O(m) each, times O(m) per mul).
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::mul_raw::{gf2m_inverse_raw, gf2m_mul_raw};
///
/// // GF(2^4) with p(x) = x^4 + x + 1
/// let a = 0b0011; // x + 1
/// let inv = gf2m_inverse_raw(a, 4, 0b10011);
/// assert_eq!(gf2m_mul_raw(a, inv, 4, 0b10011), 1); // a * a^(-1) = 1
/// ```
pub fn gf2m_inverse_raw(a: u64, m: usize, primitive_poly: u64) -> u64 {
    if a == 0 {
        return 0;
    }
    // a^(-1) = a^(2^m - 2) by Fermat's little theorem in GF(2^m)
    let exp = (1u64 << m) - 2;
    gf2m_pow_raw(a, exp, m, primitive_poly)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gf2m_mul_raw_zero() {
        // 0 * anything = 0
        assert_eq!(gf2m_mul_raw(0, 0b0101, 4, 0b10011), 0);
        assert_eq!(gf2m_mul_raw(0b0011, 0, 4, 0b10011), 0);
        assert_eq!(gf2m_mul_raw(0, 0, 4, 0b10011), 0);
    }

    #[test]
    fn test_gf2m_mul_raw_identity() {
        // a * 1 = a
        assert_eq!(gf2m_mul_raw(0b0011, 1, 4, 0b10011), 0b0011);
        assert_eq!(gf2m_mul_raw(1, 0b0101, 4, 0b10011), 0b0101);
    }

    #[test]
    fn test_gf2m_mul_raw_gf16() {
        // GF(2^4) with p(x) = x^4 + x + 1
        // x * x = x^2
        assert_eq!(gf2m_mul_raw(0b0010, 0b0010, 4, 0b10011), 0b0100);
        // x * x^3 = x^4 = x + 1 (reduced by p(x))
        assert_eq!(gf2m_mul_raw(0b0010, 0b1000, 4, 0b10011), 0b0011);
    }

    #[test]
    fn test_gf2m_mul_raw_commutative() {
        let a = 0b0111;
        let b = 0b1011;
        assert_eq!(
            gf2m_mul_raw(a, b, 4, 0b10011),
            gf2m_mul_raw(b, a, 4, 0b10011)
        );
    }

    #[test]
    fn test_gf2m_mul_raw_exhaustive_gf16() {
        // Verify against table-based multiplication from the main field impl.
        // Build GF(2^4) and compare all 16*16 products.
        let field = crate::gf2m::Gf2mField::new(4, 0b10011).with_tables();
        for a_val in 0..16u64 {
            for b_val in 0..16u64 {
                let elem_a = field.element(a_val);
                let elem_b = field.element(b_val);
                let expected = (&elem_a * &elem_b).value();
                let got = gf2m_mul_raw(a_val, b_val, 4, 0b10011);
                assert_eq!(got, expected, "mismatch at a={a_val}, b={b_val}");
            }
        }
    }

    #[test]
    fn test_gf2m_mul_raw_gf8() {
        // GF(2^3) with p(x) = x^3 + x + 1 = 0b1011
        // x * x = x^2
        assert_eq!(gf2m_mul_raw(0b010, 0b010, 3, 0b1011), 0b100);
        // x * x^2 = x^3 = x + 1
        assert_eq!(gf2m_mul_raw(0b010, 0b100, 3, 0b1011), 0b011);
        // (x+1) * (x+1) = x^2 + 1
        assert_eq!(gf2m_mul_raw(0b011, 0b011, 3, 0b1011), 0b101);
    }

    #[test]
    fn test_gf2m_mul_raw_m1() {
        // GF(2^1) = GF(2) with p(x) = x + 1 = 0b11 (m=1, word boundary)
        assert_eq!(gf2m_mul_raw(0, 0, 1, 0b11), 0);
        assert_eq!(gf2m_mul_raw(0, 1, 1, 0b11), 0);
        assert_eq!(gf2m_mul_raw(1, 0, 1, 0b11), 0);
        assert_eq!(gf2m_mul_raw(1, 1, 1, 0b11), 1); // 1 * 1 = 1 in GF(2)
    }

    #[test]
    fn test_gf2m_mul_raw_m63() {
        // m = 63 (maximum valid extension degree for u64)
        // p(x) = x^63 + x + 1 (a known primitive polynomial)
        let poly: u64 = (1u64 << 63) | 0b11;
        // 1 * 1 = 1
        assert_eq!(gf2m_mul_raw(1, 1, 63, poly), 1);
        // x * 1 = x
        assert_eq!(gf2m_mul_raw(2, 1, 63, poly), 2);
        // 1 * x = x
        assert_eq!(gf2m_mul_raw(1, 2, 63, poly), 2);
        // x * x = x^2
        assert_eq!(gf2m_mul_raw(2, 2, 63, poly), 4);
        // 0 * anything = 0
        assert_eq!(gf2m_mul_raw(0, (1u64 << 62) | 1, 63, poly), 0);
        // Result is always < 2^63
        let a = (1u64 << 62) | 0b101;
        let b = (1u64 << 61) | 0b11;
        let result = gf2m_mul_raw(a, b, 63, poly);
        assert!(result < (1u64 << 63), "result {result} should be < 2^63");
    }

    #[test]
    #[should_panic]
    fn test_gf2m_mul_raw_m64_panics() {
        // m = 64 is invalid: 1u64 << 64 overflows
        gf2m_mul_raw(1, 1, 64, 0);
    }

    #[test]
    #[should_panic]
    fn test_gf2m_mul_raw_m65_panics() {
        // m = 65 is invalid: 1u64 << 65 overflows
        gf2m_mul_raw(1, 1, 65, 0);
    }

    #[test]
    fn test_gf2m_add_raw_basic() {
        // Commutativity
        assert_eq!(gf2m_add_raw(0b1010, 0b0110), gf2m_add_raw(0b0110, 0b1010));
        // a + a = 0 in GF(2)
        assert_eq!(gf2m_add_raw(0b1111, 0b1111), 0);
        // a + 0 = a
        assert_eq!(gf2m_add_raw(0b1010, 0), 0b1010);
        // 0 + a = a
        assert_eq!(gf2m_add_raw(0, 0b0101), 0b0101);
        // 0 + 0 = 0
        assert_eq!(gf2m_add_raw(0, 0), 0);
    }

    #[test]
    fn test_gf2m_add_raw_associative() {
        let a = 0b1101u64;
        let b = 0b1011u64;
        let c = 0b0110u64;
        assert_eq!(
            gf2m_add_raw(gf2m_add_raw(a, b), c),
            gf2m_add_raw(a, gf2m_add_raw(b, c))
        );
    }

    #[test]
    fn test_gf2m_pow_raw_basic() {
        // GF(2^4) with p(x) = x^4 + x + 1 = 0b10011
        let poly = 0b10011u64;
        let alpha = 0b0010u64; // x (primitive element)

        // a^0 = 1
        assert_eq!(gf2m_pow_raw(alpha, 0, 4, poly), 1);
        // a^1 = a
        assert_eq!(gf2m_pow_raw(alpha, 1, 4, poly), alpha);
        // x^2 = 0b0100
        assert_eq!(gf2m_pow_raw(alpha, 2, 4, poly), 0b0100);
        // x^4 = x + 1 = 0b0011 (reduced by p(x))
        assert_eq!(gf2m_pow_raw(alpha, 4, 4, poly), 0b0011);
        // x^15 = 1 (order of GF(2^4)*)
        assert_eq!(gf2m_pow_raw(alpha, 15, 4, poly), 1);
        // 1^anything = 1
        assert_eq!(gf2m_pow_raw(1, 42, 4, poly), 1);
        // 0^0 = 1 by convention (loop doesn't execute)
        assert_eq!(gf2m_pow_raw(0, 0, 4, poly), 1);
        // 0^n = 0 for n > 0
        assert_eq!(gf2m_pow_raw(0, 5, 4, poly), 0);
    }

    #[test]
    fn test_gf2m_pow_raw_exhaustive_gf16_order() {
        // Every nonzero element of GF(2^4) has multiplicative order dividing 15
        let poly = 0b10011u64;
        for a in 1..16u64 {
            assert_eq!(gf2m_pow_raw(a, 15, 4, poly), 1, "a^15 != 1 for a={a}");
        }
    }

    #[test]
    fn test_gf2m_inverse_raw_zero() {
        assert_eq!(gf2m_inverse_raw(0, 4, 0b10011), 0);
    }

    #[test]
    fn test_gf2m_inverse_raw_exhaustive_gf16() {
        // For all nonzero in GF(2^4): a * inverse(a) = 1
        let poly = 0b10011u64;
        for a in 1..16u64 {
            let inv = gf2m_inverse_raw(a, 4, poly);
            assert!(
                inv > 0 && inv < 16,
                "inverse not in field for a={a}: inv={inv}"
            );
            assert_eq!(
                gf2m_mul_raw(a, inv, 4, poly),
                1,
                "a * inverse(a) != 1 for a={a}"
            );
        }
    }

    #[test]
    fn test_gf2m_inverse_raw_involution() {
        // inverse(inverse(a)) = a for all nonzero in GF(2^4)
        let poly = 0b10011u64;
        for a in 1..16u64 {
            let inv = gf2m_inverse_raw(a, 4, poly);
            let inv_inv = gf2m_inverse_raw(inv, 4, poly);
            assert_eq!(inv_inv, a, "inverse(inverse({a})) != {a}");
        }
    }

    #[test]
    fn test_gf2m_inverse_raw_gf8() {
        // GF(2^3) with p(x) = x^3 + x + 1 = 0b1011
        let poly = 0b1011u64;
        for a in 1..8u64 {
            let inv = gf2m_inverse_raw(a, 3, poly);
            assert_eq!(
                gf2m_mul_raw(a, inv, 3, poly),
                1,
                "inverse failed in GF(8) for a={a}"
            );
        }
    }

    #[test]
    fn test_gf2m_inverse_raw_m1() {
        // GF(2): only nonzero element is 1, inverse(1) = 1
        assert_eq!(gf2m_inverse_raw(1, 1, 0b11), 1);
        assert_eq!(gf2m_inverse_raw(0, 1, 0b11), 0);
    }
}
