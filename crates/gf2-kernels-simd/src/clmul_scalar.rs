//! Scalar bit-parallel carry-less multiply: the single software `clmul`
//! definition for production callers and test oracles in the workspace.

/// Carry-less multiplication of two `u64` GF(2)-coefficient polynomials,
/// producing a 128-bit product in O(popcount(b)) XOR-shifts.
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
