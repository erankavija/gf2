//! Validation of the byte-field prototype against gf2's consumers
//! (jit:19513245).
//!
//! Correctness precedes timing: the launcher runs this binary before it
//! projects a plan, and stops at the first mismatch. Every check either
//! compares the prototype's arithmetic with an independent bit-by-bit oracle
//! written without gf2-core, or compares a prototype consumer route with the
//! gf2-core entry point it would replace. The two together cover REQ-05:
//! every byte coefficient over both affected polynomials, the zero and one
//! coefficients named separately, unaligned inputs, tail lengths, and the
//! overlap contract.
//!
//! The shared field-law suite runs in the sibling `field-laws` crate, which
//! needs gf2-core's `test-support` feature; keeping it out of this crate
//! keeps that feature out of the measured arm's build.

use bytefield_consumer::{
    axpy_field_vec, axpy_region, gemm_region, matvec_region, pairwise_region, CoefficientTable,
    ProductTable, RuntimeGf256Any, WideGf256x11b,
};
use byte_field_gf2_side::workload::{self, ByteField, RuntimeGf256, WideGf256};
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{batch, Gf2mField};

/// The two GF(2^8) reduction polynomials a byte-oriented design has to
/// serve: 0x11D, the field `Gf2mField::gf256()` builds and every pinned
/// comparison library implements, and 0x11B, the other standard byte field.
const POLYNOMIALS: [u16; 2] = [0x11D, 0x11B];

/// Bit-by-bit GF(2^8) multiplication, independent of gf2-core and of the
/// prototype's doubling recurrence.
fn reference_mul(left: u8, right: u8, polynomial: u16) -> u8 {
    let mut result = 0u16;
    let mut shifted = u16::from(left);
    for bit in 0..8 {
        if (right >> bit) & 1 == 1 {
            result ^= shifted;
        }
        shifted <<= 1;
        if shifted & 0x100 != 0 {
            shifted ^= polynomial;
        }
    }
    (result & 0xFF) as u8
}

/// Deterministic operand bytes: a fixed linear congruential stream, so the
/// binary needs no dependency to be reproducible.
struct Bytes(u64);

impl Bytes {
    fn new(seed: u64) -> Self {
        Bytes(seed | 1)
    }

    fn next(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 33) as u8
    }

    fn fill(&mut self, bytes: &mut [u8]) {
        for slot in bytes.iter_mut() {
            *slot = self.next();
        }
    }
}

struct Checks {
    passed: u64,
    failed: u64,
}

impl Checks {
    fn new() -> Self {
        Checks {
            passed: 0,
            failed: 0,
        }
    }

    fn expect(&mut self, condition: bool, what: impl FnOnce() -> String) {
        if condition {
            self.passed += 1;
        } else {
            self.failed += 1;
            println!("FAIL      {}", what());
        }
    }

    fn report(&mut self, line: &str) {
        println!("ok        {line} ({} checks)", self.passed);
        self.passed = 0;
    }
}

fn main() {
    let mut checks = Checks::new();
    tables(&mut checks);
    coefficients(&mut checks);
    boundaries(&mut checks);
    unaligned(&mut checks);
    overlap(&mut checks);
    representations(&mut checks);
    products(&mut checks);
    pairwise(&mut checks);
    backends();
    if checks.failed > 0 {
        println!("FAILED    {} checks did not hold", checks.failed);
        std::process::exit(1);
    }
    println!("PASSED    every check held");
}

/// Every entry of every coefficient table and of the full table, against the
/// independent oracle, for both polynomials.
fn tables(checks: &mut Checks) {
    for polynomial in POLYNOMIALS {
        for coefficient in 0..=255u8 {
            let table = CoefficientTable::new(coefficient, polynomial);
            for value in 0..=255u8 {
                let expected = reference_mul(coefficient, value, polynomial);
                checks.expect(table.get(value) == expected, || {
                    format!("table[{coefficient}][{value}] modulo {polynomial:#X}")
                });
            }
        }
        let full = ProductTable::new(polynomial);
        for left in 0..=255u8 {
            for right in 0..=255u8 {
                checks.expect(full.get(left, right) == reference_mul(left, right, polynomial), || {
                    format!("full[{left}][{right}] modulo {polynomial:#X}")
                });
            }
        }
        checks.expect(full.polynomial() == polynomial, || {
            "the full table reports the polynomial it was built over".to_owned()
        });
    }
    checks.report(
        "coefficient and full multiplication tables against the bit-by-bit oracle, \
         all 256 coefficients by all 256 values, polynomials 0x11D and 0x11B",
    );
}

/// The region route for every byte coefficient over both polynomials, with
/// the zero and one coefficients checked for their defining property rather
/// than only against the oracle.
fn coefficients(checks: &mut Checks) {
    const LENGTH: usize = 67;
    for polynomial in POLYNOMIALS {
        let mut stream = Bytes::new(u64::from(polynomial) * 7919);
        let mut source = vec![0u8; LENGTH];
        let mut target = vec![0u8; LENGTH];
        stream.fill(&mut source);
        stream.fill(&mut target);
        for coefficient in 0..=255u8 {
            let table = CoefficientTable::new(coefficient, polynomial);
            let expected: Vec<u8> = target
                .iter()
                .zip(&source)
                .map(|(y, x)| y ^ reference_mul(coefficient, *x, polynomial))
                .collect();
            let mut actual = target.clone();
            axpy_region(&mut actual, &table, &source);
            checks.expect(actual == expected, || {
                format!("axpy_region coefficient {coefficient} modulo {polynomial:#X}")
            });
        }
        let zero = CoefficientTable::new(0, polynomial);
        let mut unchanged = target.clone();
        axpy_region(&mut unchanged, &zero, &source);
        checks.expect(unchanged == target, || {
            format!("the zero coefficient leaves the destination unchanged modulo {polynomial:#X}")
        });
        let one = CoefficientTable::new(1, polynomial);
        let mut xored = target.clone();
        axpy_region(&mut xored, &one, &source);
        let expected: Vec<u8> = target.iter().zip(&source).map(|(y, x)| y ^ x).collect();
        checks.expect(xored == expected, || {
            format!("the one coefficient reduces to XOR modulo {polynomial:#X}")
        });
    }
    checks.report(
        "region multiply-accumulate for every byte coefficient over 0x11D and 0x11B, \
         with the zero and one coefficients checked for their defining property",
    );
}

/// Empty, single-element, word-boundary and odd lengths, on both routes.
fn boundaries(checks: &mut Checks) {
    const LENGTHS: [usize; 9] = [0, 1, 2, 15, 16, 17, 63, 64, 65];
    let field = RuntimeGf256::new();
    let polynomial = field.polynomial() as u16;
    let mut stream = Bytes::new(0x5EED);
    for length in LENGTHS {
        let mut source = vec![0u8; length];
        let mut target = vec![0u8; length];
        stream.fill(&mut source);
        stream.fill(&mut target);
        let coefficient = stream.next();
        let table = CoefficientTable::new(coefficient, polynomial);
        let expected: Vec<u8> = target
            .iter()
            .zip(&source)
            .map(|(y, x)| y ^ reference_mul(coefficient, *x, polynomial))
            .collect();
        let mut region = target.clone();
        axpy_region(&mut region, &table, &source);
        checks.expect(region == expected, || {
            format!("axpy_region at length {length}")
        });
        // The in-place vector route and the gf2-core entry point on the same
        // operands, so the prototype is compared with the consumer it would
        // replace rather than only with the oracle.
        let mut prototype = workload::pack_vec(&field, &target);
        let sources = workload::pack_vec(&field, &source);
        axpy_field_vec(&field, &mut prototype, &table, &sources);
        let mut current: FieldVec<_> = workload::pack_vec(&field, &target);
        current.axpy(&field.element(coefficient), &sources);
        let mut prototype_bytes = vec![0u8; length];
        let mut current_bytes = vec![0u8; length];
        workload::unpack_vec::<RuntimeGf256>(&prototype, &mut prototype_bytes);
        workload::unpack_vec::<RuntimeGf256>(&current, &mut current_bytes);
        checks.expect(prototype_bytes == expected, || {
            format!("axpy_field_vec at length {length}")
        });
        checks.expect(current_bytes == expected, || {
            format!("FieldVec::axpy at length {length}")
        });
    }
    checks.report(
        "empty, single-element, word-boundary and odd tail lengths on the region route, \
         the in-place vector route and FieldVec::axpy",
    );
}

/// Regions that start at every byte offset inside a larger allocation, with
/// source and destination offsets chosen independently.
fn unaligned(checks: &mut Checks) {
    const LENGTH: usize = 129;
    let polynomial = 0x11Du16;
    let mut stream = Bytes::new(0xA11A);
    let mut source_buffer = vec![0u8; LENGTH + 16];
    let mut target_buffer = vec![0u8; LENGTH + 16];
    stream.fill(&mut source_buffer);
    stream.fill(&mut target_buffer);
    let coefficient = stream.next();
    let table = CoefficientTable::new(coefficient, polynomial);
    for source_offset in 0..8usize {
        for target_offset in 0..8usize {
            let source = &source_buffer[source_offset..source_offset + LENGTH];
            let original = &target_buffer[target_offset..target_offset + LENGTH];
            let expected: Vec<u8> = original
                .iter()
                .zip(source)
                .map(|(y, x)| y ^ reference_mul(coefficient, *x, polynomial))
                .collect();
            let mut actual = original.to_vec();
            axpy_region(&mut actual, &table, source);
            checks.expect(actual == expected, || {
                format!("axpy_region at source offset {source_offset}, destination offset {target_offset}")
            });
        }
    }
    checks.report("regions at every source and destination byte offset from 0 to 7");
}

/// The overlap contract, stated and checked.
///
/// `FieldVec::axpy` takes `&mut self` and `&Self`, so the borrow checker
/// makes an aliasing call impossible; `axpy_region` and `axpy_field_vec`
/// carry the same signatures and inherit the same guarantee. What remains
/// checkable is that neither route writes through its source, which a caller
/// that passes two views of one buffer through a copy would rely on.
fn overlap(checks: &mut Checks) {
    const LENGTH: usize = 96;
    let polynomial = 0x11Du16;
    let mut stream = Bytes::new(0x0FF5);
    let mut source = vec![0u8; LENGTH];
    let mut target = vec![0u8; LENGTH];
    stream.fill(&mut source);
    stream.fill(&mut target);
    let coefficient = stream.next();
    let table = CoefficientTable::new(coefficient, polynomial);
    let source_before = source.clone();
    axpy_region(&mut target, &table, &source);
    checks.expect(source == source_before, || {
        "the region route leaves its source unmodified".to_owned()
    });
    let field = RuntimeGf256::new();
    let sources = workload::pack_vec(&field, &source);
    let mut targets = workload::pack_vec(&field, &target);
    let mut source_bytes = vec![0u8; LENGTH];
    axpy_field_vec(&field, &mut targets, &table, &sources);
    workload::unpack_vec::<RuntimeGf256>(&sources, &mut source_bytes);
    checks.expect(source_bytes == source_before, || {
        "the in-place vector route leaves its source unmodified".to_owned()
    });
    println!(
        "ok        the overlap contract: destination is &mut and source is &, so no route \
         admits an aliasing call; both leave the source unmodified ({} checks)",
        std::mem::replace(&mut checks.passed, 0)
    );
}

/// Both gf2 element representations and both polynomials through the same
/// generic in-place route, against `FieldVec::axpy` on the same operands.
fn representations(checks: &mut Checks) {
    const LENGTH: usize = 257;
    check_representation(checks, &RuntimeGf256::new(), LENGTH);
    check_representation(checks, &RuntimeGf256Any::new(0x11B), LENGTH);
    check_representation(checks, &WideGf256, LENGTH);
    check_representation(checks, &WideGf256x11b, LENGTH);
    checks.report(
        "the in-place vector route against FieldVec::axpy for Gf2mElement and \
         Gf2mWide<1,_> over 0x11D and 0x11B",
    );
}

fn check_representation<B: ByteField>(checks: &mut Checks, field: &B, length: usize) {
    let polynomial = field.polynomial() as u16;
    let mut stream = Bytes::new(u64::from(polynomial) * 104729 + length as u64);
    let mut source = vec![0u8; length];
    let mut target = vec![0u8; length];
    stream.fill(&mut source);
    stream.fill(&mut target);
    for coefficient in [0u8, 1, 2, 0x53, 0x80, 0xFF, stream.next()] {
        let table = CoefficientTable::new(coefficient, polynomial);
        let sources = workload::pack_vec(field, &source);
        let mut prototype = workload::pack_vec(field, &target);
        axpy_field_vec(field, &mut prototype, &table, &sources);
        let mut current = workload::pack_vec(field, &target);
        current.axpy(&field.element(coefficient), &sources);
        let mut prototype_bytes = vec![0u8; length];
        let mut current_bytes = vec![0u8; length];
        workload::unpack_vec::<B>(&prototype, &mut prototype_bytes);
        workload::unpack_vec::<B>(&current, &mut current_bytes);
        checks.expect(prototype_bytes == current_bytes, || {
            format!(
                "{} coefficient {coefficient} modulo {polynomial:#X}",
                B::NAME
            )
        });
    }
}

/// The dense product and the matrix-vector product against `gemm` and
/// `matvec`, at boundary and measured shapes.
fn products(checks: &mut Checks) {
    const SHAPES: [(usize, usize, usize); 6] = [
        (1, 1, 1),
        (1, 7, 1),
        (2, 3, 4),
        (7, 9, 5),
        (8, 8, 8),
        (64, 64, 64),
    ];
    let field = RuntimeGf256::new();
    let polynomial = field.polynomial() as u16;
    let table = ProductTable::new(polynomial);
    let mut stream = Bytes::new(0xC0FFEE);
    for (rows, inner, cols) in SHAPES {
        let mut left_bytes = vec![0u8; rows * inner];
        let mut right_bytes = vec![0u8; inner * cols];
        stream.fill(&mut left_bytes);
        stream.fill(&mut right_bytes);
        let left = workload::pack_matrix(&field, &left_bytes, rows, inner);
        let right = workload::pack_matrix(&field, &right_bytes, inner, cols);
        let current = gemm(&left, &right);
        let mut current_bytes = vec![0u8; rows * cols];
        workload::unpack_matrix::<RuntimeGf256>(&current, &mut current_bytes);
        let mut prototype_bytes = vec![0u8; rows * cols];
        gemm_region(
            &left_bytes,
            &right_bytes,
            &mut prototype_bytes,
            rows,
            inner,
            cols,
            &table,
        );
        checks.expect(prototype_bytes == current_bytes, || {
            format!("gemm_region at {rows}x{inner} by {inner}x{cols}")
        });
        // The same left operand as a matrix-vector product against the first
        // column of the right one.
        let vector_bytes: Vec<u8> = (0..inner).map(|row| right_bytes[row * cols]).collect();
        let vector = FieldVec::from(
            vector_bytes
                .iter()
                .map(|byte| field.element(*byte))
                .collect::<Vec<_>>(),
        );
        let current_matvec = left.matvec(&vector);
        let mut current_matvec_bytes = vec![0u8; rows];
        workload::unpack_vec::<RuntimeGf256>(&current_matvec, &mut current_matvec_bytes);
        let mut prototype_matvec = vec![0u8; rows];
        matvec_region(
            &left_bytes,
            &vector_bytes,
            &mut prototype_matvec,
            rows,
            inner,
            &table,
        );
        checks.expect(prototype_matvec == current_matvec_bytes, || {
            format!("matvec_region at {rows}x{inner}")
        });
    }
    let _ = FieldMatrix::<gf2_core::gf2m::Gf2mElement>::shape;
    checks.report("the dense product and the matrix-vector product against gemm and matvec");
}

/// The arbitrary pairwise route against `gf2m::batch::batch_mul`.
fn pairwise(checks: &mut Checks) {
    const LENGTHS: [usize; 6] = [0, 1, 63, 64, 65, 4096];
    let field = Gf2mField::gf256();
    let polynomial = field.primitive_polynomial() as u16;
    let table = ProductTable::new(polynomial);
    let mut stream = Bytes::new(0xBA7C4);
    for length in LENGTHS {
        let mut left = vec![0u8; length];
        let mut right = vec![0u8; length];
        stream.fill(&mut left);
        stream.fill(&mut right);
        let left_lanes = workload::widen(&left);
        let right_lanes = workload::widen(&right);
        let mut lanes = vec![0u64; length];
        batch::batch_mul(&field, &left_lanes, &right_lanes, &mut lanes);
        let mut current = vec![0u8; length];
        workload::narrow(&lanes, &mut current);
        let mut prototype = vec![0u8; length];
        pairwise_region(&left, &right, &mut prototype, &table);
        checks.expect(prototype == current, || {
            format!("pairwise_region at length {length}")
        });
    }
    checks.report("the arbitrary pairwise route against gf2m::batch::batch_mul");
}

/// The backend contract as this host reports it at run time: which
/// dispatched kernels the current consumer routes reach here, so the
/// validation record says which baseline the campaigns measure.
fn backends() {
    println!(
        "observed  gf2m batch kernel available: {}",
        workload::batch_kernel_available()
    );
    println!(
        "observed  gf2m whole-gemm kernel available: {}",
        workload::gemm_kernel_available()
    );
    println!(
        "observed  FieldMatrix<Gf2mElement> gemm route: {}",
        <RuntimeGf256 as ByteField>::gemm_route()
    );
    println!(
        "observed  FieldMatrix<Gf2mWide<1,Gf256x11d>> gemm route: {}",
        <WideGf256 as ByteField>::gemm_route()
    );
    println!(
        "observed  Gf2mField::gf256() log/antilog tables: {}",
        Gf2mField::gf256().has_tables()
    );
    println!(
        "observed  Gf2mField::new(8, 0x11B) log/antilog tables: {}",
        Gf2mField::new(8, 0x11B).has_tables()
    );
}
