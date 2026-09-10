//! gf2-side adapter validation for the byte-field survey (jit:6c6b09b1).
//!
//! The shared field-law suite already covers `Gf2mField::gf256()` itself:
//! `crates/gf2-core/src/field/axiom_tests.rs` runs `test_field_axioms` over
//! that exact field, and the survey run script executes that test before any
//! timing. This binary covers the part the field-law suite cannot: the
//! adapter this survey wraps around gf2, namely the byte-to-element
//! conversion and the four consumer entry points the cells measure.
//!
//! The oracle is a shift-and-reduce scalar multiply written here, sharing no
//! code with gf2-core, so an entry point and its reference are independent.
//!
//! Exit status is 0 when every check passes and 1 on the first mismatch.

mod workload;

use byte_field_arm_common::SplitMix64;
use gf2_core::field::FieldVec;
use gf2_core::gf2m::Gf2mField;

const POLY_11D: u32 = 0x11D;
const POLY_11B: u32 = 0x11B;

/// Independent GF(2^8) multiply: carry-less product, then reduction by the
/// full polynomial, written without reference to gf2-core.
fn reference_mul(a: u8, b: u8, poly: u32) -> u8 {
    let mut product: u32 = 0;
    for bit in 0..8 {
        if (b >> bit) & 1 == 1 {
            product ^= u32::from(a) << bit;
        }
    }
    for degree in (8..=15).rev() {
        if (product >> degree) & 1 == 1 {
            product ^= poly << (degree - 8);
        }
    }
    (product & 0xFF) as u8
}

struct Report {
    checks: usize,
    failures: usize,
}

impl Report {
    fn pass(&mut self, what: &str) {
        self.checks += 1;
        println!("ok        {what}");
    }

    fn fail(&mut self, what: &str, detail: &str) {
        self.failures += 1;
        eprintln!("FAIL {what}: {detail}");
    }
}

/// Every ordered pair of field elements agrees with the independent oracle.
fn check_table(report: &mut Report, poly: u32) {
    let field = Gf2mField::new(8, u64::from(poly));
    for a in 0u16..256 {
        for b in 0u16..256 {
            let got = (field.element(u64::from(a)) * field.element(u64::from(b))).value() as u8;
            let want = reference_mul(a as u8, b as u8, poly);
            if got != want {
                report.fail(
                    "gf2 multiplication table",
                    &format!("{a} * {b} = {got}, reference {want}"),
                );
                return;
            }
        }
    }
    report.pass(&format!("gf2 multiplication table under 0x{poly:X}"));
}

/// Byte to `FieldVec` and back is lossless, so a conversion cost buys a
/// representation change and nothing else.
fn check_round_trip(report: &mut Report) {
    let field = Gf2mField::gf256();
    let mut rng = SplitMix64::new(0x6C6B_09B1);
    for length in [0usize, 1, 63, 64, 65, 127, 128, 4096] {
        let mut bytes = vec![0u8; length];
        rng.fill(&mut bytes);
        let vector = workload::pack_vec(&field, &bytes);
        let mut back = vec![0u8; length];
        workload::unpack_vec(&vector, &mut back);
        if back != bytes {
            report.fail("FieldVec round trip", &format!("length {length} changed"));
            return;
        }
    }
    report.pass("FieldVec byte round trip over the boundary lengths");
}

/// `FieldVec::axpy` accumulates `y[i] += a * x[i]` at every boundary length.
fn check_axpy(report: &mut Report) {
    let field = Gf2mField::gf256();
    let mut rng = SplitMix64::new(0x0A_7B_9C_2D);
    for length in [0usize, 1, 63, 64, 65, 127, 128, 4096] {
        for coefficient in [0u8, 1, 2, 3, 0x53, 0xFF] {
            let mut source = vec![0u8; length];
            let mut target = vec![0u8; length];
            rng.fill(&mut source);
            rng.fill(&mut target);
            let want: Vec<u8> = target
                .iter()
                .zip(source.iter())
                .map(|(y, x)| y ^ reference_mul(coefficient, *x, POLY_11D))
                .collect();
            let x = workload::pack_vec(&field, &source);
            let mut y = workload::pack_vec(&field, &target);
            workload::axpy(&mut y, &field.element(u64::from(coefficient)), &x);
            let mut got = vec![0u8; length];
            workload::unpack_vec(&y, &mut got);
            if got != want {
                report.fail(
                    "FieldVec::axpy",
                    &format!("coefficient {coefficient} length {length} disagrees"),
                );
                return;
            }
        }
    }
    report.pass("FieldVec::axpy against the independent scalar reference");
}

/// `gf2m::batch::batch_mul` multiplies distinct pairs with no coefficient
/// reuse, on the `u64` lanes a byte consumer must widen into.
fn check_pairwise(report: &mut Report) {
    let field = Gf2mField::gf256();
    let mut rng = SplitMix64::new(0x9A11);
    for length in [1usize, 63, 64, 65, 4096] {
        let mut left = vec![0u8; length];
        let mut right = vec![0u8; length];
        rng.fill(&mut left);
        rng.fill(&mut right);
        let want: Vec<u8> = left
            .iter()
            .zip(right.iter())
            .map(|(x, y)| reference_mul(*x, *y, POLY_11D))
            .collect();
        let lanes_left = workload::widen(&left);
        let lanes_right = workload::widen(&right);
        let mut lanes_out = vec![0u64; length];
        workload::pairwise(&field, &lanes_left, &lanes_right, &mut lanes_out);
        let mut got = vec![0u8; length];
        workload::narrow(&lanes_out, &mut got);
        if got != want {
            report.fail("batch_mul", &format!("length {length} disagrees"));
            return;
        }
    }
    report.pass("gf2m::batch::batch_mul against the independent scalar reference");
}

/// `field::matrix::gemm` at square and rectangular shapes, the second being
/// the generator-encode shape.
fn check_matmul(report: &mut Report, rows: usize, inner: usize, cols: usize, label: &str) {
    let field = Gf2mField::gf256();
    let mut rng = SplitMix64::new(0x6E31 + rows as u64);
    let mut left = vec![0u8; rows * inner];
    let mut right = vec![0u8; inner * cols];
    rng.fill(&mut left);
    rng.fill(&mut right);
    let mut want = vec![0u8; rows * cols];
    for row in 0..rows {
        for col in 0..cols {
            let mut accumulator = 0u8;
            for k in 0..inner {
                accumulator ^= reference_mul(left[row * inner + k], right[k * cols + col], POLY_11D);
            }
            want[row * cols + col] = accumulator;
        }
    }
    let a = workload::pack_matrix(&field, &left, rows, inner);
    let b = workload::pack_matrix(&field, &right, inner, cols);
    let c = workload::matmul(&a, &b);
    let mut got = vec![0u8; rows * cols];
    workload::unpack_matrix(&c, &mut got);
    if got != want {
        report.fail(label, "product disagrees with the independent reference");
        return;
    }
    report.pass(label);
}

/// The two degree-8 polynomials this survey meets define different fields,
/// so an adapter fixed at one is never compared against one fixed at the
/// other.
fn check_field_distinctness(report: &mut Report) {
    let at_11d = Gf2mField::new(8, u64::from(POLY_11D));
    let at_11b = Gf2mField::new(8, u64::from(POLY_11B));
    for a in 2u64..256 {
        for b in 2u64..256 {
            let d = (at_11d.element(a) * at_11d.element(b)).value();
            let s = (at_11b.element(a) * at_11b.element(b)).value();
            if d != s {
                report.checks += 1;
                println!(
                    "ok        0x11B and 0x11D are distinct fields in gf2 too: {a} * {b} is {d} \
                     under 0x11D and {s} under 0x11B"
                );
                return;
            }
        }
    }
    report.fail("field distinctness", "the two polynomials agreed everywhere");
}

fn main() {
    let mut report = Report {
        checks: 0,
        failures: 0,
    };
    println!(
        "field     Gf2mField::gf256() = GF(2^8) modulo 0x{:X}",
        Gf2mField::gf256().primitive_polynomial()
    );
    check_table(&mut report, POLY_11D);
    check_table(&mut report, POLY_11B);
    check_round_trip(&mut report);
    check_axpy(&mut report);
    check_pairwise(&mut report);
    check_matmul(&mut report, 16, 16, 16, "field::matrix::gemm, square 16");
    check_matmul(&mut report, 3, 6, 512, "field::matrix::gemm, generator 3x6 by 6x512");
    check_field_distinctness(&mut report);
    println!("\n{} checks, {} failures", report.checks, report.failures);
    if report.failures > 0 {
        std::process::exit(1);
    }
}

/// Silences the unused warning for the vector type the workload module
/// returns without adding a second import path for it.
#[allow(dead_code)]
type Vector = FieldVec<gf2_core::gf2m::Gf2mElement>;
