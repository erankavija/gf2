//! `BitMatrix` inversion over GF(2).

use gf2_core::alg::gauss::{invert, invert_m4ri, invert_scalar};
use gf2_core::alg::m4rm::multiply;
use gf2_core::matrix::BitMatrix;
use proptest::prelude::*;

#[test]
fn test_invert_identity() {
    let id = BitMatrix::identity(3);
    let inv = invert(&id).expect("identity should be invertible");

    assert_eq!(inv.rows(), 3);
    assert_eq!(inv.cols(), 3);

    for r in 0..3 {
        for c in 0..3 {
            assert_eq!(
                inv.get(r, c),
                r == c,
                "inv at ({}, {}) should be {}",
                r,
                c,
                r == c
            );
        }
    }
}

#[test]
fn test_invert_singular_zero() {
    let m = BitMatrix::zeros(3, 3);
    let inv = invert(&m);

    assert!(inv.is_none(), "zero matrix should not be invertible");
}

#[test]
fn test_invert_singular_duplicate_rows() {
    let mut m = BitMatrix::zeros(3, 3);
    m.set(0, 0, true);
    m.set(0, 1, true);
    m.set(1, 0, true);
    m.set(1, 1, true);
    m.set(2, 2, true);

    let inv = invert(&m);

    assert!(
        inv.is_none(),
        "matrix with duplicate rows should not be invertible"
    );
}

#[test]
fn test_invert_simple_2x2() {
    // [[1, 1], [1, 0]]
    let mut m = BitMatrix::zeros(2, 2);
    m.set(0, 0, true);
    m.set(0, 1, true);
    m.set(1, 0, true);

    let inv = invert(&m).expect("should be invertible");

    let product = multiply(&m, &inv);

    for r in 0..2 {
        for c in 0..2 {
            let expected = r == c;
            assert_eq!(
                product.get(r, c),
                expected,
                "m × inv at ({}, {}) should be {}",
                r,
                c,
                expected
            );
        }
    }
}

#[test]
fn test_invert_and_verify_3x3() {
    // [[1,1,1], [1,0,1], [1,1,0]]
    let mut m = BitMatrix::zeros(3, 3);
    m.set(0, 0, true);
    m.set(0, 1, true);
    m.set(0, 2, true);
    m.set(1, 0, true);
    m.set(1, 2, true);
    m.set(2, 0, true);
    m.set(2, 1, true);

    let inv = invert(&m).expect("should be invertible");

    let product = multiply(&m, &inv);

    for r in 0..3 {
        for c in 0..3 {
            assert_eq!(product.get(r, c), r == c);
        }
    }
}

#[test]
fn test_invert_random_4x4() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(42);

    for _attempt in 0..10 {
        let mut m = BitMatrix::identity(4);

        for _ in 0..6 {
            let r = rng.gen_range(0..4);
            let c = rng.gen_range(0..4);
            m.set(r, c, !m.get(r, c));
        }

        if let Some(inv) = invert(&m) {
            let product = multiply(&m, &inv);

            for r in 0..4 {
                for c in 0..4 {
                    assert_eq!(product.get(r, c), r == c, "failed at ({}, {})", r, c);
                }
            }
            return;
        }
    }

    panic!("couldn't generate invertible matrix in 10 attempts");
}

#[test]
fn test_invert_random_8x8() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(123);

    for _attempt in 0..20 {
        let mut m = BitMatrix::identity(8);

        for _ in 0..20 {
            let r = rng.gen_range(0..8);
            let c = rng.gen_range(0..8);
            m.set(r, c, !m.get(r, c));
        }

        if let Some(inv) = invert(&m) {
            let product = multiply(&m, &inv);

            for r in 0..8 {
                for c in 0..8 {
                    assert_eq!(product.get(r, c), r == c);
                }
            }
            return;
        }
    }

    panic!("couldn't generate invertible matrix in 20 attempts");
}

#[test]
fn test_invert_various_sizes() {
    let mut m1 = BitMatrix::identity(1);
    m1.set(0, 0, true);
    let inv1 = invert(&m1).expect("1x1 [1] should be invertible");
    assert!(inv1.get(0, 0));

    let m5 = BitMatrix::identity(5);
    let inv5 = invert(&m5).expect("5x5 identity should be invertible");

    for r in 0..5 {
        for c in 0..5 {
            assert_eq!(inv5.get(r, c), r == c);
        }
    }
}

#[test]
fn test_invert_non_square() {
    let m = BitMatrix::zeros(3, 4);
    let inv = invert(&m);

    assert!(inv.is_none(), "non-square matrix should not be invertible");
}

#[test]
fn test_invert_property_double_inverse() {
    // [[1,1,1], [1,0,1], [1,1,0]]
    let mut m = BitMatrix::zeros(3, 3);
    m.set(0, 0, true);
    m.set(0, 1, true);
    m.set(0, 2, true);
    m.set(1, 0, true);
    m.set(1, 2, true);
    m.set(2, 0, true);
    m.set(2, 1, true);

    let inv = invert(&m).expect("should be invertible");
    let inv_inv = invert(&inv).expect("inverse should also be invertible");

    for r in 0..3 {
        for c in 0..3 {
            assert_eq!(inv_inv.get(r, c), m.get(r, c));
        }
    }
}

fn boundary_sizes() -> Vec<usize> {
    vec![0, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129]
}

#[test]
fn test_invert_m4ri_bit_exact_with_scalar_on_boundaries() {
    // Identity is invertible at every boundary, so this exercises every size
    // without needing seed-driven random search.
    for n in boundary_sizes() {
        let id = BitMatrix::identity(n);
        let scalar = invert_scalar(&id);
        let m4ri = invert_m4ri(&id);
        assert_eq!(scalar, m4ri, "identity at n={n}: scalar vs m4ri");
        assert_eq!(
            invert(&id),
            scalar,
            "dispatch at n={n}: must equal scalar (== m4ri)"
        );
    }
}

proptest! {
    #[test]
    fn prop_invert_m4ri_equals_gauss(
        n in prop_oneof![
            Just(1usize), Just(7), Just(8), Just(9), Just(15), Just(16),
            Just(31), Just(32), Just(63), Just(64), Just(65), Just(127),
            Just(128), Just(129),
        ],
        seed in any::<u64>(),
    ) {
        let m = BitMatrix::random_seeded(n, n, seed);
        let scalar = invert_scalar(&m);
        let m4ri = invert_m4ri(&m);
        prop_assert_eq!(scalar.clone(), m4ri.clone(),
            "scalar vs m4ri mismatch at n={}, seed={}", n, seed);
        if let Some(inv) = m4ri {
            let product = multiply(&m, &inv);
            let id = BitMatrix::identity(n);
            prop_assert_eq!(product, id,
                "m × m^-1 ≠ I at n={}, seed={}", n, seed);
        } else {
            prop_assert!(scalar.is_none());
        }
    }
}

#[test]
fn test_invert_permutation_matrix() {
    let mut m = BitMatrix::zeros(3, 3);
    m.set(0, 2, true);
    m.set(1, 1, true);
    m.set(2, 0, true);

    let inv = invert(&m).expect("permutation matrix should be invertible");

    let product = multiply(&m, &inv);
    for r in 0..3 {
        for c in 0..3 {
            assert_eq!(product.get(r, c), r == c);
        }
    }
}
