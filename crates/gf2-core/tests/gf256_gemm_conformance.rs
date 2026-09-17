//! Shared conformance suite for the GF(2^8) cached-product-table dense
//! product.
//!
//! Every path the dense `FieldMatrix` product can take over GF(2^8) runs the
//! same cases here: the cached-table lane that
//! `gf2_core::gf2m::byte_table::gf256_table_dispatch` selects for the
//! runtime-context `Gf2mElement` and for the single-word `Gf2mWide<1, Cfg>`,
//! and the route each representation takes when that dispatch declines. The
//! suite is the behavioural contract those routes hold in common, so a new
//! representation or a new caller joins it rather than growing a private test.
//!
//! # Oracles
//!
//! Two. The route without the table, reached through the process-global force
//! switch [`force_scalar_gf256_table`], is offered identical operands and must
//! produce identical matrices. Independently, a schoolbook product written
//! without the table and without gf2-core arithmetic fixes the mathematics.
//!
//! # Coverage
//!
//! Both the 0x11B and the 0x11D reduction polynomial, over every shape with
//! `m`, `k` and `n` drawn from 0 through 3, and over shapes at and above the
//! product traversal's 32-row by 64-column tiling boundary.

#![cfg(feature = "test-support")]

use std::sync::Mutex;

use gf2_core::field::matrix::{
    gemm, last_gemm_axpy_dispatch_route, reset_last_gemm_axpy_dispatch_route,
    run_gemm_axpy_dispatch_for_test, FieldMatrix, GemmAxpyRoute,
};
use gf2_core::field::triangular::{
    last_effective_trsm_panel_rows, reset_last_effective_trsm_panel_rows, trsm_route, TrsmRoute,
};
use gf2_core::field::FiniteField;
use gf2_core::gf2m::{
    force_scalar_gf256_table, last_gf256_table_lane, Gf2mElement, Gf2mElement_, Gf2mField,
    Gf2mField_, Gf2mWide, Gf2mWideConfig, GF256_SCALAR_LANE, GF256_TABLE_LANE,
};

/// Serialises `force_scalar_gf256_table` toggle-and-observe critical sections
/// across this binary's concurrently-scheduled test threads.
///
/// The override is a single process-wide `AtomicBool`: every route computes
/// the same bytes, so the override never corrupts a result, but a test that
/// asserts *which* lane [`last_gf256_table_lane`] reports can observe another
/// thread's toggle mid-section. Every function here that forces the declining
/// answer or asserts an un-forced lane holds this lock for its whole
/// toggle-execute-observe-restore section, the convention
/// `prime_route_dispatch.rs` uses for the same process-wide-toggle hazard.
static DISPATCH_LANE_MUTEX: Mutex<()> = Mutex::new(());

/// The reduction polynomial `x^8 + x^4 + x^3 + x + 1`, low eight bits.
const POLY_11B: u8 = 0x1b;

/// The reduction polynomial `x^8 + x^4 + x^3 + x^2 + 1`, low eight bits.
const POLY_11D: u8 = 0x1d;

/// The two polynomials every shape sweep runs over.
const SWEEP_POLYNOMIALS: [u8; 2] = [POLY_11B, POLY_11D];

/// GF(2^8) under `x^8 + x^4 + x^3 + x + 1` (0x11B).
struct Gf256Poly11bCfg;
impl Gf2mWideConfig<1> for Gf256Poly11bCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [POLY_11B as u64];
}

/// GF(2^8) under `x^8 + x^4 + x^3 + x^2 + 1` (0x11D).
struct Gf256Poly11dCfg;
impl Gf2mWideConfig<1> for Gf256Poly11dCfg {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [POLY_11D as u64];
}

/// GF(2^16), a single-word configuration the dispatch declines on degree.
struct Gf2m16Cfg;
impl Gf2mWideConfig<1> for Gf2m16Cfg {
    const M: usize = 16;
    const MODULUS: [u64; 1] = [0x002d];
}

/// GF(2^256), a multi-word configuration the dispatch declines on width.
struct Gf2m256Cfg;
impl Gf2mWideConfig<4> for Gf2m256Cfg {
    const M: usize = 256;
    const MODULUS: [u64; 4] = [0x425, 0, 0, 0];
}

/// Independent schoolbook oracle: the carry-less product of two bytes, reduced
/// by the degree-8 modulus bit by bit.
///
/// Names no arithmetic from gf2-core.
fn schoolbook_product(c: u8, v: u8, reduction_low: u8) -> u8 {
    let mut product: u16 = 0;
    for bit in 0..8 {
        if (v >> bit) & 1 == 1 {
            product ^= u16::from(c) << bit;
        }
    }
    let modulus: u16 = 0x100 | u16::from(reduction_low);
    for degree in (8..16).rev() {
        if product & (1u16 << degree) != 0 {
            product ^= modulus << (degree - 8);
        }
    }
    product as u8
}

/// The `m × n` product of an `m × k` and a `k × n` byte matrix under the
/// schoolbook oracle.
fn schoolbook_gemm(a: &[u8], b: &[u8], shape: (usize, usize, usize), reduction_low: u8) -> Vec<u8> {
    let (m, k, n) = shape;
    let mut out = vec![0u8; m * n];
    for i in 0..m {
        for j in 0..n {
            let mut cell = 0u8;
            for p in 0..k {
                cell ^= schoolbook_product(a[i * k + p], b[p * n + j], reduction_low);
            }
            out[i * n + j] = cell;
        }
    }
    out
}

/// SplitMix64, the seeded generator the operand fixtures draw from.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// `count` seeded bytes.
fn seeded_bytes(seed: u64, count: usize) -> Vec<u8> {
    let mut state = seed;
    (0..count).map(|_| splitmix64(&mut state) as u8).collect()
}

/// Shapes the sweep covers: every `m`, `k`, `n` in 0 through 3, then shapes at
/// and above the product traversal's 32-row by 64-column tiling boundary.
fn conformance_shapes() -> Vec<(usize, usize, usize)> {
    let mut shapes = Vec::new();
    for m in 0..=3 {
        for k in 0..=3 {
            for n in 0..=3 {
                shapes.push((m, k, n));
            }
        }
    }
    shapes.extend([
        (5, 7, 3),
        (8, 8, 8),
        (32, 32, 64),
        (33, 17, 65),
        (40, 24, 72),
    ]);
    shapes
}

/// Builds the runtime-context field for a reduction polynomial's low bits.
fn element_field(reduction_low: u8) -> Gf2mField {
    Gf2mField::new(8, 0x100 | u64::from(reduction_low))
}

/// A `rows × cols` runtime-context matrix over `bytes` in row-major order.
fn element_matrix(
    field: &Gf2mField,
    rows: usize,
    cols: usize,
    bytes: &[u8],
) -> FieldMatrix<Gf2mElement> {
    let mut matrix = FieldMatrix::new(rows, cols, field.element(0));
    for i in 0..rows {
        for j in 0..cols {
            matrix.set(i, j, field.element(u64::from(bytes[i * cols + j])));
        }
    }
    matrix
}

/// The row-major bytes of a runtime-context matrix.
fn element_bytes(matrix: &FieldMatrix<Gf2mElement>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(matrix.rows() * matrix.cols());
    for i in 0..matrix.rows() {
        for j in 0..matrix.cols() {
            bytes.push(matrix.get(i, j).value() as u8);
        }
    }
    bytes
}

/// A `rows × cols` single-word wide matrix over `bytes` in row-major order.
fn wide_matrix<Cfg: Gf2mWideConfig<1>>(
    rows: usize,
    cols: usize,
    bytes: &[u8],
) -> FieldMatrix<Gf2mWide<1, Cfg>> {
    let mut matrix = FieldMatrix::new(rows, cols, Gf2mWide::<1, Cfg>::from_u64(0));
    for i in 0..rows {
        for j in 0..cols {
            matrix.set(
                i,
                j,
                Gf2mWide::<1, Cfg>::from_u64(u64::from(bytes[i * cols + j])),
            );
        }
    }
    matrix
}

/// The row-major bytes of a single-word wide matrix.
fn wide_bytes<Cfg: Gf2mWideConfig<1>>(matrix: &FieldMatrix<Gf2mWide<1, Cfg>>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(matrix.rows() * matrix.cols());
    for i in 0..matrix.rows() {
        for j in 0..matrix.cols() {
            bytes.push(matrix.get(i, j).words()[0] as u8);
        }
    }
    bytes
}

/// The runtime-context product for one shape, as bytes.
///
/// `gemm` cannot name the zero of a runtime-context field when both factors
/// carry no storage, so the caller skips `k == 0` with `m > 0` and `n > 0`.
fn element_gemm(
    field: &Gf2mField,
    shape: (usize, usize, usize),
    left: &[u8],
    right: &[u8],
) -> Vec<u8> {
    let (m, k, n) = shape;
    let a = element_matrix(field, m, k, left);
    let b = element_matrix(field, k, n, right);
    element_bytes(&gemm(&a, &b))
}

/// The single-word wide product for one shape, as bytes.
fn wide_gemm<Cfg: Gf2mWideConfig<1>>(
    shape: (usize, usize, usize),
    left: &[u8],
    right: &[u8],
) -> Vec<u8> {
    let (m, k, n) = shape;
    let a = wide_matrix::<Cfg>(m, k, left);
    let b = wide_matrix::<Cfg>(k, n, right);
    wide_bytes(&gemm(&a, &b))
}

/// Whether a shape reaches the whole-product hook at all: `gemm` answers the
/// degenerate shapes before offering the product.
fn reaches_the_hook(shape: (usize, usize, usize)) -> bool {
    shape.0 > 0 && shape.1 > 0 && shape.2 > 0
}

/// Whether a runtime-context product of this shape is well defined.
fn element_shape_is_defined(shape: (usize, usize, usize)) -> bool {
    shape.1 > 0 || shape.0 == 0 || shape.2 == 0
}

// ---------------------------------------------------------------------------
// REQ-02 — the accepted path and the path without it agree
// ---------------------------------------------------------------------------

#[test]
fn the_element_product_agrees_with_the_path_without_the_table() {
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for reduction_low in SWEEP_POLYNOMIALS {
        let field = element_field(reduction_low);
        for shape in conformance_shapes() {
            if !element_shape_is_defined(shape) {
                continue;
            }
            let (m, k, n) = shape;
            let left = seeded_bytes(0x9eed_0001 ^ u64::from(reduction_low), m * k);
            let right = seeded_bytes(0x9eed_0002 ^ u64::from(reduction_low), k * n);

            let restore = force_scalar_gf256_table(true);
            let without = element_gemm(&field, shape, &left, &right);
            force_scalar_gf256_table(restore);
            let with = element_gemm(&field, shape, &left, &right);

            assert_eq!(
                with, without,
                "poly {reduction_low:#04x}, shape {m}x{k}x{n}"
            );
            assert_eq!(
                with,
                schoolbook_gemm(&left, &right, shape, reduction_low),
                "poly {reduction_low:#04x}, shape {m}x{k}x{n} against the schoolbook oracle"
            );
        }
    }

    drop(guard);
}

#[test]
fn the_wide_product_agrees_with_the_path_without_the_table() {
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for shape in conformance_shapes() {
        let (m, k, n) = shape;
        let left = seeded_bytes(0x9eed_0003, m * k);
        let right = seeded_bytes(0x9eed_0004, k * n);

        let restore = force_scalar_gf256_table(true);
        let without_b = wide_gemm::<Gf256Poly11bCfg>(shape, &left, &right);
        let without_d = wide_gemm::<Gf256Poly11dCfg>(shape, &left, &right);
        force_scalar_gf256_table(restore);
        let with_b = wide_gemm::<Gf256Poly11bCfg>(shape, &left, &right);
        let with_d = wide_gemm::<Gf256Poly11dCfg>(shape, &left, &right);

        assert_eq!(with_b, without_b, "0x11B, shape {m}x{k}x{n}");
        assert_eq!(with_d, without_d, "0x11D, shape {m}x{k}x{n}");
        assert_eq!(
            with_b,
            schoolbook_gemm(&left, &right, shape, POLY_11B),
            "0x11B, shape {m}x{k}x{n} against the schoolbook oracle"
        );
        assert_eq!(
            with_d,
            schoolbook_gemm(&left, &right, shape, POLY_11D),
            "0x11D, shape {m}x{k}x{n} against the schoolbook oracle"
        );
    }

    drop(guard);
}

#[test]
fn the_two_representations_agree_with_each_other_over_both_polynomials() {
    for shape in conformance_shapes() {
        if !element_shape_is_defined(shape) {
            continue;
        }
        let (m, k, n) = shape;
        let left = seeded_bytes(0x9eed_0005, m * k);
        let right = seeded_bytes(0x9eed_0006, k * n);

        assert_eq!(
            element_gemm(&element_field(POLY_11B), shape, &left, &right),
            wide_gemm::<Gf256Poly11bCfg>(shape, &left, &right),
            "0x11B, shape {m}x{k}x{n}"
        );
        assert_eq!(
            element_gemm(&element_field(POLY_11D), shape, &left, &right),
            wide_gemm::<Gf256Poly11dCfg>(shape, &left, &right),
            "0x11D, shape {m}x{k}x{n}"
        );
    }
}

// ---------------------------------------------------------------------------
// REQ-03 — zero, identity and single-column operands
// ---------------------------------------------------------------------------

#[test]
fn a_zero_operand_gives_the_zero_matrix() {
    let shape = (9usize, 7usize, 5usize);
    let (m, k, n) = shape;
    let values = seeded_bytes(0x9eed_0007, m * k.max(n));
    let zeros = vec![0u8; k * n.max(m)];

    for reduction_low in SWEEP_POLYNOMIALS {
        let field = element_field(reduction_low);
        assert_eq!(
            element_gemm(&field, shape, &values[..m * k], &zeros[..k * n]),
            vec![0u8; m * n]
        );
        assert_eq!(
            element_gemm(&field, (k, m, n), &zeros[..k * m], &values[..m * n]),
            vec![0u8; k * n]
        );
    }
    assert_eq!(
        wide_gemm::<Gf256Poly11dCfg>(shape, &values[..m * k], &zeros[..k * n]),
        vec![0u8; m * n]
    );
}

#[test]
fn an_identity_operand_reproduces_the_other_operand() {
    let (m, k, n) = (9usize, 9usize, 9usize);
    let values = seeded_bytes(0x9eed_0008, m * k);
    let mut identity = vec![0u8; k * n];
    for i in 0..k {
        identity[i * n + i] = 1;
    }

    for reduction_low in SWEEP_POLYNOMIALS {
        let field = element_field(reduction_low);
        assert_eq!(
            element_gemm(&field, (m, k, n), &values, &identity),
            values,
            "poly {reduction_low:#04x}, A · I"
        );
        assert_eq!(
            element_gemm(&field, (m, k, n), &identity, &values),
            values,
            "poly {reduction_low:#04x}, I · A"
        );
    }
    assert_eq!(
        wide_gemm::<Gf256Poly11dCfg>((m, k, n), &values, &identity),
        values
    );
    assert_eq!(
        wide_gemm::<Gf256Poly11dCfg>((m, k, n), &identity, &values),
        values
    );
}

#[test]
fn a_single_column_operand_gives_the_schoolbook_dot_products() {
    let shape = (11usize, 13usize, 1usize);
    let (m, k, n) = shape;
    let left = seeded_bytes(0x9eed_0009, m * k);
    let right = seeded_bytes(0x9eed_000a, k * n);

    for reduction_low in SWEEP_POLYNOMIALS {
        let expected = schoolbook_gemm(&left, &right, shape, reduction_low);
        assert_eq!(expected.len(), m);
        assert_eq!(
            element_gemm(&element_field(reduction_low), shape, &left, &right),
            expected,
            "poly {reduction_low:#04x}"
        );
    }
    assert_eq!(
        wide_gemm::<Gf256Poly11bCfg>(shape, &left, &right),
        schoolbook_gemm(&left, &right, shape, POLY_11B)
    );
    assert_eq!(
        wide_gemm::<Gf256Poly11dCfg>(shape, &left, &right),
        schoolbook_gemm(&left, &right, shape, POLY_11D)
    );
}

// ---------------------------------------------------------------------------
// REQ-01 and REQ-06 — what the lane accepts, what it declines, and the witness
// ---------------------------------------------------------------------------

#[test]
fn the_table_lane_accepts_both_single_word_gf256_representations() {
    let shape = (9usize, 7usize, 5usize);
    let (m, k, n) = shape;
    let left = seeded_bytes(0x9eed_000b, m * k);
    let right = seeded_bytes(0x9eed_000c, k * n);

    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let _ = element_gemm(&element_field(POLY_11D), shape, &left, &right);
    let element_lane = last_gf256_table_lane();
    let _ = wide_gemm::<Gf256Poly11dCfg>(shape, &left, &right);
    let wide_lane = last_gf256_table_lane();

    let restore = force_scalar_gf256_table(true);
    let _ = element_gemm(&element_field(POLY_11D), shape, &left, &right);
    let forced_element_lane = last_gf256_table_lane();
    let _ = wide_gemm::<Gf256Poly11dCfg>(shape, &left, &right);
    let forced_wide_lane = last_gf256_table_lane();
    force_scalar_gf256_table(restore);

    drop(guard);

    assert_eq!(element_lane, GF256_TABLE_LANE);
    assert_eq!(wide_lane, GF256_TABLE_LANE);
    assert_eq!(forced_element_lane, GF256_SCALAR_LANE);
    assert_eq!(forced_wide_lane, GF256_SCALAR_LANE);
}

#[test]
fn every_shape_that_reaches_the_hook_takes_the_table_lane() {
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for shape in conformance_shapes() {
        if !reaches_the_hook(shape) {
            continue;
        }
        let (m, k, n) = shape;
        let left = seeded_bytes(0x9eed_000d, m * k);
        let right = seeded_bytes(0x9eed_000e, k * n);

        let _ = element_gemm(&element_field(POLY_11B), shape, &left, &right);
        assert_eq!(
            last_gf256_table_lane(),
            GF256_TABLE_LANE,
            "element, shape {m}x{k}x{n}"
        );
        let _ = wide_gemm::<Gf256Poly11bCfg>(shape, &left, &right);
        assert_eq!(
            last_gf256_table_lane(),
            GF256_TABLE_LANE,
            "wide, shape {m}x{k}x{n}"
        );
    }

    drop(guard);
}

/// The product of two `FiniteField` matrices, cell by cell through the field's
/// own multiply and add.
fn naive_gemm<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> Vec<F> {
    let mut out = Vec::with_capacity(a.rows() * b.cols());
    for i in 0..a.rows() {
        for j in 0..b.cols() {
            let mut cell = a.get(i, 0).zero_like();
            for p in 0..a.cols() {
                cell += a.get(i, p) * b.get(p, j);
            }
            out.push(cell);
        }
    }
    out
}

/// Builds two matrices of the given shape from a seeded value pattern, takes
/// their product, and returns it beside the naive product and the lane the
/// call reported.
fn declined_case<F: FiniteField>(
    shape: (usize, usize, usize),
    value: impl Fn(u64) -> F,
) -> (Vec<F>, Vec<F>, &'static str) {
    let (m, k, n) = shape;
    let mut a = FieldMatrix::new(m, k, value(0));
    for i in 0..m {
        for j in 0..k {
            a.set(i, j, value((i * k + j) as u64 * 7 + 1));
        }
    }
    let mut b = FieldMatrix::new(k, n, value(0));
    for i in 0..k {
        for j in 0..n {
            b.set(i, j, value((i * n + j) as u64 * 11 + 3));
        }
    }
    let product = gemm(&a, &b);
    let lane = last_gf256_table_lane();
    let mut cells = Vec::with_capacity(m * n);
    for i in 0..m {
        for j in 0..n {
            cells.push(product.get(i, j));
        }
    }
    (cells, naive_gemm(&a, &b), lane)
}

#[test]
fn another_degree_declines_and_keeps_its_result() {
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for (m, poly) in [(4usize, 0b1_0011u64), (16, 0b1_0000_0000_0010_1101)] {
        let field = Gf2mField::new(m, poly);
        let mask = (1u64 << m) - 1;
        let (product, naive, lane) = declined_case((9, 7, 5), |v| field.element(v & mask));
        assert_eq!(lane, GF256_SCALAR_LANE, "degree {m}");
        assert_eq!(product, naive, "degree {m}");
    }

    drop(guard);
}

#[test]
fn another_backing_width_declines_and_keeps_its_result() {
    let field = Gf2mField_::<u128>::new(8, 0x11d);
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let (product, naive, lane) =
        declined_case::<Gf2mElement_<u128>>((9, 7, 5), |v| field.element(u128::from(v) & 0xff));

    drop(guard);

    assert_eq!(lane, GF256_SCALAR_LANE);
    assert_eq!(product, naive);
}

#[test]
fn a_multi_word_configuration_declines_and_keeps_its_result() {
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let (product, naive, lane) = declined_case((9, 7, 5), Gf2mWide::<4, Gf2m256Cfg>::from_u64);

    drop(guard);

    assert_eq!(lane, GF256_SCALAR_LANE);
    assert_eq!(product, naive);
}

#[test]
fn a_single_word_configuration_of_another_degree_declines_and_keeps_its_result() {
    let guard = DISPATCH_LANE_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let (product, naive, lane) = declined_case((9, 7, 5), |v| {
        Gf2mWide::<1, Gf2m16Cfg>::from_u64(v & 0xffff)
    });

    drop(guard);

    assert_eq!(lane, GF256_SCALAR_LANE);
    assert_eq!(product, naive);
}

// ---------------------------------------------------------------------------
// REQ-04 — the output matrix keeps its element type, layout and signature
// ---------------------------------------------------------------------------

#[test]
fn the_output_keeps_its_shape_layout_and_field_handles() {
    let shape = (9usize, 7usize, 5usize);
    let (m, k, n) = shape;
    let left = seeded_bytes(0x9eed_000f, m * k);
    let right = seeded_bytes(0x9eed_0010, k * n);
    let field = element_field(POLY_11D);

    let product: FieldMatrix<Gf2mElement> = gemm(
        &element_matrix(&field, m, k, &left),
        &element_matrix(&field, k, n, &right),
    );

    assert_eq!((product.rows(), product.cols()), (m, n));
    assert_eq!(
        element_bytes(&product),
        schoolbook_gemm(&left, &right, shape, POLY_11D),
        "row-major cell order"
    );
    for i in 0..m {
        for j in 0..n {
            assert_eq!(product.get(i, j).field(), field, "cell {i},{j}");
        }
    }
}

// ---------------------------------------------------------------------------
// REQ-05 — the availability probe keeps its declining answer
// ---------------------------------------------------------------------------

#[test]
fn the_availability_probe_declines_for_both_representations() {
    assert!(!<Gf2mElement as FiniteField>::has_simd_gemm_classical());
    assert!(!<Gf2mWide<1, Gf256Poly11bCfg> as FiniteField>::has_simd_gemm_classical());
    assert!(!<Gf2mWide<1, Gf256Poly11dCfg> as FiniteField>::has_simd_gemm_classical());
}

#[test]
fn the_gemm_axpy_fold_keeps_its_per_cell_route() {
    let field = element_field(POLY_11D);
    let n = 16;
    let bytes = seeded_bytes(0x9eed_0011, n * n);
    let a = element_matrix(&field, n, n, &bytes);
    let b = element_matrix(&field, n, n, &bytes);
    let mut out = element_matrix(&field, n, n, &vec![0u8; n * n]);

    reset_last_gemm_axpy_dispatch_route();
    run_gemm_axpy_dispatch_for_test(&a, &b, &mut out);

    assert_eq!(
        last_gemm_axpy_dispatch_route(),
        Some(GemmAxpyRoute::PerCell),
        "the probe gates this fold and keeps declining for GF(2^8)"
    );
    assert_eq!(element_bytes(&out), element_bytes(&gemm(&a, &b)));
}

#[test]
fn the_blocked_triangular_solve_stays_unselected() {
    let field = element_field(POLY_11D);
    let n = 64;
    assert_eq!(
        trsm_route(n),
        TrsmRoute::Blocked,
        "the dimension must be one the probe would route to the blocked solve"
    );

    let mut a = FieldMatrix::new(n, n, field.element(0));
    for i in 0..n {
        a.set(i, i, field.element(1));
    }
    let b = element_matrix(&field, n, 2, &seeded_bytes(0x9eed_0012, n * 2));

    reset_last_effective_trsm_panel_rows();
    let solution = a.solve_batch(&b).expect("the identity is non-singular");

    assert_eq!(
        last_effective_trsm_panel_rows(),
        None,
        "the probe gates the blocked solve and keeps declining for GF(2^8)"
    );
    assert_eq!(element_bytes(&solution), element_bytes(&b));
}
