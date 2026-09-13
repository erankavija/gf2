//! The matrix-level transpose contract, run over every lane of the family.
//!
//! `gf2_kernels_simd::transpose::contract` holds the block-level cases and
//! this file holds the ones only a whole matrix has: a zero-sized matrix, a
//! non-square matrix, a matrix whose rows are not a whole number of words,
//! and the partial boundary tiles at 63, 64 and 65 rows and columns. Each
//! case runs through [`BitMatrix::transpose_with_block_kernel`], so every
//! lane drives the production tiling, output allocation, zero padding and
//! tail mask rather than a second copy of them.
//!
//! Lanes whose processor feature this host lacks are skipped by
//! `gf2_kernels_simd::transpose::lane` returning `None`; the scalar lane is
//! always present, so the list is never empty.

use gf2_core::matrix::BitMatrix;
use gf2_kernels_simd::transpose::{lane, Transpose64x64Fn, TransposeLane};

/// SplitMix64 [Steele2014], so a failing case is reproducible from its seed.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A pseudorandom `rows × cols` matrix, one draw per matrix element in
/// row-major order.
fn random_matrix(rows: usize, cols: usize, seed: u64) -> BitMatrix {
    let mut state = seed;
    let mut matrix = BitMatrix::zeros(rows, cols);
    for row in 0..rows {
        for col in 0..cols {
            if splitmix64(&mut state) & 1 == 1 {
                matrix.set(row, col, true);
            }
        }
    }
    matrix
}

/// Asserts $\mathrm{output}[c][r] = \mathrm{input}[r][c]$ element by element,
/// plus the shape and the involution.
fn assert_transpose_of(label: &str, input: &BitMatrix, output: &BitMatrix) {
    assert_eq!(
        (output.rows(), output.cols()),
        (input.cols(), input.rows()),
        "{label}: transposed shape"
    );
    for row in 0..input.rows() {
        for col in 0..input.cols() {
            assert_eq!(
                output.get(col, row),
                input.get(row, col),
                "{label}: entry ({row}, {col})"
            );
        }
    }
}

/// Every shape the matrix-level contract covers, with the seed that fills it.
///
/// 0 covers the zero-sized matrix in both dimensions; 1 and 65 make the row
/// and column tail a partial word; 63, 64 and 65 bracket the block boundary;
/// the rectangular pairs make the tile grid non-square in both directions,
/// and 130×3 gives three row blocks against one narrow column block.
const SHAPES: &[(usize, usize)] = &[
    (0, 0),
    (0, 7),
    (7, 0),
    (1, 1),
    (1, 64),
    (64, 1),
    (63, 63),
    (63, 64),
    (63, 65),
    (64, 63),
    (64, 64),
    (64, 65),
    (65, 63),
    (65, 64),
    (65, 65),
    (3, 130),
    (130, 3),
    (129, 257),
];

fn assert_matrix_contract(label: &str, kernel: Transpose64x64Fn) {
    for (index, &(rows, cols)) in SHAPES.iter().enumerate() {
        let case = format!("{label}: {rows}x{cols}");
        let input = random_matrix(rows, cols, 0x1d4f_d63d_0000_0000 ^ index as u64);

        let output = input.transpose_with_block_kernel(kernel);
        assert_transpose_of(&case, &input, &output);

        // The production entry point resolves its own lane; both must agree
        // element for element whichever lane each runs.
        assert_eq!(output, input.transpose(), "{case}: agrees with transpose()");

        // Involution, which also proves the padding bits of the intermediate
        // are clear: a stray bit beyond `cols` would come back as a set bit
        // beyond `rows`.
        let back = output.transpose_with_block_kernel(kernel);
        assert_eq!(back, input, "{case}: involution");
    }
}

/// The lanes this host can run, with the tag a failure reports.
fn available_lanes() -> Vec<(&'static str, Transpose64x64Fn)> {
    TransposeLane::ALL
        .into_iter()
        .filter_map(|candidate| lane(candidate).map(|kernel| (candidate.name(), kernel)))
        .collect()
}

#[test]
fn every_lane_answers_the_matrix_transpose_contract() {
    let lanes = available_lanes();
    assert!(
        lanes
            .iter()
            .any(|(name, _)| *name == TransposeLane::Scalar.name()),
        "the scalar lane is available on every host"
    );
    for (name, kernel) in lanes {
        assert_matrix_contract(name, kernel);
    }
}

#[test]
fn every_lane_answers_the_block_contract() {
    for (name, kernel) in available_lanes() {
        gf2_kernels_simd::transpose::contract::assert_block_contract(name, kernel);
    }
}

#[test]
fn a_zero_sized_matrix_transposes_to_its_transposed_shape() {
    let scalar = lane(TransposeLane::Scalar).expect("always available");
    for &(rows, cols) in &[(0usize, 0usize), (0, 9), (9, 0)] {
        let empty = BitMatrix::zeros(rows, cols);
        let output = empty.transpose_with_block_kernel(scalar);
        assert_eq!((output.rows(), output.cols()), (cols, rows));
    }
}
