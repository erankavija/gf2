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
/// Does not panic for valid inputs. Behavior is unspecified if `m > 63`.
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
}
