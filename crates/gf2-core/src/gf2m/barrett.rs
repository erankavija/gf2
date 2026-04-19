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
//! # Width limitation (`m <= 63`) — [`BarrettReducer`]
//!
//! **[`BarrettReducer`] deliberately caps the supported degree at `m <= 63`.**
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
//! This widening is provided by [`BarrettReducerWide`] (JIT issue `9dd11973`),
//! which landed as Task 3 of story `6fb4abad`. [`BarrettReducerWide`] handles
//! arbitrary `N`-word fields (e.g. `m = 127` with `N = 2`, `m = 256` with
//! `N = 4`) using multi-word carry-less multiplication via [`super::wide::clmul_wide`].
//!
//! # Multi-word Barrett reduction — [`BarrettReducerWide`]
//!
//! [`BarrettReducerWide`] is the N-word sibling. Key design decisions:
//!
//! ## `[u64; 2*N]` and the Rust 1.80 const-generics caveat
//!
//! On stable Rust 1.80 (our MSRV), `[u64; 2 * N]` is not permitted as an
//! array-length expression in a function signature. The same workaround as
//! [`super::wide::clmul_wide`] is used throughout this module:
//!
//! - **Pattern (a): two const parameters with a compile-time assertion.**
//!   [`BarrettReducerWide::reduce`] is declared as
//!   `fn reduce<const M: usize>(&self, product: &[u64; M]) -> [u64; N]` and
//!   asserts `M == 2 * N` at the call site via `const { assert!(M == 2 * N) }`.
//!   Callers supply the turbofish: `reducer.reduce::<{2 * N}>(&product)`.
//!
//! This matches the pattern established by `clmul_wide::<N, {2*N}>` and keeps
//! the API purely functional (no `&mut` out-parameters).
//!
//! ## `mu` storage
//!
//! The Barrett constant `mu = floor(x^(2m) / P(x))` has degree exactly `m`, so
//! it needs `m + 1` bits. Because `m` occupies the same `N` words as the field
//! elements (with `64*(N-1) < m <= 64*N`), the degree-`m` bit of `mu` may spill
//! into a hypothetical `N`-th word (0-indexed). To avoid the `N+1`-word
//! allocation problem, `mu` is stored as `[u64; N]` with its **implicit leading
//! bit at position m handled explicitly** during reduction — exactly the same
//! convention used by `Gf2mWideConfig::MODULUS`.
//!
//! ## Internal arithmetic
//!
//! All multi-word carry-less multiplications use `clmul_wide` with the
//! double-const-parameter pattern. Internal helpers operate on `&[u64]` slices
//! with `debug_assert!` bounds checks when the output size cannot be expressed
//! as a compile-time constant directly in that context.

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
/// Barrett is only wired in when the backing type is `u64`. For wider
/// fields (`m = 64..=127`, `m = 128..=255`, etc.) use
/// [`BarrettReducerWide`], which handles arbitrary `N`-word fields by
/// operating through [`super::wide::clmul_wide`] and explicit
/// multi-word shift helpers. For u128-backed fields at `m >= 64`,
/// `Gf2mField_<u128>` transparently falls back to the generic schoolbook
/// primitive, so correctness is preserved — only the PCLMULQDQ + Barrett
/// fast path is unavailable at those degrees via this reducer.
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

// ---------------------------------------------------------------------------
// Multi-word Barrett reduction helpers (used by BarrettReducerWide)
// ---------------------------------------------------------------------------

/// Wide right-shift of a 2N-word little-endian polynomial by `shift` bits,
/// returning only the low N words of the result.
///
/// The full result of `shr(a, shift)` occupies `2N - shift/64` words, but the
/// caller only needs the low N words (because subsequent operations discard the
/// rest). This helper avoids building the full `2N`-word intermediate.
fn wide_shr_2n_to_n<const N: usize, const M: usize>(a: &[u64; M], shift: u32) -> [u64; N] {
    const { assert!(M == 2 * N, "wide_shr_2n_to_n: M must equal 2 * N") }
    let mut out = [0u64; N];
    if shift == 0 {
        // Return the low N words directly.
        out[..N].copy_from_slice(&a[..N]);
        return out;
    }
    let word_shift = (shift / 64) as usize;
    let bit_shift = shift % 64;
    #[allow(clippy::needless_range_loop)]
    for i in 0..N {
        let src = i + word_shift;
        if src >= M {
            break;
        }
        out[i] = a[src] >> bit_shift;
        if bit_shift != 0 && src + 1 < M {
            out[i] |= a[src + 1] << (64 - bit_shift);
        }
    }
    out
}

/// Carry-less multiply two N-word polynomials producing a 2N-word result (via
/// `clmul_wide`), but add the extra contribution from the implicit leading bit
/// of `b` (at bit position `b_implicit_bit`) to the result.
///
/// This is used to multiply by `mu` or by `modulus` when those polynomials have
/// an implicit leading-one bit stored separately (same convention as
/// `Gf2mWideConfig::MODULUS`).
///
/// The result is `a * b_stored XOR (a << b_implicit_bit)`, returned in a 2N-word
/// array.
fn clmul_wide_with_implicit_high<const N: usize, const M: usize>(
    a: &[u64; N],
    b_stored: &[u64; N],
    b_implicit_bit: u32,
) -> [u64; M] {
    const {
        assert!(
            M == 2 * N,
            "clmul_wide_with_implicit_high: M must equal 2 * N"
        )
    }
    // Main product: a * b_stored (N words × N words → 2N words).
    let mut out = super::wide::clmul_wide::<N, M>(a, b_stored);
    // Add contribution from the implicit high bit: a * x^b_implicit_bit = a << b_implicit_bit.
    let word_shift = (b_implicit_bit / 64) as usize;
    let bit_shift = b_implicit_bit % 64;
    #[allow(clippy::needless_range_loop)]
    for i in 0..N {
        let dst = i + word_shift;
        if dst >= M {
            break;
        }
        out[dst] ^= a[i] << bit_shift;
        if bit_shift != 0 && dst + 1 < M {
            out[dst + 1] ^= a[i] >> (64 - bit_shift);
        }
    }
    out
}

/// Compute `mu = floor(x^(2m) / P(x))` over GF(2) via polynomial long division.
///
/// `P` is represented by its low `N*64` bits in `modulus_words` with an implicit
/// leading 1 at bit `m`. The quotient `mu` also has degree `m`; it is stored in
/// `[u64; N]` with its implicit leading 1 at bit `m` dropped (i.e. only the low
/// m bits of mu are returned, following the same `Gf2mWideConfig::MODULUS` convention).
///
/// # Panics
///
/// Panics if `m` is 0 or greater than `64 * N`.
fn compute_mu_wide<const N: usize>(modulus_words: &[u64; N], m: u32) -> [u64; N] {
    assert!(
        m > 0 && (m as usize) <= 64 * N,
        "compute_mu_wide: m must be in 1..={n64}",
        n64 = 64 * N
    );

    // We perform long division of x^(2m) by P(x) over GF(2).
    // The dividend starts as x^(2m) and we reduce it degree by degree.
    // The quotient collects the bits from degree 2m down to m (yielding m+1
    // bits total for the full mu, but we drop the leading 1).
    //
    // Implementation: maintain the remainder as a 2N+1-word array (2m+1 bits).
    // For MSRV we cannot write [u64; 2*N+1], so we use a Vec<u64> here; this
    // is called only at construction time (precomputation), so the allocation
    // cost is incurred once.
    let total_bits = 2 * m as usize + 1;
    let total_words = total_bits.div_ceil(64);

    let mut remainder = vec![0u64; total_words];
    // Set bit 2m.
    let top_word = (2 * m as usize) / 64;
    let top_bit = (2 * m as usize) % 64;
    remainder[top_word] |= 1u64 << top_bit;

    let mut quotient = [0u64; N];

    // Process quotient bits from degree m down to 0 (the i-th quotient bit
    // corresponds to position m+i in the dividend).
    for i in (0..=m).rev() {
        let bit_pos = (m + i) as usize;
        let w = bit_pos / 64;
        let b = bit_pos % 64;
        if w >= remainder.len() {
            continue;
        }
        if (remainder[w] >> b) & 1 == 1 {
            // Set quotient bit at position i (but i == m means the implicit
            // leading bit, which we drop from the stored representation).
            if i < m {
                let qw = (i as usize) / 64;
                let qb = (i as usize) % 64;
                quotient[qw] |= 1u64 << qb;
            }
            // XOR in P shifted to align its degree at bit_pos.
            // P(x) has its implicit high bit at position m; total degree m.
            // Shifting P so that its high bit lands at bit_pos = m + i means
            // shifting by i positions.
            let p_shift = i as usize;
            // XOR the stored low bits of P (shifted by p_shift).
            let p_word_shift = p_shift / 64;
            let p_bit_shift = (p_shift % 64) as u32;
            #[allow(clippy::needless_range_loop)]
            for j in 0..N {
                let dst = j + p_word_shift;
                if dst >= remainder.len() {
                    break;
                }
                remainder[dst] ^= modulus_words[j] << p_bit_shift;
                if p_bit_shift != 0 && dst + 1 < remainder.len() {
                    remainder[dst + 1] ^= modulus_words[j] >> (64 - p_bit_shift);
                }
            }
            // XOR the implicit high bit of P (bit m) shifted by p_shift = i
            // positions, which lands at bit m + i = bit_pos (already handled
            // by the `if` condition above — the bit was set, and XOR with 1
            // clears it).
            if p_bit_shift == 0 {
                remainder[w] ^= 1u64 << b;
            } else {
                // The implicit high bit contribution was: 1 << (m + i).
                // We need to XOR that in.
                let hi_word = bit_pos / 64;
                let hi_bit = bit_pos % 64;
                if hi_word < remainder.len() {
                    remainder[hi_word] ^= 1u64 << hi_bit;
                }
            }
        }
    }

    quotient
}

// ---------------------------------------------------------------------------
// BarrettReducerWide — multi-word Barrett reduction for GF(2^m)
// ---------------------------------------------------------------------------

/// Precomputed Barrett reduction constants for an N-word GF(2^m) field.
///
/// `BarrettReducerWide<N>` is the multi-word sibling of [`BarrettReducer`]:
/// whereas [`BarrettReducer`] is limited to `m ≤ 63` (fitting within `u128`
/// intermediates), `BarrettReducerWide<N>` handles extension degrees
/// `64*(N-1) < m ≤ 64*N`, enabling fields like GF(2^127) (`N = 2`) or
/// GF(2^256) (`N = 4`).
///
/// # Algorithm
///
/// Given an unreduced product `c(x)` with `deg(c) < 2m`, the reduction
/// `c(x) mod P(x)` proceeds in four steps, all over GF(2) (XOR arithmetic):
///
/// 1. `q1 = c >> (m − 1)` — extract high bits of the product.
/// 2. `q2 = q1 * mu` — multiply by the precomputed Barrett constant.
/// 3. `q3 = q2 >> (m + 1)` — extract the quotient estimate.
/// 4. `r = c XOR (q3 * P)` — subtract the estimated multiple of P.
///
/// The result `r` has `deg(r) < m` after at most two XOR corrections.
///
/// # MSRV caveat — `[u64; 2*N]` in function signatures
///
/// Stable Rust 1.80 (the project MSRV) does not permit const arithmetic in
/// array-length positions. [`BarrettReducerWide::reduce`] therefore takes a
/// second const parameter `M` with a compile-time assertion `M == 2 * N`:
///
/// ```text
/// reducer.reduce::<{2 * N}>(&product)
/// ```
///
/// This matches the pattern of [`super::wide::clmul_wide`] and avoids `&mut`
/// out-parameters.
///
/// # `mu` storage convention
///
/// The Barrett constant `mu = floor(x^(2m) / P(x))` has degree exactly `m`,
/// requiring `m + 1` bits. The **implicit leading bit at position m** is dropped
/// from the stored representation, exactly as `Gf2mWideConfig::MODULUS` drops
/// the implicit high bit of the irreducible polynomial. The stored `mu` field
/// holds only the low `m` bits of the Barrett constant.
///
/// # Examples
///
/// ```
/// use gf2_core::gf2m::barrett::BarrettReducerWide;
///
/// // GF(2^63) with P(x) = x^63 + x + 1 (N = 1).
/// // The modulus low-bits (excluding the implicit x^63 term) are 0b11 = 3.
/// let reducer = BarrettReducerWide::<1>::new([3u64], 63);
///
/// // A product with degree ≤ 2*63 - 2 = 124.
/// let product = [0xDEAD_BEEF_CAFE_BABEu64, 0x0000_0000_0000_1234u64];
/// let reduced = reducer.reduce::<2>(&product);
/// // Result fits in 63 bits (bit 63 and above are zero).
/// assert_eq!(reduced[0] >> 63, 0, "high bit must be zero after reduction");
/// ```
///
/// # Panics
///
/// [`BarrettReducerWide::new`] panics if `m` is 0, greater than `64 * N`, or if
/// the implicit leading bit of `modulus` at position `m` is inconsistent with `N`.
///
/// [`BarrettReducerWide::reduce`] panics (at compile time) if `M != 2 * N`.
#[derive(Debug, Clone)]
pub struct BarrettReducerWide<const N: usize> {
    /// The low m bits of the irreducible polynomial P(x), in `N` little-endian
    /// u64 words. The leading bit at position m is implicit (not stored).
    modulus: [u64; N],
    /// The low m bits of `mu = floor(x^(2m) / P(x))`, in `N` little-endian
    /// u64 words. The leading bit at position m is implicit (not stored).
    mu: [u64; N],
    /// The extension degree m.
    m: u32,
}

impl<const N: usize> BarrettReducerWide<N> {
    /// Precompute Barrett constants for the given irreducible polynomial.
    ///
    /// The modulus is given as its **low m bits** in `N` little-endian `u64`
    /// words; the implicit leading bit at position `m` must not be included
    /// (same convention as `Gf2mWideConfig::MODULUS`).
    ///
    /// Internally, this function performs polynomial long division of `x^(2m)`
    /// by `P(x)` over GF(2) to obtain `mu = floor(x^(2m) / P(x))`.
    ///
    /// # Arguments
    ///
    /// * `modulus` - The low `m` bits of the irreducible polynomial in `N`
    ///   little-endian `u64` words. The implicit leading bit at bit `m` must
    ///   not be set.
    /// * `m` - The extension degree. Must satisfy `1 <= m <= 64 * N`.
    ///   Typical callers have `64*(N-1) < m <= 64*N` (the natural per-word
    ///   range), but any value in `1..=64*N` is accepted.
    ///
    /// # Panics
    ///
    /// Panics if `m` is 0, greater than `64 * N`, or if bit `m` (the
    /// implicit leading bit of the polynomial) is set in `modulus`
    /// (the leading bit must be implicit, not stored).
    ///
    /// # Complexity
    ///
    /// O(m²) for the polynomial long division used to compute mu.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducerWide;
    ///
    /// // GF(2^127) with P(x) = x^127 + x + 1.
    /// // Low bits are stored in 2 words; leading bit at position 127 is implicit.
    /// let reducer = BarrettReducerWide::<2>::new([3u64, 0u64], 127);
    /// assert_eq!(reducer.degree(), 127);
    /// ```
    pub fn new(modulus: [u64; N], m: u32) -> Self {
        assert!(
            m > 0 && (m as usize) <= 64 * N,
            "BarrettReducerWide: m must be in 1..={n64}",
            n64 = 64 * N
        );
        // The implicit leading bit at position m must not be set in the stored
        // low bits. Position m is at word m/64, bit (m%64). For m == 64*N the
        // bit would be in word N (out of range), so we only check when in range.
        if (m as usize) < 64 * N {
            let w = (m as usize) / 64;
            let b = (m as usize) % 64;
            assert_eq!(
                (modulus[w] >> b) & 1,
                0,
                "BarrettReducerWide: modulus must not have its leading bit set at position {m}; \
                 store only the low m bits (implicit-leading-one convention)"
            );
        }
        let mu = compute_mu_wide(&modulus, m);
        BarrettReducerWide { modulus, mu, m }
    }

    /// Reduce an unreduced product of degree `< 2m` back to an `m`-bit element.
    ///
    /// Applies multi-word Barrett reduction: given `c(x)` with `deg(c) < 2m`,
    /// returns `c(x) mod P(x)` as `N` little-endian `u64` words.
    ///
    /// # MSRV caveat: second const parameter `M`
    ///
    /// Because `[u64; 2 * N]` is not legal as an array-length expression on
    /// stable Rust 1.80, the product is passed as `&[u64; M]` where `M` is a
    /// separate const parameter. A compile-time assertion `M == 2 * N` is
    /// enforced. Callers must supply the turbofish:
    /// `reducer.reduce::<{2 * N}>(&product)`.
    ///
    /// # Arguments
    ///
    /// * `product` - The polynomial to reduce, as `M = 2 * N` little-endian
    ///   `u64` words. Must have `deg(product) < 2m`; higher bits are ignored.
    ///
    /// # Returns
    ///
    /// The remainder `c(x) mod P(x)` as `N` little-endian `u64` words, with
    /// all bits at positions `>= m` equal to zero.
    ///
    /// # Panics
    ///
    /// Panics at compile time if `M != 2 * N`.
    ///
    /// # Complexity
    ///
    /// O(N²) carry-less word multiplications; two `clmul_wide` calls of cost
    /// O(N²) each, plus O(N) shift and XOR operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducerWide;
    ///
    /// // GF(2^63) with P(x) = x^63 + x + 1.
    /// let reducer = BarrettReducerWide::<1>::new([3u64], 63);
    ///
    /// // The zero product reduces to zero.
    /// let zero = reducer.reduce::<2>(&[0u64, 0u64]);
    /// assert_eq!(zero, [0u64]);
    ///
    /// // An already-reduced element is unchanged.
    /// let already = reducer.reduce::<2>(&[42u64, 0u64]);
    /// assert_eq!(already, [42u64]);
    /// ```
    pub fn reduce<const M: usize>(&self, product: &[u64; M]) -> [u64; N] {
        const { assert!(M == 2 * N, "BarrettReducerWide::reduce: M must equal 2 * N") }
        let m = self.m;

        // Fast path: if product has no bits at or above position m, it is
        // already reduced — return the low N words directly.
        if wide_is_already_reduced::<N, M>(product, m) {
            let mut out = [0u64; N];
            out[..N].copy_from_slice(&product[..N]);
            return out;
        }

        // Step 1: q1 = product >> (m - 1).
        // q1 is the high part of the product; degree <= m + 1.
        // We only need the low N words of q1 (the rest are discarded by step 3).
        let q1: [u64; N] = wide_shr_2n_to_n::<N, M>(product, m - 1);

        // Step 2: q2 = q1 * mu (with implicit leading bit of mu at position m).
        // q2 fits in 2N words; we only need the high part for step 3.
        let q2: [u64; M] = clmul_wide_with_implicit_high::<N, M>(&q1, &self.mu, m);

        // Step 3: q3 = q2 >> (m + 1).
        // q3 has at most m - 1 bits; we need the low N words.
        let q3: [u64; N] = wide_shr_2n_to_n::<N, M>(&q2, m + 1);

        // Step 4: r = product XOR (q3 * modulus).
        // q3 * modulus (with implicit leading bit at m) fits in 2N words.
        let q3p: [u64; M] = clmul_wide_with_implicit_high::<N, M>(&q3, &self.modulus, m);

        // XOR product with q3 * modulus; only the low N words matter (the high
        // words of a correct result must be zero after reduction).
        let mut r = [0u64; N];
        for i in 0..N {
            r[i] = product[i] ^ q3p[i];
        }

        // Step 5: at most two corrections — if deg(r) >= m, XOR with P once.
        let mask = field_mask::<N>(m);
        if is_high_bit_set(&r, m) {
            xor_modulus_into(&mut r, &self.modulus, m);
        }
        if is_high_bit_set(&r, m) {
            xor_modulus_into(&mut r, &self.modulus, m);
        }

        // Mask off any residual high bits (defensive; should not be needed after
        // at most two corrections, but ensures the invariant holds regardless).
        r[N - 1] &= mask;
        r
    }

    /// Slice-taking variant of [`BarrettReducerWide::reduce`] for callers that
    /// cannot construct a `[u64; 2 * N]` array at the call site.
    ///
    /// Under MSRV 1.80 stable, generic const expressions of the form `{2 * N}`
    /// are not accepted in array-length position from a generic context, so
    /// callers that parameterise over `N` alone (notably
    /// [`crate::gf2m::Gf2mWide::mul_ref`]) cannot directly invoke the
    /// array-typed `reduce`. This method accepts a `&[u64]` slice of length
    /// exactly `2 * N` and performs the same Barrett reduction; the
    /// array-typed [`BarrettReducerWide::reduce`] is a thin wrapper around
    /// this method, so both share a single implementation.
    ///
    /// # Arguments
    ///
    /// * `product` — 2N-word carry-less product to reduce. Panics if
    ///   `product.len() != 2 * N`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducerWide;
    ///
    /// let reducer = BarrettReducerWide::<1>::new([3u64], 63);
    /// let reduced = reducer.reduce_slice(&[0u64, 0u64]);
    /// assert_eq!(reduced, [0u64]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `product.len() != 2 * N`.
    ///
    /// # Complexity
    ///
    /// O(N²) word-level operations, matching [`BarrettReducerWide::reduce`].
    pub fn reduce_slice(&self, product: &[u64]) -> [u64; N] {
        assert_eq!(
            product.len(),
            2 * N,
            "BarrettReducerWide::reduce_slice: product.len() must equal 2 * N"
        );
        let m = self.m;

        if slice_wide_is_already_reduced::<N>(product, m) {
            let mut out = [0u64; N];
            out[..N].copy_from_slice(&product[..N]);
            return out;
        }

        // Step 1: q1 = product >> (m - 1). Only the low N words are needed.
        let q1: [u64; N] = slice_wide_shr_2n_to_n::<N>(product, m - 1);

        // Step 2: q2 = q1 * mu (with implicit leading bit of mu at position m).
        // Accumulate into a 2N-length buffer on the stack-like Vec.
        let mut q2 = vec![0u64; 2 * N];
        slice_clmul_wide_with_implicit_high::<N>(&q1, &self.mu, m, &mut q2);

        // Step 3: q3 = q2 >> (m + 1).
        let q3: [u64; N] = slice_wide_shr_2n_to_n::<N>(&q2, m + 1);

        // Step 4: r = product XOR (q3 * modulus).
        let mut q3p = vec![0u64; 2 * N];
        slice_clmul_wide_with_implicit_high::<N>(&q3, &self.modulus, m, &mut q3p);

        let mut r = [0u64; N];
        for i in 0..N {
            r[i] = product[i] ^ q3p[i];
        }

        // Step 5: at most two corrections.
        let mask = field_mask::<N>(m);
        if is_high_bit_set(&r, m) {
            xor_modulus_into(&mut r, &self.modulus, m);
        }
        if is_high_bit_set(&r, m) {
            xor_modulus_into(&mut r, &self.modulus, m);
        }

        r[N - 1] &= mask;
        r
    }

    /// Returns the precomputed Barrett constant `mu = floor(x^(2m) / P(x))`,
    /// stored as `N` little-endian `u64` words (implicit leading bit at
    /// position `m` is dropped).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducerWide;
    ///
    /// let reducer = BarrettReducerWide::<1>::new([3u64], 63);
    /// let mu = reducer.mu();
    /// assert_eq!(mu.len(), 1);
    /// assert!(mu[0] > 0); // mu is non-trivial for a non-degenerate polynomial
    /// ```
    pub fn mu(&self) -> &[u64; N] {
        &self.mu
    }

    /// Returns the extension degree `m`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducerWide;
    ///
    /// let reducer = BarrettReducerWide::<2>::new([3u64, 0u64], 127);
    /// assert_eq!(reducer.degree(), 127);
    /// ```
    pub fn degree(&self) -> u32 {
        self.m
    }

    /// Returns the stored low bits of the irreducible polynomial `P(x)`, as
    /// `N` little-endian `u64` words (implicit leading bit at position `m` is
    /// dropped).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::gf2m::barrett::BarrettReducerWide;
    ///
    /// let reducer = BarrettReducerWide::<1>::new([3u64], 63);
    /// assert_eq!(reducer.modulus(), &[3u64]);
    /// ```
    pub fn modulus(&self) -> &[u64; N] {
        &self.modulus
    }
}

/// Returns the top-word mask for an `N`-word field of degree `m`.
///
/// Bits at positions `>= m` in the top word must be zero in a reduced element.
/// For `m == 64 * N` (top word fully used) this returns `u64::MAX`.
#[inline]
fn field_mask<const N: usize>(m: u32) -> u64 {
    let bits_in_top = (m as usize) - 64 * (N - 1);
    if bits_in_top >= 64 {
        u64::MAX
    } else {
        (1u64 << bits_in_top) - 1
    }
}

/// Returns `true` if the N-word value has any bit at position `>= m` set.
#[inline]
fn is_high_bit_set<const N: usize>(a: &[u64; N], m: u32) -> bool {
    if N == 0 {
        return false;
    }
    let top_mask = field_mask::<N>(m);
    // Any word above the top-field word must be zero in a reduced value.
    // The top-field word must have no bits above the mask.
    a[N - 1] & !top_mask != 0
}

/// Returns `true` if the 2N-word product has no bits at positions `>= m`.
///
/// Used for the fast-path check in `reduce`.
#[inline]
fn wide_is_already_reduced<const N: usize, const M: usize>(product: &[u64; M], m: u32) -> bool {
    const { assert!(M == 2 * N, "wide_is_already_reduced: M must equal 2 * N") }
    if M == 0 {
        return true;
    }
    // Words above index N-1 must all be zero.
    if product[N..M].iter().any(|&w| w != 0) {
        return false;
    }
    // In the top kept word (index N-1), bits at positions >= (m mod 64 within that word)
    // must be zero.
    let top_mask = field_mask::<N>(m);
    product[N - 1] & !top_mask == 0
}

/// Slice-based equivalent of [`wide_is_already_reduced`] for callers that
/// cannot produce a `[u64; 2 * N]` under MSRV 1.80 stable generics. Asserts
/// `product.len() == 2 * N` as a debug check.
#[inline]
fn slice_wide_is_already_reduced<const N: usize>(product: &[u64], m: u32) -> bool {
    debug_assert_eq!(product.len(), 2 * N);
    if product.len() < N {
        return true;
    }
    if product[N..].iter().any(|&w| w != 0) {
        return false;
    }
    let top_mask = field_mask::<N>(m);
    product[N - 1] & !top_mask == 0
}

/// Slice-based equivalent of [`wide_shr_2n_to_n`].
#[inline]
fn slice_wide_shr_2n_to_n<const N: usize>(a: &[u64], shift: u32) -> [u64; N] {
    debug_assert_eq!(a.len(), 2 * N);
    let m = 2 * N;
    let mut out = [0u64; N];
    if shift == 0 {
        out[..N].copy_from_slice(&a[..N]);
        return out;
    }
    let word_shift = (shift / 64) as usize;
    let bit_shift = shift % 64;
    #[allow(clippy::needless_range_loop)]
    for i in 0..N {
        let src = i + word_shift;
        if src >= m {
            break;
        }
        out[i] = a[src] >> bit_shift;
        if bit_shift != 0 && src + 1 < m {
            out[i] |= a[src + 1] << (64 - bit_shift);
        }
    }
    out
}

/// Slice-based equivalent of [`clmul_wide_with_implicit_high`] — writes the
/// product into `out: &mut [u64]` (length `2 * N`) via XOR accumulation.
#[inline]
fn slice_clmul_wide_with_implicit_high<const N: usize>(
    a: &[u64; N],
    b_stored: &[u64; N],
    b_implicit_bit: u32,
    out: &mut [u64],
) {
    debug_assert_eq!(out.len(), 2 * N);
    // Main product a * b_stored via schoolbook carry-less multiply.
    for i in 0..N {
        for j in 0..N {
            let p: u128 = clmul(a[i], b_stored[j]);
            out[i + j] ^= p as u64;
            out[i + j + 1] ^= (p >> 64) as u64;
        }
    }
    // Add contribution from implicit leading bit: a * x^b_implicit_bit.
    let word_shift = (b_implicit_bit / 64) as usize;
    let bit_shift = b_implicit_bit % 64;
    #[allow(clippy::needless_range_loop)]
    for i in 0..N {
        let dst = i + word_shift;
        if dst >= 2 * N {
            break;
        }
        out[dst] ^= a[i] << bit_shift;
        if bit_shift != 0 && dst + 1 < 2 * N {
            out[dst + 1] ^= a[i] >> (64 - bit_shift);
        }
    }
}

/// XOR the irreducible polynomial (stored low bits + implicit high bit at `m`)
/// into `r` in place.
#[inline]
fn xor_modulus_into<const N: usize>(r: &mut [u64; N], modulus: &[u64; N], m: u32) {
    // XOR in the low bits of P.
    for i in 0..N {
        r[i] ^= modulus[i];
    }
    // XOR in the implicit high bit at position m.
    let mw = (m as usize) / 64;
    let mb = (m as usize) % 64;
    if mw < N {
        r[mw] ^= 1u64 << mb;
    }
    // If mw >= N, the high bit is above all stored words — it lives in the
    // 2N space but r only has N words. Since deg(r) was < 2m and we XOR P
    // of degree m, the leading bit of P at position m cancels the leading bit
    // of r (which must equal the leading bit of P for the correction to make
    // sense). When m == 64*N, position m is word N (out of range), but the
    // is_high_bit_set check would not have triggered (top_mask == u64::MAX),
    // so this branch is unreachable in practice.
}

/// Naive O(m) shift-and-XOR polynomial reduction over GF(2) for multi-word
/// operands. Used only in tests as a reference oracle against which
/// [`BarrettReducerWide`] is checked.
///
/// This function is only compiled in test builds (`#[cfg(test)]`). It is
/// kept outside the `tests` module so that hypothetical integration tests in
/// the same crate can reach it, but callers outside of test code should use
/// [`BarrettReducerWide::reduce`] instead.
///
/// # Arguments
///
/// * `product` - The unreduced polynomial as `M = 2 * N` little-endian u64 words.
///   Must have `deg(product) < 2m`.
/// * `modulus` - The low m bits of the irreducible polynomial as `N` words.
///   The implicit leading bit at position `m` is not stored.
/// * `m` - The extension degree. Must satisfy `1 <= m <= 64 * N`.
///
/// # Panics
///
/// Panics at compile time if `M != 2 * N`.
///
/// # Complexity
///
/// O(m · N) — up to m shift-and-XOR passes, each O(N) words.
#[cfg(test)]
pub(crate) fn reference_reduce_wide<const N: usize, const M: usize>(
    product: &[u64; M],
    modulus: &[u64; N],
    m: u32,
) -> [u64; N] {
    const { assert!(M == 2 * N, "reference_reduce_wide: M must equal 2 * N") }
    // Work in a 2N-word mutable scratch buffer.
    let mut r = *product;

    // Total bits to scan: 2m down to m (exclusive).
    // For each bit position `bit` from 2m-2 down to m, if that bit is set in r,
    // XOR in P shifted so its degree-m term aligns with bit.
    let max_bit = 2 * m as usize;
    for bit in (m as usize..max_bit).rev() {
        let w = bit / 64;
        let b = bit % 64;
        if w >= M {
            continue;
        }
        if (r[w] >> b) & 1 == 1 {
            // XOR in P << (bit - m): low bits of P are in modulus[], implicit
            // high bit at position m, so P << (bit-m) has high bit at position bit.
            let shift = bit - m as usize;
            let wshift = shift / 64;
            let bshift = (shift % 64) as u32;
            // XOR in stored low bits.
            #[allow(clippy::needless_range_loop)]
            for j in 0..N {
                let dst = j + wshift;
                if dst >= M {
                    break;
                }
                r[dst] ^= modulus[j] << bshift;
                if bshift != 0 && dst + 1 < M {
                    r[dst + 1] ^= modulus[j] >> (64 - bshift);
                }
            }
            // XOR in implicit high bit at position (m + shift) = bit.
            r[w] ^= 1u64 << b;
        }
    }

    // Extract the low N words.
    let mut out = [0u64; N];
    out[..N].copy_from_slice(&r[..N]);
    out
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

    // =========================================================================
    // BarrettReducerWide tests
    // =========================================================================

    /// Helper: convert a GF(2^m) product stored in a `u128` into the 2-word
    /// format used by BarrettReducerWide<1>.
    fn u128_to_2words(v: u128) -> [u64; 2] {
        [v as u64, (v >> 64) as u64]
    }

    /// Helper: convert a 1-word result from BarrettReducerWide<1> to u64.
    fn word1_to_u64(w: [u64; 1]) -> u64 {
        w[0]
    }

    // -------------------------------------------------------------------------
    // N = 1, m = 63: cross-check against BarrettReducer (oracle)
    // -------------------------------------------------------------------------

    /// Cross-check BarrettReducerWide<1> at m=63 against BarrettReducer
    /// (the existing single-word oracle).
    ///
    /// The test uses the exact polynomial and sample products from the existing
    /// [`test_reduce_at_m_equals_63_boundary`] test, treating BarrettReducer
    /// as ground truth. Any divergence between the two implementations
    /// indicates a bug in BarrettReducerWide.
    #[test]
    fn test_wide_n1_m63_cross_check_against_barrett_reducer() {
        // Same polynomial as the existing m=63 oracle test.
        // P(x) = x^63 + x + 1; low bits = 0b11 = 3.
        let poly_u128: u128 = (1u128 << 63) | 0b11;
        let poly_u64: u64 = 0b11; // low 63 bits (implicit leading bit dropped)
        let m: u32 = 63;

        // Oracle: BarrettReducer (single-word path).
        let oracle = BarrettReducer::new(poly_u128, m);
        // Subject: BarrettReducerWide<1>.
        let wide = BarrettReducerWide::<1>::new([poly_u64], m);

        assert_eq!(wide.degree(), 63);
        assert_eq!(wide.modulus(), &[poly_u64]);

        // Same products used by test_reduce_at_m_equals_63_boundary.
        let max_deg_mask: u128 = (1u128 << 125) - 1;
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
            let expected: u64 = oracle.reduce(p);
            let product_words: [u64; 2] = u128_to_2words(p);
            let got: u64 = word1_to_u64(wide.reduce::<2>(&product_words));
            assert_eq!(
                got, expected,
                "m=63 cross-check failed for product {p:#x}: \
                 BarrettReducerWide got {got:#x}, oracle got {expected:#x}"
            );
        }
    }

    /// Additional known-value cross-check for N=1, m=63: verify several products
    /// also agree with naive_reduce.
    #[test]
    fn test_wide_n1_m63_matches_naive_reduce() {
        let poly_u128: u128 = (1u128 << 63) | 0b11;
        let poly_u64: u64 = 0b11;
        let m: u32 = 63;
        let wide = BarrettReducerWide::<1>::new([poly_u64], m);

        let samples: [u128; 6] = [
            0,
            1,
            (1u128 << 63) - 1,       // largest reduced element
            (1u128 << 63) | 7,       // just above m
            (1u128 << 124) | 0xDEAD, // near-max degree product
            (1u128 << 125) - 1,
        ];
        for &p in &samples {
            let expected = naive_reduce(p, poly_u128, m);
            let words = u128_to_2words(p);
            let got = word1_to_u64(wide.reduce::<2>(&words));
            assert_eq!(
                got, expected,
                "m=63 naive mismatch for product {p:#x}: wide={got:#x}, naive={expected:#x}"
            );
        }
    }

    // -------------------------------------------------------------------------
    // N = 2, m = 127: proptest vs reference_reduce_wide
    // -------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Proptest: BarrettReducerWide<2> at m=127 agrees with reference_reduce_wide.
        ///
        /// Polynomial: P(x) = x^127 + x + 1 (primitive trinomial for GF(2^127)).
        /// This is a well-known primitive polynomial and has been verified independently.
        ///
        /// Products are 256-bit random values (4 u64 words) with degree masked to
        /// ≤ 253 bits (2m-1 = 253; the algorithm tolerates degree 2m-1 for m=127
        /// since bit 127 fits within 2 u64 words).
        #[test]
        #[allow(renamed_and_removed_lints, clippy::arithmetic_side_effects)]
        fn prop_wide_n2_m127_matches_reference(
            p0 in proptest::num::u64::ANY,
            p1 in proptest::num::u64::ANY,
            p2 in 0u64..=(1u64 << 62) - 1,  // keep degree < 2*127 = 254 bits (max bit 253)
            p3 in 0u64..=(1u64 << 62) - 1,
        ) {
            // GF(2^127) with P(x) = x^127 + x + 1.
            // Low 127 bits = 0b11 (bits 1 and 0 set), split across 2 words:
            // word 0 = 3, word 1 = 0 (bit 127 is implicit, not stored).
            let modulus = [3u64, 0u64];
            let m: u32 = 127;
            let wide = BarrettReducerWide::<2>::new(modulus, m);
            let product = [p0, p1, p2, p3];
            let barrett = wide.reduce::<4>(&product);
            let reference = reference_reduce_wide::<2, 4>(&product, &modulus, m);
            prop_assert_eq!(barrett, reference,
                "m=127 mismatch for product [{:#x}, {:#x}, {:#x}, {:#x}]: \
                 barrett=[{:#x}, {:#x}] reference=[{:#x}, {:#x}]",
                p0, p1, p2, p3,
                barrett[0], barrett[1],
                reference[0], reference[1]);
        }
    }

    // -------------------------------------------------------------------------
    // N = 4, m = 256: proptest vs reference_reduce_wide
    // -------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Proptest: BarrettReducerWide<4> at m=256 agrees with reference_reduce_wide.
        ///
        /// Polynomial: P(x) = x^256 + x^10 + x^5 + x^2 + 1 (Seroussi HPL-98-135,
        /// Table 1 row m=256). Low bits = x^10 + x^5 + x^2 + 1 = 0x425.
        ///
        /// Valid products from field multiplication have degree ≤ 2*256 - 2 = 510.
        /// Bit 510 is word 7 bit 62, so word 7 must have its high bit (bit 63)
        /// clear. The constraint `p7 in 0..=i64::MAX as u64` enforces this.
        ///
        /// Note: the Barrett algorithm works for degree ≤ 2m-2 (the actual output
        /// of `clmul_wide` on two m-bit inputs). Inputs with degree 2m-1 would
        /// require one extra bit of precision in the q1 computation for the
        /// m = 64*N case (e.g. m=256), but such inputs cannot arise from valid
        /// field multiplication and are therefore not required to reduce correctly.
        #[test]
        #[allow(renamed_and_removed_lints, clippy::arithmetic_side_effects)]
        fn prop_wide_n4_m256_matches_reference(
            p0 in proptest::num::u64::ANY,
            p1 in proptest::num::u64::ANY,
            p2 in proptest::num::u64::ANY,
            p3 in proptest::num::u64::ANY,
            p4 in proptest::num::u64::ANY,
            p5 in proptest::num::u64::ANY,
            p6 in proptest::num::u64::ANY,
            // bit 510 = word-7 bit 62; word 7 bit 63 = bit 511 = 2m-1 is outside
            // the valid product range (products have degree ≤ 2m-2 = 510).
            p7 in 0u64..=(i64::MAX as u64),
        ) {
            // GF(2^256) with P(x) = x^256 + x^10 + x^5 + x^2 + 1.
            // Low bits: 0x425 = 2^10 + 2^5 + 2^2 + 1 = 1024 + 32 + 4 + 1.
            // Implicit high bit at position 256 (word 4) is NOT stored.
            let modulus = [0x425u64, 0u64, 0u64, 0u64];
            let m: u32 = 256;
            let wide = BarrettReducerWide::<4>::new(modulus, m);
            let product = [p0, p1, p2, p3, p4, p5, p6, p7];
            let barrett = wide.reduce::<8>(&product);
            let reference = reference_reduce_wide::<4, 8>(&product, &modulus, m);
            prop_assert_eq!(barrett, reference,
                "m=256 mismatch for product (first word {:#x}, last word {:#x}): \
                 barrett[0]={:#x} reference[0]={:#x}",
                p0, p7, barrett[0], reference[0]);
        }
    }

    // -------------------------------------------------------------------------
    // Additional known-value sanity checks for BarrettReducerWide
    // -------------------------------------------------------------------------

    #[test]
    fn test_wide_reduce_zero_n2_m127() {
        let wide = BarrettReducerWide::<2>::new([3u64, 0u64], 127);
        assert_eq!(wide.reduce::<4>(&[0u64; 4]), [0u64; 2]);
    }

    #[test]
    fn test_wide_reduce_zero_n4_m256() {
        let wide = BarrettReducerWide::<4>::new([0x425u64, 0u64, 0u64, 0u64], 256);
        assert_eq!(wide.reduce::<8>(&[0u64; 8]), [0u64; 4]);
    }

    #[test]
    fn test_wide_reduce_one_is_unchanged_n2_m127() {
        // 1 is already reduced (degree 0 < 127).
        let wide = BarrettReducerWide::<2>::new([3u64, 0u64], 127);
        let product = [1u64, 0u64, 0u64, 0u64];
        assert_eq!(wide.reduce::<4>(&product), [1u64, 0u64]);
    }

    #[test]
    fn test_wide_reduce_one_is_unchanged_n4_m256() {
        let wide = BarrettReducerWide::<4>::new([0x425u64, 0u64, 0u64, 0u64], 256);
        let product = [1u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64];
        assert_eq!(wide.reduce::<8>(&product), [1u64, 0u64, 0u64, 0u64]);
    }

    #[test]
    fn test_wide_reduce_result_fits_in_field_n2_m127() {
        // Any reduced result must have bits >= m cleared.
        let wide = BarrettReducerWide::<2>::new([3u64, 0u64], 127);
        let product = [u64::MAX, u64::MAX, u64::MAX, u64::MAX];
        let r = wide.reduce::<4>(&product);
        // m = 127; bit 127 is the high bit of word 1 (127 & 63 = 63).
        assert_eq!(r[1] >> 63, 0, "bit 127 must be zero after reduction");
    }

    #[test]
    fn test_wide_reduce_result_fits_in_field_n4_m256() {
        // m = 256 means all 256 bits in 4 words are valid; any result must be < 2^256.
        // Valid products have degree ≤ 2*256-2 = 510, so the top bit of word 7
        // (bit 511 = 2m-1) must be zero.  Use a product with all bits set except
        // the top bit of word 7 to exercise the near-maximum case.
        let wide = BarrettReducerWide::<4>::new([0x425u64, 0u64, 0u64, 0u64], 256);
        // [u64::MAX; 7] for words 0..6, then u64::MAX >> 1 for word 7 (clears bit 63 = bit 511).
        let product = [
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX >> 1,
        ];
        let r = wide.reduce::<8>(&product);
        // Compare against the naive reference — both must agree.
        let reference = reference_reduce_wide::<4, 8>(&product, &[0x425u64, 0u64, 0u64, 0u64], 256);
        assert_eq!(
            r, reference,
            "near-max-degree product reduction must match reference"
        );
    }

    #[test]
    fn test_wide_n1_m63_proptest_configurations() {
        // Proptest config: ≤100 cases.
        use proptest::test_runner::{Config, TestRunner};
        let mut runner = TestRunner::new(Config {
            cases: 100,
            ..Config::default()
        });
        let poly_u64: u64 = 0b11;
        let poly_u128: u128 = (1u128 << 63) | 0b11;
        let m: u32 = 63;
        let wide = BarrettReducerWide::<1>::new([poly_u64], m);

        let strat = (proptest::num::u64::ANY, 0u64..(1u64 << 61));
        runner
            .run(&strat, |(lo, hi)| {
                let p_u128 = (lo as u128) | ((hi as u128) << 64);
                let expected = naive_reduce(p_u128, poly_u128, m);
                let words = u128_to_2words(p_u128);
                let got = word1_to_u64(wide.reduce::<2>(&words));
                proptest::prop_assert_eq!(
                    got,
                    expected,
                    "N=1 m=63 proptest mismatch for product {:#x}",
                    p_u128
                );
                Ok(())
            })
            .unwrap();
    }
}
