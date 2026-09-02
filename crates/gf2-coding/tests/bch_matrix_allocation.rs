//! Allocation witness for the caller-buffer matrix materialization.
//!
//! `generator_matrix_into` and `parity_check_matrix_into` carry the parity
//! recurrence in the caller's own buffer, so a correctly sized buffer is the
//! only storage a call needs. Witnessing that from outside the crate takes a
//! counting global allocator, a global allocator is a process-wide choice,
//! and the counter is process-wide too, so this witness owns its own test
//! binary and runs a single test in it.
//!
//! `unsafe` appears here for the [`GlobalAlloc`] implementation alone. This
//! is a test binary; the production crates outside `gf2-kernels-simd` and
//! `gf2-kernels-hip` keep `#![deny(unsafe_code)]`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

/// Forwards to the system allocator and counts every allocating call made
/// while [`ARMED`] is set.
struct CountingAllocator;

/// Whether allocations are currently being counted.
static ARMED: AtomicBool = AtomicBool::new(false);

/// Allocating calls observed since the counter was last reset.
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

fn record() {
    if ARMED.load(Ordering::Relaxed) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

// SAFETY: every method forwards its arguments unchanged to the system
// allocator, which satisfies the `GlobalAlloc` contract, and the counter
// touches no allocator of its own.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        System.realloc(ptr, layout, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Runs `body` with the counter armed and returns how many allocating calls
/// it made.
fn allocations_during(body: impl FnOnce()) -> usize {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ARMED.store(true, Ordering::Relaxed);
    body();
    ARMED.store(false, Ordering::Relaxed);
    ALLOCATIONS.load(Ordering::Relaxed)
}

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

    let generator_allocations = allocations_during(|| {
        packed
            .generator_matrix_into(&mut generator)
            .expect("caller generator buffer");
    });
    let parity_allocations = allocations_during(|| {
        packed
            .parity_check_matrix_into(&mut parity)
            .expect("caller parity buffer");
    });
    assert_eq!(generator_allocations, 0, "packed generator materialization");
    assert_eq!(parity_allocations, 0, "packed parity materialization");
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

    let generator_allocations = allocations_during(|| {
        dense
            .generator_matrix_into(&mut generator)
            .expect("caller generator buffer");
    });
    let parity_allocations = allocations_during(|| {
        dense
            .parity_check_matrix_into(&mut parity)
            .expect("caller parity buffer");
    });
    assert_eq!(
        generator_allocations, 0,
        "field-generic generator materialization"
    );
    assert_eq!(
        parity_allocations, 0,
        "field-generic parity materialization"
    );
    assert_eq!(generator, expected_generator);
    assert_eq!(parity, expected_parity);
}
