//! SIMD-routed row-XOR matrix multiplication equivalence tests.

mod simd_equiv;

use gf2_core::BitMatrix;
use proptest::prelude::{any, prop_assert_eq};
use proptest::strategy::{Just, Strategy};

use simd_equiv::{assert_simd_matches_scalar, WORD_BOUNDARY_LENGTHS};

#[derive(Clone, Debug, PartialEq)]
struct MatmulFixture {
    rows: usize,
    inner: usize,
    cols: usize,
    lhs_bits: Vec<bool>,
    rhs_bits: Vec<bool>,
}

fn fixture_to_inputs(fixture: &MatmulFixture) -> (BitMatrix, BitMatrix) {
    let mut lhs = BitMatrix::zeros(fixture.rows, fixture.inner);
    for (i, &bit) in fixture.lhs_bits.iter().enumerate() {
        if bit {
            lhs.set(i / fixture.inner, i % fixture.inner, true);
        }
    }

    let mut rhs = BitMatrix::zeros(fixture.inner, fixture.cols);
    for (i, &bit) in fixture.rhs_bits.iter().enumerate() {
        if bit {
            rhs.set(i / fixture.cols, i % fixture.cols, true);
        }
    }

    (lhs, rhs)
}

fn scalar_matmul_reference(lhs: &BitMatrix, rhs: &BitMatrix) -> BitMatrix {
    assert_eq!(
        lhs.cols(),
        rhs.rows(),
        "incompatible dimensions for multiplication"
    );

    let mut out = BitMatrix::zeros(lhs.rows(), rhs.cols());
    for row in 0..lhs.rows() {
        for col in 0..rhs.cols() {
            let mut bit = false;
            for k in 0..lhs.cols() {
                bit ^= lhs.get(row, k) & rhs.get(k, col);
            }
            out.set(row, col, bit);
        }
    }
    out
}

fn patterned_square(n: usize) -> (BitMatrix, BitMatrix) {
    let mut lhs = BitMatrix::zeros(n, n);
    let mut rhs = BitMatrix::zeros(n, n);

    for row in 0..n {
        for col in 0..n {
            lhs.set(row, col, ((row * 17 + col * 31 + n) & 3) == 1);
            rhs.set(row, col, ((row * 19 + col * 29 + n) & 7) <= 2);
        }
    }

    (lhs, rhs)
}

fn matmul_strategy() -> impl Strategy<Value = MatmulFixture> {
    (0usize..=8, 0usize..=32, 0usize..=512).prop_flat_map(|(rows, inner, cols)| {
        let lhs_len = rows * inner;
        let rhs_len = inner * cols;
        (
            Just(rows),
            Just(inner),
            Just(cols),
            proptest::collection::vec(any::<bool>(), lhs_len..=lhs_len),
            proptest::collection::vec(any::<bool>(), rhs_len..=rhs_len),
        )
            .prop_map(|(rows, inner, cols, lhs_bits, rhs_bits)| MatmulFixture {
                rows,
                inner,
                cols,
                lhs_bits,
                rhs_bits,
            })
    })
}

#[test]
fn matmul_word_boundary_lengths_match_scalar_reference() {
    for &n in WORD_BOUNDARY_LENGTHS
        .iter()
        .filter(|&&n| matches!(n, 0 | 1 | 63 | 64 | 65 | 127 | 128 | 129))
    {
        let (lhs, rhs) = patterned_square(n);
        let expected = scalar_matmul_reference(&lhs, &rhs);

        assert_eq!(
            &lhs * &rhs,
            expected,
            "BitMatrix::mul diverged from scalar reference at n={n}"
        );
        assert_eq!(
            lhs.mul_row_xor_for_test(&rhs),
            expected,
            "row-XOR fallback diverged from scalar reference at n={n}"
        );
    }
}

/// jit:bdf60780 — the production M4RM dispatch (`&a * &b`) must stay bit-exact
/// against a naive GF(2) reference at the small-n boundary lengths and at the
/// two parity targets (n=64, n=256), which exercise the lowered register-tiled
/// gate (stride 4) and the new SIMD Gray-table builders (stride 4 / stride 8).
#[test]
fn production_m4rm_matches_scalar_reference_at_smalln_boundaries_and_targets() {
    for &n in &[0usize, 1, 15, 16, 17, 63, 64, 65, 256] {
        let (lhs, rhs) = patterned_square(n);
        let expected = scalar_matmul_reference(&lhs, &rhs);
        assert_eq!(
            &lhs * &rhs,
            expected,
            "production M4RM diverged from scalar reference at n={n}"
        );
    }
}

/// Property-based bit-exactness for the production M4RM path across the small-n
/// boundary lengths {0,1,15,16,17,63,64,65} and the n=64,256 parity targets.
/// Random fill exercises the SIMD Gray-table build (stride 4: n in {193..=256};
/// stride 8: n in {449..=512}) plus the register-tiled C-update.
#[test]
fn production_m4rm_proptest_smalln_boundary_lengths_match_scalar() {
    use proptest::prelude::ProptestConfig;
    use proptest::test_runner::TestRunner;

    let boundary_ns = [1usize, 15, 16, 17, 63, 64, 65, 256];
    let mut runner = TestRunner::new(ProptestConfig::with_cases(48));
    runner
        .run(
            &(proptest::sample::select(boundary_ns.to_vec()), any::<u64>()),
            |(n, seed)| {
                // Deterministic pseudo-random fill from the seed.
                let mut state = seed | 1;
                let mut next_bit = || {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state & 1) == 1
                };
                let mut lhs = BitMatrix::zeros(n, n);
                let mut rhs = BitMatrix::zeros(n, n);
                for r in 0..n {
                    for c in 0..n {
                        if next_bit() {
                            lhs.set(r, c, true);
                        }
                        if next_bit() {
                            rhs.set(r, c, true);
                        }
                    }
                }
                let expected = scalar_matmul_reference(&lhs, &rhs);
                prop_assert_eq!(&lhs * &rhs, expected, "n={}", n);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn row_xor_matmul_matches_scalar_reference_proptest_sizes_0_to_512() {
    assert_simd_matches_scalar::<MatmulFixture, BitMatrix, _, _, _>(
        |fixture| {
            let (lhs, rhs) = fixture_to_inputs(fixture);
            scalar_matmul_reference(&lhs, &rhs)
        },
        |fixture| {
            let (lhs, rhs) = fixture_to_inputs(fixture);
            lhs.mul_row_xor_for_test(&rhs)
        },
        matmul_strategy(),
    );
}
