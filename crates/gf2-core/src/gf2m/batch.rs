//! Batch element-wise GF(2^m) multiply and square over canonical inputs
//! (each element `< 2^m`), through a SIMD kernel for `m ∈ {8, 16, 32}` where
//! the CPU supports it and a scalar path otherwise.

#[cfg(feature = "simd")]
use crate::gf2m::barrett::BarrettReducer;
use crate::gf2m::Gf2mField;

/// Batch element-wise multiply: `out[i] = a[i] * b[i] mod P(x)`.
///
/// Uses the SIMD batch kernel when it is available and `field` has
/// `m ∈ {8, 16, 32}`, and a raw schoolbook loop otherwise. Elements of `a`
/// and `b` are canonical (`< 2^m`).
///
/// # Panics
///
/// Panics if `a.len() != b.len()` or `a.len() != out.len()`.
///
/// # Complexity
///
/// `a.len()` field multiplications; each is `O(m)` shift-XOR steps on the
/// scalar path and a Barrett-reduced carry-less product in the SIMD kernel.
pub fn batch_mul(field: &Gf2mField, a: &[u64], b: &[u64], out: &mut [u64]) {
    batch_mul_raw(field.degree(), field.primitive_polynomial(), a, b, out);
}

/// Crate-internal raw counterpart to [`batch_mul`].
///
/// Takes the field parameters directly so matrix kernels can hoist element
/// context handling out of their dot-product hot path. `primitive_poly` must
/// include the degree-`m` leading bit, matching [`Gf2mField::new`].
pub(crate) fn batch_mul_raw(m: usize, primitive_poly: u64, a: &[u64], b: &[u64], out: &mut [u64]) {
    assert_eq!(a.len(), b.len(), "input slices must have equal length");
    assert_eq!(
        a.len(),
        out.len(),
        "output slice must match input slice length"
    );

    #[cfg(feature = "simd")]
    {
        let m_u32 = m as u32;
        if matches!(m_u32, 8 | 16 | 32) {
            if let Some(fns) = crate::simd::maybe_gf2m_batch() {
                let reducer = BarrettReducer::new(primitive_poly as u128, m_u32);
                (fns.mul_fn)(
                    a,
                    b,
                    out,
                    reducer.mu() as u64,
                    reducer.modulus() as u64,
                    reducer.degree(),
                );
                return;
            }
        }
    }

    // Scalar fallback: allocation-free raw schoolbook multiplication.
    for i in 0..a.len() {
        out[i] = crate::gf2m::mul_raw::gf2m_mul_raw(a[i], b[i], m, primitive_poly);
    }
}

/// Batch element-wise square: `out[i] = a[i] * a[i] mod P(x)`.
///
/// Uses the SIMD batch kernel under the same condition as [`batch_mul`], and
/// per-element [`Gf2mField`] multiplication otherwise.
///
/// # Panics
///
/// Panics if `a.len() != out.len()`.
///
/// # Complexity
///
/// `a.len()` field squarings; each is one [`Gf2mField`] multiplication on the
/// scalar path and a Barrett-reduced carry-less square in the SIMD kernel.
pub fn batch_square(field: &Gf2mField, a: &[u64], out: &mut [u64]) {
    assert_eq!(
        a.len(),
        out.len(),
        "output slice must match input slice length"
    );

    #[cfg(feature = "simd")]
    {
        let m = field.degree() as u32;
        if matches!(m, 8 | 16 | 32) {
            if let Some(fns) = crate::simd::maybe_gf2m_batch() {
                let reducer = BarrettReducer::new(field.primitive_polynomial() as u128, m);
                (fns.square_fn)(
                    a,
                    out,
                    reducer.mu() as u64,
                    reducer.modulus() as u64,
                    reducer.degree(),
                );
                return;
            }
        }
    }

    for i in 0..a.len() {
        let ea = field.element(a[i]);
        out[i] = (&ea * &ea).value();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::Gf2mField;

    fn primitive_polys() -> &'static [(u32, u64)] {
        &[
            (8, 0b100011101),
            (16, 0b1_0001_0000_0000_1011),
            (32, 0b1_0000_0000_0100_0000_0000_0000_0000_0111),
        ]
    }

    fn scalar_mul_one(a: u64, b: u64, m: u32, poly: u64) -> u64 {
        // Bit-by-bit GF(2^m) multiplication — independent of every other
        // path so it serves as an oracle.
        if a == 0 || b == 0 {
            return 0;
        }
        let mut result = 0u64;
        let mut temp = a;
        for i in 0..m {
            if (b >> i) & 1 == 1 {
                result ^= temp;
            }
            let will_overflow = (temp >> (m - 1)) & 1 == 1;
            temp <<= 1;
            if will_overflow {
                temp ^= poly;
            }
        }
        result & if m == 64 { u64::MAX } else { (1u64 << m) - 1 }
    }

    #[test]
    fn batch_mul_matches_scalar_for_each_supported_m() {
        for &(m, poly) in primitive_polys() {
            let field = Gf2mField::new(m as usize, poly);
            let mask = if m == 64 { u64::MAX } else { (1u64 << m) - 1 };
            let n = 100;
            let a: Vec<u64> = (0..n)
                .map(|i| (i as u64).wrapping_mul(0x9E37_79B9) & mask)
                .collect();
            let b: Vec<u64> = (0..n)
                .map(|i| (i as u64).wrapping_mul(0x6C62_272E + 7) & mask)
                .collect();
            let mut out = vec![0u64; n];
            batch_mul(&field, &a, &b, &mut out);

            for i in 0..n {
                let expected = scalar_mul_one(a[i], b[i], m, poly);
                assert_eq!(out[i], expected, "m={m}, i={i}");
            }
        }
    }

    #[test]
    fn batch_square_matches_scalar_for_each_supported_m() {
        for &(m, poly) in primitive_polys() {
            let field = Gf2mField::new(m as usize, poly);
            let mask = if m == 64 { u64::MAX } else { (1u64 << m) - 1 };
            let n = 50;
            let a: Vec<u64> = (0..n)
                .map(|i| (i as u64).wrapping_mul(0xDEAD_BEEF) & mask)
                .collect();
            let mut out = vec![0u64; n];
            batch_square(&field, &a, &mut out);

            for i in 0..n {
                let expected = scalar_mul_one(a[i], a[i], m, poly);
                assert_eq!(out[i], expected, "m={m}, i={i}");
            }
        }
    }

    #[test]
    fn batch_mul_falls_back_for_unsupported_m() {
        // m = 12 is not in {8, 16, 32}; must fall back to per-element dispatch
        let m = 12u32;
        let poly = 0b1000001010011u64;
        let field = Gf2mField::new(m as usize, poly);
        let mask = (1u64 << m) - 1;
        let a: Vec<u64> = vec![0xABC, 0x123, 0xFFF, 1, 0];
        let b: Vec<u64> = vec![0xDEF, 0x456, 0x001, 0xFFF, 0xAAA];
        let mut out = vec![0u64; 5];
        batch_mul(&field, &a, &b, &mut out);
        for i in 0..5 {
            let expected = scalar_mul_one(a[i] & mask, b[i] & mask, m, poly);
            assert_eq!(out[i], expected, "fallback i={i}");
        }
    }

    #[test]
    fn batch_mul_word_boundary_lengths() {
        let field = Gf2mField::gf256();
        let m = 8u32;
        let poly = 0b100011101u64;
        let mask = (1u64 << m) - 1;

        for &len in &[0usize, 1, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
            let a: Vec<u64> = (0..len)
                .map(|i| (i as u64).wrapping_mul(0x9E37_79B9) & mask)
                .collect();
            let b: Vec<u64> = (0..len)
                .map(|i| (i as u64).wrapping_mul(0x6C62_272E) & mask)
                .collect();
            let mut out = vec![0u64; len];
            batch_mul(&field, &a, &b, &mut out);
            for i in 0..len {
                let expected = scalar_mul_one(a[i], b[i], m, poly);
                assert_eq!(out[i], expected, "len={len}, i={i}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "input slices must have equal length")]
    fn batch_mul_panics_on_length_mismatch() {
        let field = Gf2mField::gf256();
        let a = vec![1u64, 2, 3];
        let b = vec![1u64, 2];
        let mut out = vec![0u64; 3];
        batch_mul(&field, &a, &b, &mut out);
    }
}
