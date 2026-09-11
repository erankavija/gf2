//! gf2-side adapter validation for the byte-field survey (jit:6c6b09b1).
//!
//! The shared field-law suite covers the two element types themselves
//! (`survey/field-laws` runs `test_field_axioms` over both). This binary
//! covers what that suite cannot: the adapter the survey wraps around gf2,
//! namely the byte-to-element conversion and the consumer entry points the
//! cells measure, for both representations.
//!
//! The oracle is a shift-and-reduce scalar multiply written here, sharing no
//! code with gf2-core, so an entry point and its reference are independent.
//! Operands come from the in-harness SplitMix64 of `byte-field-arm-common`.
//!
//! Exit status is 0 when every check passes and 1 after any mismatch.

use byte_field_arm_common::SplitMix64;
use byte_field_gf2_side::workload::{self, ByteField, RuntimeGf256, WideGf256};
use gf2_core::gf2m::Gf2mField;

const POLY_11D: u32 = 0x11D;
const POLY_11B: u32 = 0x11B;
/// Byte-region analogues of the word boundary cases.
const LENGTHS: [usize; 9] = [0, 1, 31, 32, 63, 64, 65, 128, 4096];
const COEFFICIENTS: [u8; 6] = [0, 1, 2, 3, 0x53, 0xFF];

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

/// Every product of the independent oracle, so the large matrix checks cost
/// one lookup per multiply-accumulate.
fn reference_table(poly: u32) -> Vec<[u8; 256]> {
    (0..=255u8)
        .map(|a| {
            let mut row = [0u8; 256];
            for (b, slot) in row.iter_mut().enumerate() {
                *slot = reference_mul(a, b as u8, poly);
            }
            row
        })
        .collect()
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
        self.checks += 1;
        self.failures += 1;
        println!("FAIL      {what}: {detail}");
    }
}

/// Every ordered pair of elements agrees with the independent oracle.
fn check_table<B: ByteField>(report: &mut Report, field: &B) {
    let poly = field.polynomial();
    let what = format!("{} multiplication table under 0x{poly:X}", B::NAME);
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let got = B::byte(&(field.element(a) * field.element(b)));
            let want = reference_mul(a, b, poly);
            if got != want {
                report.fail(&what, &format!("{a} * {b} = {got}, reference {want}"));
                return;
            }
        }
    }
    report.pass(&what);
}

/// Byte to `FieldVec` and back is lossless, both through a fresh vector and
/// through the in-place overwrite the whole-consumer cells use.
fn check_round_trip<B: ByteField>(report: &mut Report, field: &B) {
    let what = format!("{} byte round trip at the boundary lengths", B::NAME);
    let mut rng = SplitMix64::new(0x6C6B_09B1);
    for length in LENGTHS {
        let mut bytes = vec![0u8; length];
        rng.fill(&mut bytes);
        let mut vector = workload::pack_vec(field, &vec![0u8; length]);
        workload::pack_into_vec(field, &bytes, &mut vector);
        let mut back = vec![0u8; length];
        workload::unpack_vec::<B>(&vector, &mut back);
        if back != bytes {
            report.fail(&what, &format!("length {length} changed"));
            return;
        }
    }
    report.pass(&what);
}

/// `FieldVec::axpy` accumulates `y[i] += a * x[i]` at every boundary length
/// for the edge coefficients, and for every one of the 256 coefficients at
/// one unaligned length.
fn check_axpy<B: ByteField>(report: &mut Report, field: &B) {
    let what = format!(
        "{} FieldVec::axpy against the independent reference",
        B::NAME
    );
    let poly = field.polynomial();
    let mut rng = SplitMix64::new(0x0A7B_9C2D);
    let mut cases: Vec<(usize, u8)> = LENGTHS
        .iter()
        .flat_map(|&length| COEFFICIENTS.iter().map(move |&c| (length, c)))
        .collect();
    cases.extend((0..=255u8).map(|c| (65, c)));
    for (length, coefficient) in cases {
        let mut source = vec![0u8; length];
        let mut target = vec![0u8; length];
        rng.fill(&mut source);
        rng.fill(&mut target);
        let want: Vec<u8> = target
            .iter()
            .zip(&source)
            .map(|(y, x)| y ^ reference_mul(coefficient, *x, poly))
            .collect();
        let x = workload::pack_vec(field, &source);
        let mut y = workload::pack_vec(field, &target);
        workload::axpy(&mut y, &field.element(coefficient), &x);
        let mut got = vec![0u8; length];
        workload::unpack_vec::<B>(&y, &mut got);
        if got != want {
            report.fail(
                &what,
                &format!("coefficient {coefficient} length {length} disagrees"),
            );
            return;
        }
    }
    report.pass(&what);
}

/// `gf2m::batch::batch_mul` multiplies distinct pairs with no coefficient
/// reuse, on the `u64` lanes a byte consumer must widen into.
fn check_pairwise(report: &mut Report, field: &RuntimeGf256) {
    let what = "gf2m::batch::batch_mul against the independent reference";
    let mut rng = SplitMix64::new(0x9A11);
    for length in LENGTHS {
        let mut left = vec![0u8; length];
        let mut right = vec![0u8; length];
        rng.fill(&mut left);
        rng.fill(&mut right);
        let want: Vec<u8> = left
            .iter()
            .zip(&right)
            .map(|(x, y)| reference_mul(*x, *y, POLY_11D))
            .collect();
        let mut lanes_left = vec![0u64; length];
        let mut lanes_right = vec![0u64; length];
        workload::widen_into(&left, &mut lanes_left);
        workload::widen_into(&right, &mut lanes_right);
        let mut lanes_out = vec![0u64; length];
        workload::pairwise(&field.field, &lanes_left, &lanes_right, &mut lanes_out);
        let mut got = vec![0u8; length];
        workload::narrow(&lanes_out, &mut got);
        if got != want {
            report.fail(what, &format!("length {length} disagrees"));
            return;
        }
    }
    report.pass(what);
}

/// `field::matrix::gemm` at square and generator-encode shapes, through the
/// same pack, in-place repack and unpack paths the arm uses.
fn check_product<B: ByteField>(
    report: &mut Report,
    field: &B,
    rows: usize,
    inner: usize,
    cols: usize,
) {
    let what = format!(
        "{} field::matrix::gemm {rows}x{inner} by {inner}x{cols} ({})",
        B::NAME,
        B::gemm_route()
    );
    let table = reference_table(field.polynomial());
    let mut rng = SplitMix64::new(0x6E31 + (rows * 1000 + cols) as u64);
    let mut left = vec![0u8; rows * inner];
    let mut right = vec![0u8; inner * cols];
    rng.fill(&mut left);
    rng.fill(&mut right);
    let mut want = vec![0u8; rows * cols];
    for row in 0..rows {
        for k in 0..inner {
            let products = &table[usize::from(left[row * inner + k])];
            for col in 0..cols {
                want[row * cols + col] ^= products[usize::from(right[k * cols + col])];
            }
        }
    }
    let mut a = workload::pack_matrix(field, &vec![0u8; rows * inner], rows, inner);
    let mut b = workload::pack_matrix(field, &vec![0u8; inner * cols], inner, cols);
    workload::pack_into_matrix(field, &left, &mut a);
    workload::pack_into_matrix(field, &right, &mut b);
    let c = workload::matmul(&a, &b);
    let mut got = vec![0u8; rows * cols];
    workload::unpack_matrix::<B>(&c, &mut got);
    if got == want {
        report.pass(&what);
    } else {
        report.fail(&what, "product disagrees with the independent reference");
    }
}

fn check_representation<B: ByteField>(report: &mut Report, field: &B) {
    println!(
        "field     {} = GF(2^8) modulo 0x{:X}",
        B::NAME,
        field.polynomial()
    );
    check_table(report, field);
    check_round_trip(report, field);
    check_axpy(report, field);
    // Boundary dimensions, the square cells' dimensions and the
    // generator-encode cells' shape.
    for n in [1, 16, 63, 64, 65, 256, 512] {
        check_product(report, field, n, n, n);
    }
    check_product(report, field, 3, 6, 512);
    check_product(report, field, 4, 10, 65536);
}

/// The two degree-8 polynomials this survey meets define different fields,
/// so an adapter fixed at one is never compared against one fixed at the
/// other. gf2 accepts 0x11B only as a caller-supplied runtime polynomial;
/// its multiplication table is checked against the oracle here.
fn check_field_distinctness(report: &mut Report) {
    let at_11b = Gf2mField::new(8, u64::from(POLY_11B));
    let what = "Gf2mField::new(8, 0x11B) multiplication table under 0x11B";
    let mut table_ok = true;
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let got = (at_11b.element(u64::from(a)) * at_11b.element(u64::from(b))).value() as u8;
            if got != reference_mul(a, b, POLY_11B) {
                table_ok = false;
            }
        }
    }
    if table_ok {
        report.pass(what);
    } else {
        report.fail(what, "a product disagrees with the independent reference");
    }
    let at_11d = RuntimeGf256::new();
    let d = RuntimeGf256::byte(&(at_11d.element(2) * at_11d.element(128)));
    let s = (at_11b.element(2) * at_11b.element(128)).value();
    if u64::from(d) != s {
        report.pass(&format!(
            "0x11B and 0x11D are distinct fields: 2 * 128 is {d} under 0x11D and {s} under 0x11B"
        ));
    } else {
        report.fail(
            "field distinctness",
            "2 * 128 agrees under both polynomials",
        );
    }
}

fn main() {
    let mut report = Report {
        checks: 0,
        failures: 0,
    };
    let runtime = RuntimeGf256::new();
    check_representation(&mut report, &runtime);
    check_pairwise(&mut report, &runtime);
    check_representation(&mut report, &WideGf256);
    check_field_distinctness(&mut report);
    println!("\n{} checks, {} failures", report.checks, report.failures);
    if report.failures > 0 {
        std::process::exit(1);
    }
}
