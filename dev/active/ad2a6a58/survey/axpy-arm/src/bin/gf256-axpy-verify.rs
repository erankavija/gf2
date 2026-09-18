//! Lane equivalence check of the measured executable (jit:ad2a6a58).
//!
//! Both arms of a pair are this crate's two lanes, so a campaign is worth
//! queuing only once they agree: the check runs `FieldVec::axpy` on both lanes
//! over both element representations, at the word-boundary lengths and every
//! byte coefficient, and reads the shipped lane witness to establish that the
//! lane it asked for is the lane that ran.
//!
//! The lane switch is process-global and this binary is single-threaded, so
//! each toggle-execute-observe section runs alone.

use byte_field_arm_common::{OperandStream, SplitMix64};
use byte_field_gf2_side::workload::{self, ByteField, RuntimeGf256, WideGf256};
use gf2_core::gf2m::{GF256_SCALAR_LANE, GF256_TABLE_LANE};
use gf256_axpy_arm::Lane;

/// Lengths every representation is checked at: the empty and single-element
/// cases, the word boundary, and a length past any unrolling.
const LENGTHS: [usize; 8] = [0, 1, 63, 64, 65, 255, 1024, 4097];

fn main() {
    let mut checks = 0usize;
    checks += verify(&RuntimeGf256::new());
    checks += verify(&WideGf256);
    println!("# GF(2^8) axpy lane equivalence (jit:ad2a6a58)");
    println!("# every case runs both lanes of this executable on identical operands");
    println!("PASS lane equivalence: {checks} cases agree elementwise");
}

/// Runs both lanes over one representation and compares their results.
fn verify<B: ByteField>(field: &B) -> usize {
    let mut rng = SplitMix64::new(0xad2a_6a58);
    let mut cases = 0usize;
    for length in LENGTHS {
        let mut source = vec![0u8; length];
        let mut target = vec![0u8; length];
        rng.fill(&mut source);
        rng.fill(&mut target);
        for coefficient in 0u8..=255 {
            let scalar = run_lane(field, Lane::Scalar, coefficient, &source, &target);
            let table = run_lane(field, Lane::Table, coefficient, &source, &target);
            assert_eq!(
                scalar, table,
                "{} length {length} coefficient {coefficient}: the lanes disagree",
                B::NAME
            );
            cases += 1;
        }
    }
    cases
}

/// One `FieldVec::axpy` on the named lane, returning the destination bytes.
///
/// A zero-length call reaches no dispatch, so only a non-empty call carries a
/// lane assertion.
fn run_lane<B: ByteField>(
    field: &B,
    lane: Lane,
    coefficient: u8,
    source: &[u8],
    target: &[u8],
) -> Vec<u8> {
    lane.apply();
    let x = workload::pack_vec(field, source);
    let mut y = workload::pack_vec(field, target);
    workload::axpy(&mut y, &field.element(coefficient), &x);
    if !source.is_empty() {
        let expected = match lane {
            Lane::Scalar => GF256_SCALAR_LANE,
            Lane::Table => GF256_TABLE_LANE,
        };
        assert_eq!(
            gf2_core::gf2m::last_gf256_table_lane(),
            expected,
            "{} took a lane other than the one this arm selected",
            B::NAME
        );
    }
    let mut bytes = vec![0u8; target.len()];
    workload::unpack_vec::<B>(&y, &mut bytes);
    bytes
}
