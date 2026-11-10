//! Allocation witness for the GF(2^8) split-table dense product: pins
//! the allocation count and total bytes at one shape for both single-word
//! representations.

#![cfg(feature = "test-support")]

use gf2_core::field::FiniteField;
use gf2_core::gf2m::{
    last_gf256_table_lane, Gf2mElement, Gf2mField, Gf2mWide, Gf2mWideConfig, GF256_TABLE_LANE,
};
use thread_allocations::{count, CountingAllocator};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// GF(2^8) under `x^8 + x^4 + x^3 + x^2 + 1` (0x11D).
struct Gf256Poly11dCfg;
impl Gf2mWideConfig<1> for Gf256Poly11dCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1d];
}

/// The named shape: a 40 × 24 left operand by a 24 × 72 right operand.
const SHAPE: (usize, usize, usize) = (40, 24, 72);

/// Bytes of scratch the accepted path is allowed: the packed left operand, the
/// restored right operand and the byte accumulator.
const EXPECTED_BYTES: u64 = (SHAPE.0 * SHAPE.1 + SHAPE.1 * SHAPE.2 + SHAPE.0 * SHAPE.2) as u64;

/// Buffers of the named shape: left operand, transposed right operand and a
/// zeroed destination, as the whole-product hook receives them.
fn operands<F: FiniteField>(value: impl Fn(u64) -> F) -> (Vec<F>, Vec<F>, Vec<F>) {
    let (m, k, n) = SHAPE;
    let a: Vec<F> = (0..m * k)
        .map(|i| value((i as u64 * 7 + 1) & 0xff))
        .collect();
    let b_t: Vec<F> = (0..n * k)
        .map(|i| value((i as u64 * 11 + 3) & 0xff))
        .collect();
    let out: Vec<F> = (0..m * n).map(|_| value(0)).collect();
    (a, b_t, out)
}

#[test]
fn the_element_product_allocates_three_byte_buffers() {
    let field = Gf2mField::new(8, 0x11d);
    let (a, b_t, mut out) = operands(|v| field.element(v));
    let (m, k, n) = SHAPE;

    // The first touch builds the table and initialises the lane witness; both
    // are process-wide and paid once, so they run before the counters arm.
    assert!(<Gf2mElement as FiniteField>::try_simd_gemm_classical(
        &a, &b_t, m, k, n, &mut out
    ));

    let ((), counts) = count(|| {
        assert!(<Gf2mElement as FiniteField>::try_simd_gemm_classical(
            &a, &b_t, m, k, n, &mut out
        ));
    });

    assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);
    assert_eq!(counts.allocations + counts.reallocations, 3);
    assert_eq!(counts.bytes, EXPECTED_BYTES);
}

#[test]
fn the_wide_product_allocates_three_byte_buffers() {
    type Wide = Gf2mWide<1, Gf256Poly11dCfg>;
    let (a, b_t, mut out) = operands(Wide::from_u64);
    let (m, k, n) = SHAPE;

    assert!(<Wide as FiniteField>::try_simd_gemm_classical(
        &a, &b_t, m, k, n, &mut out
    ));

    let ((), counts) = count(|| {
        assert!(<Wide as FiniteField>::try_simd_gemm_classical(
            &a, &b_t, m, k, n, &mut out
        ));
    });

    assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);
    assert_eq!(counts.allocations + counts.reallocations, 3);
    assert_eq!(counts.bytes, EXPECTED_BYTES);
}
