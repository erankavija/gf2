//! SIMD-routed `BitMatrix::matvec` equivalence tests.

mod simd_equiv;

use gf2_core::matrix::{matvec_route, MatvecRoute};
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

/// Column counts at the word boundaries of the scalar lane and around the
/// eight- and nine-word strides of the SIMD lane; a nine-word stride starts
/// successive rows at every 32-byte phase.
const BOUNDARY_COLS: [usize; 12] = [0, 1, 63, 64, 65, 449, 511, 512, 513, 575, 576, 577];

fn assert_zero_tail(vector: &BitVec, context: &str) {
    let words = vector.words();
    assert_eq!(words.len(), vector.len().div_ceil(64), "{context}");
    if !vector.len().is_multiple_of(64) {
        assert_eq!(
            words[words.len() - 1] >> (vector.len() % 64),
            0,
            "nonzero tail padding, {context}"
        );
    }
}

#[test]
fn matvec_boundary_shapes_index_canonically_and_keep_zero_tails() {
    for rows in [0, 1, 63, 64, 65] {
        for cols in BOUNDARY_COLS {
            let context = format!("rows={rows} cols={cols}");
            let mut matrix = BitMatrix::zeros(rows, cols);
            for row in 0..rows {
                for col in 0..cols {
                    matrix.set(row, col, ((row * 17 + col * 31 + cols) & 3) == 1);
                }
            }

            let mut pushed = BitVec::with_capacity(cols);
            for col in 0..cols {
                pushed.push_bit(((col * 13 + rows) & 1) == 0);
            }
            // Dirty padding and a surplus word: `from_words` owns the masking.
            let all_set = BitVec::from_words(vec![u64::MAX; cols.div_ceil(64) + 1], cols);

            for vector in [&pushed, &all_set] {
                let product = matrix.matvec(vector);
                assert_eq!(product.len(), rows, "{context}");
                assert_zero_tail(&product, &context);
                for row in 0..rows {
                    let parity = (0..cols)
                        .filter(|&col| matrix.get(row, col) && vector.get(col))
                        .count()
                        % 2
                        == 1;
                    assert_eq!(product.get(row), parity, "row {row}, {context}");
                }
            }
        }
    }
}

/// Column counts whose strides are 0, 1, 63, 64 and 65 words, with a partial
/// last word beside the 64- and 65-word strides.
const STRIDE_BOUNDARY_COLS: [usize; 7] = [0, 64, 4032, 4095, 4096, 4097, 4160];

#[test]
fn each_route_matches_the_reference_at_stride_and_word_boundaries() {
    for rows in [0, 1, 65] {
        for cols in BOUNDARY_COLS.into_iter().chain(STRIDE_BOUNDARY_COLS) {
            let mut matrix = BitMatrix::zeros(rows, cols);
            for row in 0..rows {
                for col in 0..cols {
                    matrix.set(row, col, ((row * 17 + col * 31 + cols) & 3) == 1);
                }
            }
            let mut vector = BitVec::with_capacity(cols);
            for col in 0..cols {
                vector.push_bit(((col * 13 + rows) & 1) == 0);
            }
            let reference = scalar_matvec_reference(&matrix, &vector);

            for route in [MatvecRoute::Scalar, MatvecRoute::Simd] {
                let context = format!("rows={rows} cols={cols} route={route:?}");
                let product = matrix.matvec_with_route(&vector, route);
                assert_eq!(product, reference, "{context}");
                assert_zero_tail(&product, &context);
            }
            assert_eq!(
                matrix.matvec(&vector),
                matrix.matvec_with_route(&vector, matvec_route(cols.div_ceil(64))),
                "rows={rows} cols={cols}"
            );
        }
    }
}

#[test]
#[should_panic(expected = "input BitVec length must equal cols")]
fn a_pinned_route_rejects_a_vector_of_another_length() {
    let matrix = BitMatrix::zeros(2, 65);
    let _ = matrix.matvec_with_route(&BitVec::zeros(64), MatvecRoute::Scalar);
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
