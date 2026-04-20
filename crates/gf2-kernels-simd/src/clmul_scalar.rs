//! Scalar bit-parallel carry-less multiply — the SSOT for the
//! software-only `clmul` used across the workspace. Production
//! callers (e.g. `gf2_core::gf2m::barrett::clmul`) and test-only
//! reference oracles both delegate here so the algorithm has a single
//! definition.

/// Carry-less multiplication of two `u64` GF(2)-coefficient polynomials,
/// producing a 128-bit product. Bit-parallel iteration over the set bits
/// of `b` — O(popcount(b)) XOR-shifts. Safe, pure Rust, no CPU feature
/// requirements.
///
/// # Examples
///
/// ```
/// use gf2_kernels_simd::clmul_u64_scalar;
///
/// // (x + 1) * (x + 1) = x^2 + 1  (no carry: x + x = 0 in GF(2))
/// assert_eq!(clmul_u64_scalar(0b11, 0b11), 0b101);
///
/// // x * x = x^2
/// assert_eq!(clmul_u64_scalar(0b10, 0b10), 0b100);
/// ```
pub fn clmul_u64_scalar(a: u64, b: u64) -> u128 {
    let a = a as u128;
    let mut result: u128 = 0;
    let mut b_remaining = b;
    while b_remaining != 0 {
        let bit = b_remaining.trailing_zeros();
        result ^= a << bit;
        b_remaining &= b_remaining - 1;
    }
    result
}
