//! Lane equivalence check of the measured executable (jit:4c1e441f).
//!
//! Both arms of a pair are this crate's two lanes, so a campaign is worth
//! queuing only once they agree: the check runs `field::matrix::gemm` on both
//! lanes over both element representations, at the square dimensions the family
//! declares and at the shapes around the region kernel's unrolled step, and
//! reads the shipped lane witness to establish that the lane it asked for is
//! the lane that ran.
//!
//! The lane switch is process-global and this binary is single-threaded, so
//! each toggle-execute-observe section runs alone.

use byte_field_arm_common::{OperandStream, SplitMix64};
use byte_field_gf2_side::workload::{ByteField, RuntimeGf256, WideGf256};
use gf2_core::gf2m::{last_gf256_table_lane, GF256_SCALAR_LANE, GF256_TABLE_LANE};
use gf256_gemm_arm::{product_on_lane, Lane};

/// Shapes every representation is checked at: the single-element product, the
/// shapes around the region kernel's eight-element unrolled step, shapes whose
/// three dimensions all differ, and the three square dimensions the family's
/// cells declare. A shape with a zero dimension returns before `gemm` offers the
/// product to the hook and is the shipped conformance suite's case, not a lane
/// comparison.
const SHAPES: [(usize, usize, usize); 10] = [
    (1, 1, 1),
    (1, 8, 1),
    (7, 9, 7),
    (8, 8, 8),
    (9, 7, 9),
    (63, 64, 65),
    (64, 64, 64),
    (65, 65, 63),
    (256, 256, 256),
    (512, 512, 512),
];

fn main() {
    let element = verify(&RuntimeGf256::new());
    let wide = verify(&WideGf256);
    println!("# GF(2^8) dense product lane equivalence (jit:4c1e441f)");
    println!("# every case runs both lanes of this executable on identical operands");
    println!("PASS lane equivalence: {element} shapes agree byte for byte, Gf2mElement");
    println!("PASS lane equivalence: {wide} shapes agree byte for byte, Gf2mWide<1,Gf256x11d>");
}

/// Runs both lanes over one representation and compares their product bytes.
fn verify<B: ByteField>(field: &B) -> usize {
    let mut rng = SplitMix64::new(0x4c1e_441f);
    let mut shapes = 0usize;
    for shape in SHAPES {
        let (m, k, n) = shape;
        let mut a = vec![0u8; m * k];
        let mut b = vec![0u8; k * n];
        rng.fill(&mut a);
        rng.fill(&mut b);
        let scalar = run_lane(field, Lane::Scalar, shape, &a, &b);
        let table = run_lane(field, Lane::Table, shape, &a, &b);
        assert_eq!(
            scalar,
            table,
            "{} shape {m}x{k}x{n}: the lanes disagree",
            B::NAME
        );
        shapes += 1;
    }
    shapes
}

/// One product on the named lane, returning the result bytes, having established
/// that the lane the shipped witness recorded is the lane this check selected.
fn run_lane<B: ByteField>(
    field: &B,
    lane: Lane,
    shape: (usize, usize, usize),
    a: &[u8],
    b: &[u8],
) -> Vec<u8> {
    let bytes = product_on_lane(field, lane, shape, a, b);
    let expected = match lane {
        Lane::Scalar => GF256_SCALAR_LANE,
        Lane::Table => GF256_TABLE_LANE,
    };
    assert_eq!(
        last_gf256_table_lane(),
        expected,
        "{} took a lane other than the one this check selected",
        B::NAME
    );
    bytes
}
