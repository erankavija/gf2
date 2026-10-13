//! Allocation witness: `generator_matrix_into` and `parity_check_matrix_into`
//! reach no allocator given a correctly sized buffer.

#[path = "support/thread_allocations.rs"]
mod thread_allocations;

use gf2_coding::bch::spec::{BchSpec, BinaryBchCode, DenseBchCode, DesignedDistance};
use gf2_coding::traits::block::{
    BlockCode, GeneratorMatrixAccess, ParityCheckMatrixAccess, SymbolMatrix,
};
use gf2_core::field::extension::BinaryPrimeExt;
use gf2_core::field::matrix::FieldMatrix;
use gf2_core::field::{ConstField, FieldPoly};
use gf2_core::gf2m::Gf2mField;
use gf2_core::gfp::Fp;
use gf2_core::gfpn::QuotientField;
use gf2_core::BitMatrix;
use thread_allocations::count;

/// The workload contract's B3 row, $255 \times 223$ over `GF(2^8)`.
fn packed_code() -> BinaryBchCode {
    let extension = BinaryPrimeExt::new(Gf2mField::new(8, 0b1_0001_1101))
        .expect("the contract's prim column is a primitive polynomial");
    BinaryBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(9).expect("a positive designed distance"),
    })
    .expect("the B3 row constructs")
}

/// A field-generic code over `GF(5)`, whose symbols carry no heap storage of
/// their own, so the materialization is the only possible allocator.
fn field_generic_code() -> DenseBchCode<QuotientField<Fp<5>>> {
    let modulus = FieldPoly::new(vec![Fp::<5>::new(1), Fp::new(1), Fp::new(1)]);
    let extension = QuotientField::new(Fp::<5>::zero(), modulus).expect("a valid GF(25)");
    DenseBchCode::construct(BchSpec::PrimitiveNarrowSense {
        extension,
        designed_distance: DesignedDistance::try_from(5).expect("a positive designed distance"),
    })
    .expect("a valid GF(5) BCH code")
}

#[test]
fn caller_buffer_materialization_reaches_no_allocator() {
    let packed = packed_code();
    let expected_generator = packed.generator_matrix().expect("allocating generator");
    let expected_parity = packed
        .parity_check_matrix()
        .expect("allocating parity check");
    let mut generator = BitMatrix::zeros(packed.k(), packed.n());
    let mut parity = BitMatrix::zeros(packed.redundancy(), packed.n());

    let ((), generator_requests) = count(|| {
        packed
            .generator_matrix_into(&mut generator)
            .expect("caller generator buffer");
    });
    let ((), parity_requests) = count(|| {
        packed
            .parity_check_matrix_into(&mut parity)
            .expect("caller parity buffer");
    });
    assert!(
        generator_requests.is_zero(),
        "packed generator materialization requested {generator_requests:?}"
    );
    assert!(
        parity_requests.is_zero(),
        "packed parity materialization requested {parity_requests:?}"
    );
    assert_eq!(generator, expected_generator);
    assert_eq!(parity, expected_parity);

    let dense = field_generic_code();
    let expected_generator = dense.generator_matrix().expect("allocating generator");
    let expected_parity = dense
        .parity_check_matrix()
        .expect("allocating parity check");
    let zero = dense.symbol_zero();
    let mut generator = FieldMatrix::zeroed(dense.k(), dense.n(), &zero);
    let mut parity = FieldMatrix::zeroed(dense.redundancy(), dense.n(), &zero);

    let ((), generator_requests) = count(|| {
        dense
            .generator_matrix_into(&mut generator)
            .expect("caller generator buffer");
    });
    let ((), parity_requests) = count(|| {
        dense
            .parity_check_matrix_into(&mut parity)
            .expect("caller parity buffer");
    });
    assert!(
        generator_requests.is_zero(),
        "field-generic generator materialization requested {generator_requests:?}"
    );
    assert!(
        parity_requests.is_zero(),
        "field-generic parity materialization requested {parity_requests:?}"
    );
    assert_eq!(generator, expected_generator);
    assert_eq!(parity, expected_parity);
}
