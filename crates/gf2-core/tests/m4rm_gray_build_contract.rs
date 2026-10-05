//! The Gray-table argument contract, run over every builder in the
//! workspace: the kernel crate's portable reference and its specialized
//! builders ([`kernel_builders`]), and the matrix-level
//! [`build_gray_table_flat`] at one stride per route it takes. The reference
//! is always present, so the list is never empty.

use gf2_core::alg::m4rm::build_gray_table_flat;
use gf2_core::kernels::ops::resolve_xor_inplace;
use gf2_core::matrix::BitMatrix;
use gf2_kernels_simd::m4rm::contract::{assert_contract, kernel_builders, Builder};
use std::panic::catch_unwind;

/// [`build_gray_table_flat`] in the call shape of the contract: the panel
/// becomes the rows of a `valid_rows × 64 * stride_words` matrix and
/// `table_size`, a power of two, the block of `log2(table_size)` rows from
/// row 0. Rows the matrix lacks are absent, as `valid_rows` makes them.
fn matrix_level(
    buffer: &mut [u64],
    panel: &[u64],
    stride_words: usize,
    table_size: usize,
    valid_rows: usize,
) {
    assert!(table_size.is_power_of_two(), "adapter: power-of-two tables");
    let columns = 64 * stride_words;
    let mut matrix = BitMatrix::zeros(valid_rows, columns);
    for row in 0..valid_rows {
        for column in 0..columns {
            let word = panel[row * stride_words + column / 64];
            matrix.set(row, column, word >> (column % 64) & 1 == 1);
        }
    }
    let k_block = table_size.trailing_zeros() as usize;
    let xor = resolve_xor_inplace(stride_words);
    build_gray_table_flat(&matrix, 0, k_block, columns, buffer, xor);
}

/// One stride per route of [`build_gray_table_flat`]: the two specialized
/// SIMD builders (4, 8), the two-tile walk (16), whole tiles (24, 32), a
/// partial last tile (1, 5, 20) and the walk beyond four tiles (40).
const MATRIX_LEVEL_STRIDES: [usize; 9] = [1, 4, 5, 8, 16, 20, 24, 32, 40];

#[test]
fn every_gray_table_builder_answers_the_contract() {
    let mut builders = kernel_builders();
    builders.extend(MATRIX_LEVEL_STRIDES.map(|stride_words| Builder {
        label: format!("build_gray_table_flat, stride {stride_words}"),
        panic_prefix: "build_gray_table_flat",
        stride_words,
        takes_sizes: false,
        build: &matrix_level,
    }));
    assert_contract(&builders);
}

#[test]
fn a_block_of_word_size_rows_is_rejected() {
    let matrix = BitMatrix::zeros(1, 64);
    let outcome = catch_unwind(|| {
        let mut buffer = vec![0u64; 2];
        build_gray_table_flat(
            &matrix,
            0,
            usize::BITS as usize,
            64,
            &mut buffer,
            resolve_xor_inplace(1),
        );
    });
    let payload = outcome.expect_err("a table of 2^64 entries was accepted");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert!(
        message.starts_with("build_gray_table_flat") && message.contains("overflows usize"),
        "panicked with {message:?}"
    );
}
