//! Barrett reduction for GF(2^m) polynomial arithmetic.
//!
//! Barrett reduction replaces the standard shift-and-XOR reduction loop with a
//! precomputed-reciprocal approach. Given the irreducible polynomial P(x) of degree m,
//! the Barrett constant `mu = x^(2m) / P(x)` is precomputed once. Reduction of a
//! product c(x) of degree ≤ 2(m-1) then requires two carry-less multiplications
//! and a possible single correction, rather than an O(m) loop of conditional XORs.
//!
//! All arithmetic here is over GF(2): addition is XOR, and multiplication is
//! carry-less (no carries propagate between bit positions).
//!
//! # Width limitation (`m <= 63`)
//!
//! **This module deliberately caps the supported degree at `m <= 63`.**
//!
//! The implementation represents both the Barrett constant
//! `mu = x^(2m) / P(x)` (up to `m+1` bits) and the dividend `x^(2m)` (up to
//! `2m + 1` bits) in a single `u128`, which caps `2m + 1 <= 128`, i.e.
//! `m <= 63`. Extending Barrett to `m = 64..=127` requires true 256-bit
//! intermediate arithmetic:
//!
//! - the product `c(x)` has degree up to `2m - 2` (252 bits at `m = 127`);
//! - the Barrett constant `mu` has degree up to `m` (up to 128 bits);
//! - the two carry-less multiplications `c_high * mu` and `q * P` then
//!   produce 256-bit intermediates.
//!
//! That widening is deliberately deferred to the multi-word GF(2^m) story
//! (JIT issue `6fb4abad`). Until that lands, [`crate::gf2m::Gf2mField_`]
//! over `u128` storage transparently falls back to the generic schoolbook
//! primitive for `m >= 64`, so correctness is preserved — only the
//! PCLMULQDQ + Barrett fast path is unavailable at those degrees.

/// Carry-less multiplication of two GF(2) polynomials.
///
/// Computes the product `a(x) * b(x)` over GF(2), where each bit of `a` and `b`
/// represents a coefficient. The result can have degree up to `deg(a) + deg(b)`,
/// fitting in a `u128`.
///
/// # Arguments
///
/// * `a` - First polynomial (up to 64 bits).
/// * `b` - Second polynomial (up to 64 bits).
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::barrett::clmul;
///
/// // (x + 1) * (x + 1) = x^2 + 1  (no carry: x + x = 0 in GF(2))
/// assert_eq!(clmul(0b11, 0b11), 0b101);
///
/// // x * x = x^2
/// assert_eq!(clmul(0b10, 0b10), 0b100);
/// ```
///
/// # Complexity
///
/// O(n) where n is the number of set bits in `b`.
pub fn clmul(a: u64, b: u64) -> u128 {
    let a = a as u128;
    let mut result: u128 = 0;
    let mut b_remaining = b;
    while b_remaining != 0 {
        let bit = b_remaining.trailing_zeros();
        result ^= a << bit;
        b_remaining &= b_remaining - 1; // clear lowest set bit
    }
    result
}

/// Carry-less multiplication of two `u128` GF(2) polynomials, returning a `u128`.
///
/// This is a truncating variant — the caller must ensure the result fits in 128 bits
/// (i.e., `deg(a) + deg(b) < 128`). Used internally for Barrett reduction steps
/// where operand degrees are bounded.
fn clmul128_trunc(a: u128, b: u128) -> u128 {
    let mut result: u128 = 0;
    let mut b_remaining = b;
    while b_remaining != 0 {
        let bit = b_remaining.trailing_zeros();
        // Only shift if bit < 128 (trailing_zeros returns 128 for zero, but loop guards against that)
        result ^= a << bit;
        b_remaining &= b_remaining.wrapping_sub(1);
    }
    result
}

/// Precomputed Barrett reduction constants for a specific irreducible polynomial.
///
/// Barrett reduction converts the modular reduction step of GF(2^m) multiplication
/// from an O(m) conditional-XOR loop into two carry-less multiplications plus a
/// possible single correction. The tradeoff is worthwhile when reducing many
/// products by the same modulus (e.g., during field multiplication tables or
/// repeated arithmetic).
///
/// # Width limitation (`degree <= 63`)
///
/// **Warning:** This reducer is restricted to `degree <= 63` and will panic
/// in [`BarrettReducer::new`] for any larger degree. The restriction exists
/// because both the Barrett constant `mu = x^(2m) / P(x)` and the dividend
/// `x^(2m)` are stored in a single `u128`, capping `2m` at 128 bits.
///
/// The SIMD dispatch in [`crate::gf2m::Gf2mField_`] mirrors that cap —
/// Barrett is only wired in when the backing type is `u64`. Extending
/// Barrett to `m = 64..=127` requires 256-bit intermediate arithmetic
/// (see the module-level docs) and is deliberately deferred to a later
/// wider-SIMD story (tracked as JIT issue `6fb4abad`). For u128-backed
/// fields at `m >= 64`, `Gf2mField_<u128>` transparently falls back to
/// the generic schoolbook primitive, so correctness is preserved — only
/// the PCLMULQDQ + Barrett fast path is unavailable at those degrees.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::barrett::BarrettReducer;
///
/// // GF(2^8) with AES polynomial x^8 + x^4 + x^3 + x^2 + 1 = 0x11B
/// let reducer = BarrettReducer::new(0x11B, 8);
///
/// // Reduce a product back to the field
/// let product: u128 = 0x1234; // some 16-bit polynomial
/// let reduced = reducer.reduce(product);
/// assert!(reduced < 256); // result fits in 8 bits
/// ```
///
/// # Panics
///
/// Panics if `degree` is 0 or greater than 63, or if the leading coefficient
/// of `irreducible_poly` is not at position `degree`.
#[derive(Debug)]
pub struct BarrettReducer {
    /// The irreducible polynomial P(x), degree m.
    modulus: u128,
    /// The Barrett constant mu = x^(2m) / P(x), a polynomial of degree m.
    mu: u128,
    /// The field degree m.
    degree: u32,
}

impl BarrettReducer {
    /// Precompute Barrett constants for the given irreducible polynomial.
    ///
    /// Computes `mu = x^(2m) / P(x)` via polynomial long division over GF(2).
    ///
    /// # Arguments
    ///
    /// * `irreducible_poly` - The irreducible polynomial P(x) as a bitmask.
    ///   Bit `i` represents the coefficient of x^i. Must have degree exactly `degree`.
    /// * `degree` - The degree m of the irreducible polynomial.
    ///
    /// # Panics
    ///
    /// Panics if `degree` is 0 or greater than 63, or if the polynomial does not
    /// have its leading bit at position `degree`. The upper bound of 63 is a
    /// deliberate contract, not a bug — see the struct-level and module-level
    /// docs for the 256-bit-arithmetic reasoning, and JIT issue `6fb4abad`
    /// for the planned extension to `m = 64..=127`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducer;
    ///
    /// // x^4 + x + 1 = 0b10011 for GF(2^4)
    /// let reducer = BarrettReducer::new(0b10011, 4);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(m) for the polynomial long division.
    pub fn new(irreducible_poly: u128, degree: u32) -> Self {
        assert!(degree > 0 && degree <= 63, "degree must be in 1..=63");
        assert_eq!(
            irreducible_poly >> degree,
            1,
            "polynomial must have leading bit at position {degree}"
        );

        // Compute mu = x^(2m) / P(x) via polynomial long division over GF(2).
        // Dividend is x^(2m) = 1 << (2*m). We divide by P(x).
        let m = degree;
        let p = irreducible_poly;

        // Long division: process bits from degree 2m down to degree m.
        // The quotient has degree m.
        let mut remainder: u128 = 1u128 << (2 * m); // x^(2m)
        let mut quotient: u128 = 0;

        // For each bit position from 2m down to m:
        // if the corresponding bit of the remainder is set, set the quotient bit
        // and XOR in P shifted to that position.
        for i in (0..=m).rev() {
            // We're looking at degree (m + i) in the remainder
            let bit_pos = m + i;
            if (remainder >> bit_pos) & 1 == 1 {
                quotient |= 1u128 << i;
                remainder ^= p << i;
            }
        }

        BarrettReducer {
            modulus: p,
            mu: quotient,
            degree: m,
        }
    }

    /// Reduce a polynomial product of degree ≤ 2(m-1) to an m-bit field element.
    ///
    /// Applies Barrett reduction: given `c(x)` with `deg(c) < 2m`, computes
    /// `c(x) mod P(x)` using the precomputed Barrett constant.
    ///
    /// # Arguments
    ///
    /// * `product` - The polynomial to reduce, with degree at most `2m - 2`.
    ///
    /// # Returns
    ///
    /// The remainder `c(x) mod P(x)` as a `u64`, fitting in m bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducer;
    ///
    /// // GF(2^4) with P(x) = x^4 + x + 1
    /// let reducer = BarrettReducer::new(0b10011, 4);
    ///
    /// // Reducing 0 gives 0
    /// assert_eq!(reducer.reduce(0), 0);
    ///
    /// // Reducing a value < 2^m gives itself
    /// assert_eq!(reducer.reduce(0b1010), 0b1010);
    /// ```
    ///
    /// # Complexity
    ///
    /// O(m²) for two carry-less multiplications of m-bit polynomials.
    pub fn reduce(&self, product: u128) -> u64 {
        let m = self.degree;
        let field_mask = (1u128 << m) - 1;

        // If already reduced, return immediately
        if product >> m == 0 {
            return product as u64;
        }

        // Step 1: q = (c >> m) clmul mu >> m
        let c_high = product >> m; // upper bits of c(x), degree ≤ m-2
        let q = clmul128_trunc(c_high, self.mu) >> m;

        // Step 2: r = c XOR (q clmul P)
        let qp = clmul128_trunc(q, self.modulus);
        let r = product ^ qp;

        // Step 3: if deg(r) >= m, correct by XORing with P once
        let mut result = r;
        if result >> m != 0 {
            result ^= self.modulus;
        }
        // One more correction may be needed in edge cases
        if result >> m != 0 {
            result ^= self.modulus;
        }

        (result & field_mask) as u64
    }

    /// Reduce using an externally-provided carry-less multiplication function.
    ///
    /// This allows using SIMD PCLMULQDQ for the two internal carry-less
    /// multiplications instead of the scalar fallback, turning Barrett reduction
    /// from O(m²) into O(1) (two hardware `PCLMULQDQ` instructions).
    ///
    /// # Arguments
    ///
    /// * `product` - The polynomial to reduce, with degree at most `2m - 2`.
    /// * `clmul` - A carry-less multiplication function `(u64, u64) -> u128`.
    ///   Both operands in the Barrett steps fit in `u64` for `m ≤ 63`.
    ///
    /// # Returns
    ///
    /// The remainder `c(x) mod P(x)` as a `u64`, fitting in m bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::{clmul, BarrettReducer};
    ///
    /// // GF(2^4) with P(x) = x^4 + x + 1
    /// let reducer = BarrettReducer::new(0b10011, 4);
    ///
    /// // Use the scalar clmul as the function pointer
    /// let result = reducer.reduce_with_clmul(0b1010, clmul);
    /// assert_eq!(result, 0b1010); // already reduced
    ///
    /// let product = clmul(0b1111, 0b1010); // some GF(2^4) product
    /// let reduced = reducer.reduce_with_clmul(product, clmul);
    /// assert!(reduced < 16); // fits in 4 bits
    /// ```
    ///
    /// # Complexity
    ///
    /// O(1) when `clmul` is a hardware PCLMULQDQ instruction (two multiplications
    /// plus constant-time correction). O(m) when `clmul` is the scalar fallback.
    pub fn reduce_with_clmul(&self, product: u128, clmul: fn(u64, u64) -> u128) -> u64 {
        let m = self.degree;
        let field_mask = (1u128 << m) - 1;

        // If already reduced, return immediately
        if product >> m == 0 {
            return product as u64;
        }

        // Step 1: q = (c >> m) clmul mu >> m
        // c_high has at most m-1 bits, mu has m bits — both fit in u64 for m ≤ 63
        let c_high = (product >> m) as u64;
        let mu = self.mu as u64;
        let q = (clmul(c_high, mu) >> m) as u64;

        // Step 2: r = c XOR (q clmul P)
        // q has at most m bits, modulus has m+1 bits — both fit in u64 for m ≤ 63
        let modulus = self.modulus as u64;
        let qp = clmul(q, modulus);
        let r = product ^ qp;

        // Step 3: if deg(r) >= m, correct by XORing with P (at most twice)
        let mut result = r;
        if result >> m != 0 {
            result ^= self.modulus;
        }
        if result >> m != 0 {
            result ^= self.modulus;
        }

        (result & field_mask) as u64
    }

    /// Returns the precomputed Barrett constant `mu = x^(2m) / P(x)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducer;
    ///
    /// let reducer = BarrettReducer::new(0b111, 2);
    /// // mu = x^4 / (x^2 + x + 1) = x^2 + x + 1 = 0b111
    /// // (since x^4 = (x^2+x+1)(x^2+x+1) + 0 when P divides x^4 evenly...
    /// // actually let's just verify it's computed)
    /// let mu = reducer.mu();
    /// assert!(mu > 0);
    /// ```
    pub fn mu(&self) -> u128 {
        self.mu
    }

    /// Returns the degree m of the irreducible polynomial.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducer;
    ///
    /// let reducer = BarrettReducer::new(0b10011, 4);
    /// assert_eq!(reducer.degree(), 4);
    /// ```
    pub fn degree(&self) -> u32 {
        self.degree
    }

    /// Returns the irreducible polynomial P(x).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducer;
    ///
    /// let reducer = BarrettReducer::new(0b10011, 4);
    /// assert_eq!(reducer.modulus(), 0b10011);
    /// ```
    pub fn modulus(&self) -> u128 {
        self.modulus
    }
}

/// Naive polynomial reduction over GF(2) by repeated subtraction (XOR).
///
/// Used as a reference implementation for testing Barrett reduction correctness.
///
/// # Arguments
///
/// * `product` - The polynomial to reduce.
/// * `modulus` - The irreducible polynomial P(x) of degree `degree`.
/// * `degree` - The degree of the modulus.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::barrett::naive_reduce;
///
/// // Reduce x^5 mod (x^4 + x + 1): x^5 = x*(x^4) = x*(x+1) = x^2 + x
/// // Actually: x^5 XOR (x^4+x+1)<<1 = 0b100000 XOR 0b100110 = 0b000110
/// assert_eq!(naive_reduce(0b100000, 0b10011, 4), 0b0110);
/// ```
///
/// # Complexity
///
/// O(m) shift-and-XOR operations.
pub fn naive_reduce(product: u128, modulus: u128, degree: u32) -> u64 {
    let mut r = product;
    // Find the degree of r
    for bit in (degree..128).rev() {
        if (r >> bit) & 1 == 1 {
            r ^= modulus << (bit - degree);
        }
    }
    r as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive_polys::PrimitivePolynomialDatabase;
    use proptest::prelude::*;

    // ---- Known-value tests ----

    #[test]
    fn test_clmul_identity() {
        // a * 1 = a
        assert_eq!(clmul(0b1010, 1), 0b1010);
        assert_eq!(clmul(1, 0b1010), 0b1010);
    }

    #[test]
    fn test_clmul_zero() {
        assert_eq!(clmul(0, 0xFF), 0);
        assert_eq!(clmul(0xFF, 0), 0);
    }

    #[test]
    fn test_clmul_known_products() {
        // (x+1)*(x+1) = x^2 + 2x + 1 = x^2 + 1 (in GF(2), 2x = 0)
        assert_eq!(clmul(0b11, 0b11), 0b101);
        // x * x = x^2
        assert_eq!(clmul(0b10, 0b10), 0b100);
        // (x^2+1)*(x+1) = x^3 + x^2 + x + 1
        assert_eq!(clmul(0b101, 0b11), 0b1111);
    }

    #[test]
    fn test_clmul_commutative() {
        assert_eq!(clmul(0x1234, 0x5678), clmul(0x5678, 0x1234));
    }

    #[test]
    fn test_barrett_new_gf2_4() {
        // P(x) = x^4 + x + 1 = 0b10011
        let reducer = BarrettReducer::new(0b10011, 4);
        assert_eq!(reducer.degree(), 4);
        assert_eq!(reducer.modulus(), 0b10011);

        // mu = x^8 / (x^4 + x + 1)
        // Verify: mu * P should give x^8 + remainder of degree < 4
        let mu_times_p = clmul128_trunc(reducer.mu(), reducer.modulus());
        // x^(2m) = mu * P + remainder, and remainder has degree < m
        let x_2m: u128 = 1u128 << 8;
        let remainder = x_2m ^ mu_times_p;
        assert!(remainder < (1u128 << 4), "remainder should have degree < m");
    }

    #[test]
    fn test_reduce_zero() {
        let reducer = BarrettReducer::new(0b10011, 4);
        assert_eq!(reducer.reduce(0), 0);
    }

    #[test]
    fn test_reduce_already_reduced() {
        let reducer = BarrettReducer::new(0b10011, 4);
        for v in 0u128..16 {
            assert_eq!(reducer.reduce(v), v as u64);
        }
    }

    #[test]
    fn test_reduce_matches_naive_gf2_4() {
        let poly: u128 = 0b10011;
        let m = 4;
        let reducer = BarrettReducer::new(poly, m);

        // Test all possible products in GF(2^4): product degree ≤ 2*(4-1) = 6, so up to 7 bits
        for product in 0u128..(1 << (2 * m - 1)) {
            let barrett = reducer.reduce(product);
            let naive = naive_reduce(product, poly, m);
            assert_eq!(
                barrett, naive,
                "mismatch for product {product:#b}: barrett={barrett:#b}, naive={naive:#b}"
            );
        }
    }

    #[test]
    fn test_reduce_gf2_8_aes() {
        // GF(2^8) with AES polynomial: x^8 + x^4 + x^3 + x^2 + 1 = 0x11B
        let poly: u128 = 0x11B;
        let m = 8;
        let reducer = BarrettReducer::new(poly, m);

        // Test a sampling of products
        let test_cases: Vec<u128> = vec![
            0, 1, 0xFF, 0x100,  // x^8
            0x1FE,  // near-max for single element
            0x3FFF, // max degree 13 (< 2*8-1=15)
            0x5A5A, 0xAAAA,
        ];
        for product in test_cases {
            let barrett = reducer.reduce(product);
            let naive = naive_reduce(product, poly, m);
            assert_eq!(
                barrett, naive,
                "GF(2^8) mismatch for product {product:#x}: barrett={barrett:#x}, naive={naive:#x}"
            );
        }
    }

    #[test]
    fn test_reduce_max_degree_product() {
        // GF(2^8): maximum product has degree 2*(8-1) = 14
        let poly: u128 = 0x11B;
        let m = 8;
        let reducer = BarrettReducer::new(poly, m);

        // Product with all bits set up to degree 14
        let max_product: u128 = (1u128 << 15) - 1; // 0x7FFF
        let barrett = reducer.reduce(max_product);
        let naive = naive_reduce(max_product, poly, m);
        assert_eq!(barrett, naive);
        assert!(barrett < 256); // must fit in 8 bits
    }

    #[test]
    fn test_barrett_all_primitive_polys() {
        // Test Barrett reduction matches naive for all primitive polynomials m=2..16
        for m in 2u32..=16 {
            let poly = PrimitivePolynomialDatabase::standard(m as usize).unwrap() as u128;
            let reducer = BarrettReducer::new(poly, m);

            // Test a range of products
            let max_product_deg = 2 * m - 2;
            let num_tests = if max_product_deg <= 12 {
                1u128 << (max_product_deg + 1) // exhaustive for small fields
            } else {
                4096 // sample for larger fields
            };

            for product in 0..num_tests {
                let p = if max_product_deg > 12 {
                    // Use a pseudo-random sampling for larger fields
                    // Mix bits to get good coverage
                    let p = product
                        .wrapping_mul(0x9E3779B97F4A7C15)
                        .wrapping_add(product ^ 0xDEAD);
                    p & ((1u128 << (max_product_deg + 1)) - 1)
                } else {
                    product
                };

                let barrett = reducer.reduce(p);
                let naive = naive_reduce(p, poly, m);
                assert_eq!(
                    barrett, naive,
                    "m={m}, product={p:#x}: barrett={barrett:#x}, naive={naive:#x}"
                );
            }
        }
    }

    #[test]
    fn test_barrett_multiplication_roundtrip() {
        // Verify that clmul followed by Barrett reduce gives correct field multiplication
        // in GF(2^8) with AES polynomial
        let poly: u128 = 0x11B;
        let m: u32 = 8;
        let reducer = BarrettReducer::new(poly, m);
        let mask = (1u64 << m) - 1;

        // Multiply all pairs of small elements
        for a in 0u64..16 {
            for b in 0u64..16 {
                let product = clmul(a, b);
                let result = reducer.reduce(product);
                assert!(result <= mask, "result {result:#x} exceeds field size");

                // Verify commutativity
                let product_rev = clmul(b, a);
                let result_rev = reducer.reduce(product_rev);
                assert_eq!(result, result_rev, "commutativity failed for {a} * {b}");
            }
        }
    }

    #[test]
    fn test_naive_reduce_basic() {
        // x^4 mod (x^4 + x + 1) = x + 1 = 0b11
        assert_eq!(naive_reduce(0b10000, 0b10011, 4), 0b0011);

        // x^5 mod (x^4 + x + 1):
        // x^5 = x * x^4 = x * (x+1) = x^2 + x = 0b110
        // Via naive: bit 5 is set, XOR poly<<1 = 0b100110
        // 0b100000 ^ 0b100110 = 0b000110 = 6
        assert_eq!(naive_reduce(0b100000, 0b10011, 4), 0b0110);
    }

    // ---- Property-based tests ----

    proptest! {
        #[test]
        fn test_clmul_commutative_prop(a in 0u64..=0xFFFF, b in 0u64..=0xFFFF) {
            prop_assert_eq!(clmul(a, b), clmul(b, a));
        }

        #[test]
        fn test_clmul_distributive_prop(a in 0u64..=0xFF, b in 0u64..=0xFF, c in 0u64..=0xFF) {
            // a * (b XOR c) = (a * b) XOR (a * c)
            let lhs = clmul(a, b ^ c);
            let rhs = clmul(a, b) ^ clmul(a, c);
            prop_assert_eq!(lhs, rhs);
        }

        #[test]
        fn test_barrett_matches_naive_gf2_8_prop(product in 0u128..0x10000u128) {
            let poly: u128 = 0x11B;
            let m: u32 = 8;
            let reducer = BarrettReducer::new(poly, m);
            let barrett = reducer.reduce(product & ((1u128 << (2 * m - 1)) - 1));
            let naive = naive_reduce(product & ((1u128 << (2 * m - 1)) - 1), poly, m);
            prop_assert_eq!(barrett, naive);
        }

        #[test]
        fn test_barrett_matches_naive_gf2_16_prop(product in 0u128..0x80000000u128) {
            let poly: u128 = 0b10000000000101101; // x^16 + x^5 + x^3 + x^2 + 1
            let m: u32 = 16;
            let reducer = BarrettReducer::new(poly, m);
            let masked = product & ((1u128 << (2 * m - 1)) - 1);
            let barrett = reducer.reduce(masked);
            let naive = naive_reduce(masked, poly, m);
            prop_assert_eq!(barrett, naive);
        }

        #[test]
        fn test_reduce_result_fits_in_field(m in 2u32..=16u32) {
            let poly = PrimitivePolynomialDatabase::standard(m as usize).unwrap() as u128;
            let reducer = BarrettReducer::new(poly, m);
            let max_product_bits = 2 * m - 1;
            // Test with max value
            let product = (1u128 << max_product_bits) - 1;
            let result = reducer.reduce(product);
            prop_assert!(result < (1u64 << m), "result {result} >= 2^{m}");
        }

        #[test]
        fn test_barrett_clmul_reduce_matches_naive_gf2_8(a in 0u64..256, b in 0u64..256) {
            // Generate random field elements, multiply, then verify Barrett == naive
            let poly: u128 = 0x11B; // x^8 + x^4 + x^3 + x^2 + 1
            let m: u32 = 8;
            let reducer = BarrettReducer::new(poly, m);
            let product = clmul(a, b);
            let barrett = reducer.reduce(product);
            let naive = naive_reduce(product, poly, m);
            prop_assert_eq!(barrett, naive);
        }

        #[test]
        fn test_barrett_clmul_reduce_matches_naive_gf2_16(a in 0u64..65536, b in 0u64..65536) {
            // Generate random field elements, multiply, then verify Barrett == naive
            let poly: u128 = 0b10000000000101101; // x^16 + x^5 + x^3 + x^2 + 1
            let m: u32 = 16;
            let reducer = BarrettReducer::new(poly, m);
            let product = clmul(a, b);
            let barrett = reducer.reduce(product);
            let naive = naive_reduce(product, poly, m);
            prop_assert_eq!(barrett, naive);
        }

        #[test]
        fn test_barrett_mul_associative_gf2_4(a in 1u64..16, b in 1u64..16, c in 1u64..16) {
            // (a*b)*c == a*(b*c) in GF(2^4)
            let poly: u128 = 0b10011;
            let m: u32 = 4;
            let reducer = BarrettReducer::new(poly, m);

            let ab = reducer.reduce(clmul(a, b));
            let ab_c = reducer.reduce(clmul(ab, c));

            let bc = reducer.reduce(clmul(b, c));
            let a_bc = reducer.reduce(clmul(a, bc));

            prop_assert_eq!(ab_c, a_bc);
        }
    }

    /// Verify BarrettReducer handles the maximum-width boundary (m=63) correctly.
    ///
    /// At m=63 the Barrett constant mu has degree 63 (fits in u128) and the
    /// dividend x^(2m) = x^126 is the largest we can store in a u128. Any
    /// arithmetic bug at this edge would produce a reducer that disagrees with
    /// the naive reference.
    #[test]
    fn test_reduce_at_m_equals_63_boundary() {
        // x^63 + x + 1 is a primitive trinomial for GF(2^63).
        let poly: u128 = (1u128 << 63) | 0b11;
        let m: u32 = 63;
        let reducer = BarrettReducer::new(poly, m);
        assert_eq!(reducer.degree(), 63);

        // Random-ish products with degree up to 2m-2 = 124.
        let max_deg_mask = (1u128 << 125) - 1;
        let samples: [u128; 8] = [
            0,
            1,
            1u128 << 62,
            1u128 << 124,
            (1u128 << 125) - 1,
            0xDEAD_BEEF_CAFE_BABE_0123_4567_89AB_CDEFu128 & max_deg_mask,
            0x5555_5555_5555_5555_5555_5555_5555_5555u128 & max_deg_mask,
            0xAAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAA_AAAAu128 & max_deg_mask,
        ];
        for &p in &samples {
            let barrett = reducer.reduce(p);
            let naive = naive_reduce(p, poly, m);
            assert_eq!(
                barrett, naive,
                "m=63 reducer mismatch for product {p:#x}: barrett={barrett:#x} naive={naive:#x}"
            );
        }
    }

    /// Pins the current `degree <= 63` boundary of [`BarrettReducer::new`].
    ///
    /// This test intentionally asserts the panic message, so any future
    /// widening (JIT issue `6fb4abad`, multi-word GF(2^m)) forces a
    /// deliberate update here — it is NOT a latent bug. See the module-
    /// level docs for the underlying 256-bit arithmetic requirement that
    /// extending Barrett to `m >= 64` would entail.
    ///
    /// Removing this test (rather than relaxing the bound after a proper
    /// 256-bit Barrett implementation lands) would silently change the
    /// dispatch contract and is explicitly discouraged.
    #[test]
    #[should_panic(expected = "degree must be in 1..=63")]
    fn test_new_rejects_degree_64_today() {
        // GF(2^64) standard polynomial — degree 64 not supported by Barrett yet.
        let poly: u128 = (1u128 << 64) | 0b11011;
        let _ = BarrettReducer::new(poly, 64);
    }

    #[test]
    fn test_reduce_with_clmul_matches_reduce_all_primitive_polys() {
        // Verify that reduce_with_clmul using the scalar clmul produces identical
        // results to reduce() for all primitive polynomials m=2..16.
        for m in 2u32..=16 {
            let poly = PrimitivePolynomialDatabase::standard(m as usize).unwrap() as u128;
            let reducer = BarrettReducer::new(poly, m);

            let max_product_deg = 2 * m - 2;
            let num_tests = if max_product_deg <= 12 {
                1u128 << (max_product_deg + 1) // exhaustive for small fields
            } else {
                4096 // sample for larger fields
            };

            for product in 0..num_tests {
                let p = if max_product_deg > 12 {
                    let p = product
                        .wrapping_mul(0x9E3779B97F4A7C15)
                        .wrapping_add(product ^ 0xDEAD);
                    p & ((1u128 << (max_product_deg + 1)) - 1)
                } else {
                    product
                };

                let via_reduce = reducer.reduce(p);
                let via_clmul = reducer.reduce_with_clmul(p, clmul);
                assert_eq!(
                    via_reduce, via_clmul,
                    "m={m}, product={p:#x}: reduce={via_reduce:#x}, reduce_with_clmul={via_clmul:#x}"
                );
            }
        }
    }
}
