//! The one-byte GF(2^8) element `Gf256<Cfg>` on the shared field-law suite,
//! and the generic `FieldVec` and `FieldMatrix` API over it against the same
//! operations over `Gf2mWide<1, Cfg>` and against the byte-region API, under
//! the 0x11B and 0x11D moduli.

#![cfg(feature = "test-support")]

use gf2_core::field::axiom_tests::test_const_field_axioms;
use gf2_core::field::matrix::{gemm, FieldMatrix};
use gf2_core::field::FieldVec;
use gf2_core::gf2m::{Gf256, Gf2mField, Gf2mWide, Gf2mWideConfig};
use proptest::prelude::*;

struct Poly11b;
impl Gf2mWideConfig<1> for Poly11b {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1b];
}

struct Poly11d;
impl Gf2mWideConfig<1> for Poly11d {
    const M: usize = 8;
    const MODULUS: [u64; 1] = [0x1d];
}

fn lcg(state: &mut u64) -> u8 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 56) as u8
}

fn narrow<Cfg: Gf2mWideConfig<1>>(bytes: &[u8]) -> FieldVec<Gf256<Cfg>> {
    bytes.iter().map(|&b| Gf256::new(b)).collect()
}

fn wide<Cfg: Gf2mWideConfig<1>>(bytes: &[u8]) -> FieldVec<Gf2mWide<1, Cfg>> {
    bytes
        .iter()
        .map(|&b| Gf2mWide::from_u64(b.into()))
        .collect()
}

fn narrow_bytes<Cfg: Gf2mWideConfig<1>>(v: &FieldVec<Gf256<Cfg>>) -> Vec<u8> {
    v.iter().map(|e| e.value()).collect()
}

fn wide_bytes<Cfg: Gf2mWideConfig<1>>(v: &FieldVec<Gf2mWide<1, Cfg>>) -> Vec<u8> {
    v.iter().map(|e| e.words()[0] as u8).collect()
}

fn matrix<F: gf2_core::field::FiniteField>(
    rows: usize,
    cols: usize,
    values: Vec<F>,
) -> FieldMatrix<F> {
    FieldMatrix::from_rows(
        values
            .chunks(cols.max(1))
            .take(rows)
            .map(|r| r.to_vec().into())
            .collect(),
    )
}

const LENGTHS: [usize; 18] = [
    0, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 2051,
];

#[test]
fn the_element_is_one_byte_and_passes_the_field_laws() {
    assert_eq!(core::mem::size_of::<Gf256<Poly11d>>(), 1);
    test_const_field_axioms(any::<u8>().prop_map(Gf256::<Poly11b>::new).boxed(), 2);
    test_const_field_axioms(any::<u8>().prop_map(Gf256::<Poly11d>::new).boxed(), 2);
}

fn vector_operations_agree<Cfg: Gf2mWideConfig<1>>(field: &Gf2mField, seed: u64) {
    let mut state = seed;
    for len in LENGTHS {
        let c = lcg(&mut state);
        let x: Vec<u8> = (0..len).map(|_| lcg(&mut state)).collect();
        let y: Vec<u8> = (0..len).map(|_| lcg(&mut state)).collect();

        let mut narrow_y = narrow::<Cfg>(&y);
        narrow_y.axpy(&Gf256::new(c), &narrow::<Cfg>(&x));
        let mut wide_y = wide::<Cfg>(&y);
        wide_y.axpy(&Gf2mWide::from_u64(c.into()), &wide::<Cfg>(&x));
        let mut bytes = y.clone();
        field.gf256_axpy(c, &x, &mut bytes);
        assert_eq!(
            narrow_bytes(&narrow_y),
            wide_bytes(&wide_y),
            "axpy len {len}"
        );
        assert_eq!(narrow_bytes(&narrow_y), bytes, "axpy len {len}");

        // The dot product rejects empty vectors.
        if len == 0 {
            continue;
        }
        let dot = narrow::<Cfg>(&x).dot_product(&narrow::<Cfg>(&y));
        let wide_dot = wide::<Cfg>(&x).dot_product(&wide::<Cfg>(&y));
        assert_eq!(Gf2mWide::from(dot), wide_dot, "dot len {len}");
    }
}

#[test]
fn vector_operations_match_the_wide_representation_and_the_byte_api() {
    vector_operations_agree::<Poly11d>(&Gf2mField::gf256(), 1);
    vector_operations_agree::<Poly11b>(&Gf2mField::new(8, 0x11b), 2);
}

fn matrix_operations_agree<Cfg: Gf2mWideConfig<1>>(field: &Gf2mField, seed: u64) {
    let mut state = seed;
    for (m, k, n) in [
        (1, 1, 1),
        (3, 7, 31),
        (5, 33, 65),
        (4, 257, 3),
        (9, 70, 1030),
    ] {
        let a: Vec<u8> = (0..m * k).map(|_| lcg(&mut state)).collect();
        let b: Vec<u8> = (0..k * n).map(|_| lcg(&mut state)).collect();
        let x: Vec<u8> = (0..k).map(|_| lcg(&mut state)).collect();

        let narrow_a = matrix(m, k, narrow::<Cfg>(&a).into_iter().collect());
        let narrow_b = matrix(k, n, narrow::<Cfg>(&b).into_iter().collect());
        let wide_a = matrix(m, k, wide::<Cfg>(&a).into_iter().collect());
        let wide_b = matrix(k, n, wide::<Cfg>(&b).into_iter().collect());

        let product = gemm(&narrow_a, &narrow_b);
        let wide_product = gemm(&wide_a, &wide_b);
        let mut bytes = vec![0u8; m * n];
        field.gf256_gemm(m, k, n, &a, &b, &mut bytes);
        for i in 0..m {
            for j in 0..n {
                let value = product.get(i, j).value();
                assert_eq!(
                    value,
                    wide_product.get(i, j).words()[0] as u8,
                    "{m}x{k}x{n}"
                );
                assert_eq!(value, bytes[i * n + j], "{m}x{k}x{n}");
            }
        }

        let image = narrow_a.matvec(&narrow::<Cfg>(&x));
        let wide_image = wide_a.matvec(&wide::<Cfg>(&x));
        assert_eq!(
            narrow_bytes(&image),
            wide_bytes(&wide_image),
            "matvec {m}x{k}"
        );
    }
}

#[test]
fn matrix_operations_match_the_wide_representation_and_the_byte_api() {
    matrix_operations_agree::<Poly11d>(&Gf2mField::gf256(), 3);
    matrix_operations_agree::<Poly11b>(&Gf2mField::new(8, 0x11b), 4);
}

/// Shapes with an empty dimension, each paired with an extent past one or both
/// cache blocks of the product.
const DEGENERATE_SHAPES: [(usize, usize, usize); 9] = [
    (0, 257, 5),
    (0, 3, 1025),
    (0, 257, 1025),
    (4, 0, 1025),
    (0, 0, 1025),
    (4, 257, 0),
    (0, 257, 0),
    (1025, 0, 0),
    (5, 0, 3),
];

fn degenerate_products_are_zero<Cfg: Gf2mWideConfig<1>>() {
    use gf2_core::field::FiniteField;
    for (m, k, n) in DEGENERATE_SHAPES {
        let a = FieldMatrix::new(m, k, Gf256::<Cfg>::new(0x37));
        let b = FieldMatrix::new(k, n, Gf256::<Cfg>::new(0x91));
        let product = gemm(&a, &b);
        assert_eq!((product.rows(), product.cols()), (m, n), "{m}x{k}x{n}");
        for i in 0..m {
            for j in 0..n {
                assert!(product.get(i, j).is_zero(), "{m}x{k}x{n} at {i},{j}");
            }
        }

        // The whole-product hooks themselves, which the generic product
        // guards against these shapes.
        let a = vec![Gf256::<Cfg>::new(0x37); m * k];
        let b = vec![Gf256::<Cfg>::new(0x91); k * n];
        let mut out = vec![Gf256::<Cfg>::new(0xc3); m * n];
        assert!(Gf256::try_simd_gemm_row_major(&a, &b, m, k, n, &mut out));
        assert!(out.iter().all(Gf256::is_zero), "row-major {m}x{k}x{n}");
        let mut out = vec![Gf256::<Cfg>::new(0xc3); m * n];
        assert!(Gf256::try_simd_gemm_classical(&a, &b, m, k, n, &mut out));
        assert!(out.iter().all(Gf256::is_zero), "classical {m}x{k}x{n}");
    }
}

#[test]
fn degenerate_products_are_the_empty_or_zero_matrix() {
    degenerate_products_are_zero::<Poly11d>();
    degenerate_products_are_zero::<Poly11b>();
}

#[test]
fn the_wide_representation_round_trips() {
    for v in 0..=255u8 {
        let element = Gf256::<Poly11d>::new(v);
        assert_eq!(Gf256::from(Gf2mWide::from(element)), element);
    }
}
