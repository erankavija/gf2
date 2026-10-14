//! Allocation witness for the GF(2^8) cached-table axpy lane: a call allocates
//! nothing.

#![cfg(feature = "test-support")]

use gf2_core::field::FieldVec;
use gf2_core::gf2m::{
    last_gf256_table_lane, Gf2mField, Gf2mWide, Gf2mWideConfig, GF256_TABLE_LANE,
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

#[test]
fn neither_representation_allocates_on_the_table_lane() {
    let field = Gf2mField::new(8, 0x11d);
    let coefficient = field.element(0x53);
    let mut y = FieldVec::from(
        (0..137u64)
            .map(|v| field.element(v & 0xff))
            .collect::<Vec<_>>(),
    );
    let x = FieldVec::from(
        (0..137u64)
            .map(|v| field.element((v * 7 + 3) & 0xff))
            .collect::<Vec<_>>(),
    );

    let wide_coefficient = Gf2mWide::<1, Gf256Poly11dCfg>::from_u64(0x53);
    let mut wide_y = FieldVec::from(
        (0..137u64)
            .map(|v| Gf2mWide::<1, Gf256Poly11dCfg>::from_u64(v & 0xff))
            .collect::<Vec<_>>(),
    );
    let wide_x = FieldVec::from(
        (0..137u64)
            .map(|v| Gf2mWide::<1, Gf256Poly11dCfg>::from_u64((v * 7 + 3) & 0xff))
            .collect::<Vec<_>>(),
    );

    // First touch builds the table and first use initialises the lane witness;
    // both are process-wide and paid once, so they run before the counter arms.
    y.axpy(&coefficient, &x);
    wide_y.axpy(&wide_coefficient, &wide_x);
    assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);

    let ((), element) = count(|| y.axpy(&coefficient, &x));
    assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);
    let ((), wide) = count(|| wide_y.axpy(&wide_coefficient, &wide_x));
    assert_eq!(last_gf256_table_lane(), GF256_TABLE_LANE);

    assert_eq!(element.allocations + element.reallocations, 0);
    assert_eq!(wide.allocations + wide.reallocations, 0);
}
