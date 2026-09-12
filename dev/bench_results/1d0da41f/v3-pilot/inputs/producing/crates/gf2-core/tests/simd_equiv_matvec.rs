//! SIMD-routed `BitMatrix::matvec` equivalence tests.

mod simd_equiv;

use gf2_core::{BitMatrix, BitVec};
use proptest::prelude::any;
use proptest::strategy::{Just, Strategy};

use simd_equiv::{assert_simd_matches_scalar, WORD_BOUNDARY_LENGTHS};

#[derive(Clone, Debug, PartialEq)]
struct MatvecFixture {
    rows: usize,
    cols: usize,
    matrix_bits: Vec<bool>,
    vector_bits: Vec<bool>,
}

fn fixture_to_inputs(fixture: &MatvecFixture) -> (BitMatrix, BitVec) {
    let mut matrix = BitMatrix::zeros(fixture.rows, fixture.cols);
    for (i, &bit) in fixture.matrix_bits.iter().enumerate() {
        if bit {
            matrix.set(i / fixture.cols, i % fixture.cols, true);
        }
    }

    let mut vector = BitVec::with_capacity(fixture.cols);
    for &bit in &fixture.vector_bits {
        vector.push_bit(bit);
    }

    (matrix, vector)
}

fn scalar_matvec_reference(matrix: &BitMatrix, vector: &BitVec) -> BitVec {
    assert_eq!(
        vector.len(),
        matrix.cols(),
        "input BitVec length must equal cols"
    );

    let mut out = BitVec::with_capacity(matrix.rows());
    for row in 0..matrix.rows() {
        let parity = (0..matrix.cols()).fold(false, |acc, col| {
            acc ^ (matrix.get(row, col) & vector.get(col))
        });
        out.push_bit(parity);
    }
    out
}

fn matvec_strategy() -> impl Strategy<Value = MatvecFixture> {
    (0usize..=32, 0usize..=1024).prop_flat_map(|(rows, cols)| {
        let matrix_len = rows * cols;
        (
            Just(rows),
            Just(cols),
            proptest::collection::vec(any::<bool>(), matrix_len..=matrix_len),
            proptest::collection::vec(any::<bool>(), cols..=cols),
        )
            .prop_map(|(rows, cols, matrix_bits, vector_bits)| MatvecFixture {
                rows,
                cols,
                matrix_bits,
                vector_bits,
            })
    })
}

#[test]
fn matvec_word_boundary_lengths_match_scalar_reference() {
    for &n in WORD_BOUNDARY_LENGTHS
        .iter()
        .filter(|&&n| matches!(n, 0 | 1 | 63 | 64 | 65 | 127 | 128 | 129))
    {
        let mut matrix = BitMatrix::zeros(n, n);
        for row in 0..n {
            for col in 0..n {
                matrix.set(row, col, ((row * 17 + col * 31 + n) & 3) == 1);
            }
        }

        let mut vector = BitVec::with_capacity(n);
        for i in 0..n {
            vector.push_bit(((i * 13 + n) & 1) == 0);
        }

        assert_eq!(
            matrix.matvec(&vector),
            scalar_matvec_reference(&matrix, &vector),
            "matvec diverged from scalar reference at n={n}"
        );
    }
}

#[test]
fn matvec_matches_scalar_reference_proptest_sizes_0_to_1024() {
    assert_simd_matches_scalar::<MatvecFixture, BitVec, _, _, _>(
        |fixture| {
            let (matrix, vector) = fixture_to_inputs(fixture);
            scalar_matvec_reference(&matrix, &vector)
        },
        |fixture| {
            let (matrix, vector) = fixture_to_inputs(fixture);
            matrix.matvec(&vector)
        },
        matvec_strategy(),
    );
}
