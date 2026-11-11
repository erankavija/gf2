//! The GF(2^8) byte-region APIs against the field's own element arithmetic,
//! on the kernels the build and host dispatch: region lengths at the word and
//! vector boundaries, unaligned windows, every constant, both moduli, and
//! products across the cache-block edges.

use gf2_core::gf2m::{Gf256GemmBlocking, Gf2mField};
use std::num::NonZeroUsize;

fn lcg(state: &mut u64) -> u8 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 56) as u8
}

fn fields() -> [Gf2mField; 2] {
    [Gf2mField::gf256(), Gf2mField::new(8, 0x11b)]
}

fn product(field: &Gf2mField, a: u8, b: u8) -> u8 {
    (field.element(u64::from(a)) * field.element(u64::from(b))).value() as u8
}

const LENGTHS: [usize; 20] = [
    0, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 95, 96, 127, 128, 129, 4099,
];

#[test]
fn axpy_matches_element_arithmetic_at_every_boundary_length() {
    let mut state = 1;
    for field in fields() {
        for len in LENGTHS {
            for (x_offset, y_offset) in [(0, 0), (1, 0), (0, 3), (5, 7)] {
                let c = lcg(&mut state);
                let x: Vec<u8> = (0..len + x_offset).map(|_| lcg(&mut state)).collect();
                let y: Vec<u8> = (0..len + y_offset).map(|_| lcg(&mut state)).collect();
                let mut got = y.clone();
                field.gf256_axpy(c, &x[x_offset..], &mut got[y_offset..]);
                assert_eq!(got[..y_offset], y[..y_offset]);
                for i in 0..len {
                    let expected = y[y_offset + i] ^ product(&field, c, x[x_offset + i]);
                    assert_eq!(got[y_offset + i], expected, "len {len} index {i}");
                }
            }
        }
    }
}

#[test]
fn axpy_matches_element_arithmetic_for_every_constant() {
    let x: Vec<u8> = (0..=255).collect();
    for field in fields() {
        for c in 0..=255u8 {
            let mut y = vec![0x5a; 256];
            field.gf256_axpy(c, &x, &mut y);
            for (v, &got) in y.iter().enumerate() {
                assert_eq!(got, 0x5a ^ product(&field, c, v as u8));
            }
        }
    }
}

#[test]
fn gemm_matches_element_arithmetic_across_block_edges() {
    let mut state = 2;
    for field in fields() {
        for (m, k, n) in [
            (0, 4, 5),
            (4, 0, 5),
            (4, 5, 0),
            (1, 1, 1),
            (3, 7, 31),
            (5, 33, 65),
            (7, 257, 3),
            (2, 3, 1025),
            (9, 260, 1057),
        ] {
            let a: Vec<u8> = (0..m * k).map(|_| lcg(&mut state)).collect();
            let b: Vec<u8> = (0..k * n).map(|_| lcg(&mut state)).collect();
            let mut c = vec![0xc3; m * n];
            field.gf256_gemm(m, k, n, &a, &b, &mut c);
            for i in 0..m {
                for j in 0..n {
                    let expected = (0..k).fold(0, |sum, p| {
                        sum ^ product(&field, a[i * k + p], b[p * n + j])
                    });
                    assert_eq!(c[i * n + j], expected, "{m}x{k}x{n} at {i},{j}");
                }
            }
        }
    }
}

#[test]
fn gemm_gives_one_product_under_every_blocking() {
    let mut state = 3;
    let blocking = |depth, width| {
        Gf256GemmBlocking::new(
            NonZeroUsize::new(depth).unwrap(),
            NonZeroUsize::new(width).unwrap(),
        )
    };
    let structural = Gf256GemmBlocking::structural();
    for field in fields() {
        for (m, k, n) in [
            (5, 33, 65),
            (9, structural.depth() + 1, 70),
            (3, 7, structural.width() + 1),
        ] {
            let a: Vec<u8> = (0..m * k).map(|_| lcg(&mut state)).collect();
            let b: Vec<u8> = (0..k * n).map(|_| lcg(&mut state)).collect();
            let mut expected = vec![0; m * n];
            field.gf256_gemm(m, k, n, &a, &b, &mut expected);
            for blocking in [
                blocking(1, 1),
                blocking(3, 33),
                blocking(16, 32),
                blocking(k, n),
            ] {
                let mut c = vec![0x5a; m * n];
                field.gf256_gemm_with(m, k, n, &a, &b, &mut c, blocking);
                assert_eq!(c, expected, "{m}x{k}x{n} under {blocking:?}");
            }
        }
    }
}

/// Shapes with an empty dimension, each paired with an extent past one or both
/// cache blocks of the product, so the blocked traversal steps over a block
/// edge of an empty operand.
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

#[test]
fn degenerate_products_write_the_empty_or_zero_matrix() {
    for field in fields() {
        for (m, k, n) in DEGENERATE_SHAPES {
            let a = vec![0x37u8; m * k];
            let b = vec![0x91u8; k * n];
            let mut c = vec![0xc3u8; m * n];
            field.gf256_gemm(m, k, n, &a, &b, &mut c);
            assert!(c.iter().all(|&v| v == 0), "{m}x{k}x{n}");
        }
    }
}

#[test]
#[should_panic(expected = "region lengths differ")]
fn axpy_rejects_regions_of_different_lengths() {
    Gf2mField::gf256().gf256_axpy(3, &[0; 33], &mut [0; 32]);
}

#[test]
#[should_panic(expected = "c holds m * n bytes")]
fn gemm_rejects_a_short_output() {
    Gf2mField::gf256().gf256_gemm(2, 2, 2, &[0; 4], &[0; 4], &mut [0; 3]);
}

#[test]
#[should_panic(expected = "need a GF(2^8) field")]
fn the_byte_operations_reject_other_degrees() {
    Gf2mField::gf65536().gf256_axpy(3, &[0; 4], &mut [0; 4]);
}
