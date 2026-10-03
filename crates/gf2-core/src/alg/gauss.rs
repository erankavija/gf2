//! Matrix inversion over GF(2) by Gauss–Jordan elimination on the augmented
//! matrix `[A | I]`: [`invert`] selects between a scalar path
//! ([`invert_scalar`]) and an M4RM Gray-table path ([`invert_m4ri`]), and
//! [`invert_route`] reports the selected arm.

use crate::alg::m4rm::build_gray_table_flat;
use crate::alg::rref::default_block_size;
use crate::kernels::ops::{resolve_xor_inplace, xor_inplace, XorInplaceFn};
use crate::matrix::BitMatrix;
use crate::tuning;

/// Conservative default for `dense_inverse.m4ri_min_dim()`, consumed by
/// [`crate::tuning::CoreTuning::CONSERVATIVE`]: [`invert`] takes
/// [`invert_scalar`] below the active profile value and [`invert_m4ri`] at
/// or above it.
pub const INVERT_M4RI_THRESHOLD: usize = 8;

/// The selected arm of the [`invert`] dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvertRoute {
    /// Use the scalar Gauss–Jordan path ([`invert_scalar`]).
    Scalar,
    /// Use the M4RM Gray-table path ([`invert_m4ri`]).
    M4ri,
}

/// Reports the [`invert`] arm for a matrix of dimension `n`.
///
/// The comparison uses the active `dense_inverse.m4ri_min_dim()` profile
/// value: matrices with `n` strictly below it take the scalar path, at or
/// above it the M4RM Gray-table path.
#[must_use]
pub fn invert_route(n: usize) -> InvertRoute {
    invert_route_resolved(tuning::active().dense_inverse().m4ri_min_dim(), n)
}

/// Reports the [`invert`] arm for a matrix of dimension `n` against an
/// already-resolved `m4ri_min_dim`.
fn invert_route_resolved(m4ri_min_dim: usize, n: usize) -> InvertRoute {
    if n < m4ri_min_dim {
        InvertRoute::Scalar
    } else {
        InvertRoute::M4ri
    }
}

/// Inverts a square matrix over GF(2).
///
/// Takes the arm [`invert_route`] reports. Returns `None` if the matrix is
/// non-square or singular.
pub fn invert(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    if n != m.cols() {
        return None;
    }
    match invert_route(n) {
        InvertRoute::Scalar => invert_scalar(m),
        InvertRoute::M4ri => invert_m4ri(m),
    }
}

/// Inverts a square matrix by Gauss–Jordan elimination over the augmented
/// matrix `[A | I]`, one row XOR per non-pivot row per column.
///
/// Returns `None` if the matrix is non-square or singular.
///
/// # Complexity
///
/// O(n³) bit operations, with n = `m.rows()`.
pub fn invert_scalar(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    if n != m.cols() {
        return None;
    }
    if n == 0 {
        return Some(BitMatrix::zeros(0, 0));
    }

    let mut aug = BitMatrix::zeros(n, 2 * n);
    for r in 0..n {
        for c in 0..n {
            aug.set(r, c, m.get(r, c));
        }
        aug.set(r, n + r, true);
    }

    for col in 0..n {
        let pivot_row = aug.find_pivot_row(col, col)?;
        if pivot_row != col {
            aug.swap_rows(col, pivot_row);
        }

        for r in 0..n {
            if r != col && aug.get(r, col) {
                let pivot: Vec<u64> = aug.row_words(col).to_vec();
                let row_r = aug.row_words_mut(r);
                xor_inplace(row_r, &pivot);
            }
        }
    }

    let mut inv = BitMatrix::zeros(n, n);
    for r in 0..n {
        for c in 0..n {
            inv.set(r, c, aug.get(r, n + c));
        }
    }
    Some(inv)
}

/// Inverts a square matrix by Gauss–Jordan elimination on `[A | I]` in
/// column blocks of width `k`: each block's pivot rows feed a 2ᵏ-entry
/// Gray-code table ([`crate::alg::m4rm::build_gray_table_flat`]), and every
/// row outside the pivot stripe clears the block with one suffix XOR.
///
/// Returns `None` if the matrix is non-square or singular.
///
/// # Complexity
///
/// O(n³ / (64 · k)) word operations, with block width `k ∈ {4, 8}`.
pub fn invert_m4ri(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    if n != m.cols() {
        return None;
    }
    if n == 0 {
        return Some(BitMatrix::zeros(0, 0));
    }

    let aug_cols = 2 * n;
    let mut aug = BitMatrix::zeros(n, aug_cols);
    for r in 0..n {
        let src_words = m.row_words(r);
        let stride_in = m.stride_words();
        let stride_out = aug.stride_words();
        let dst = aug.row_words_mut(r);
        dst[..stride_in].copy_from_slice(src_words);
        let bit = n + r;
        let word = bit / 64;
        let mask = 1u64 << (bit & 63);
        debug_assert!(word < stride_out);
        dst[word] |= mask;
    }

    let stride_words = aug.stride_words();
    let xor = resolve_xor_inplace(stride_words);

    let block_size = default_block_size(n);
    debug_assert!((1..=10).contains(&block_size));

    let mut col = 0usize;
    let mut current_row = 0usize;
    while current_row < n {
        let block_row_start = current_row;
        let first_block_word = col / 64;
        let mut block_pivots: Vec<usize> = Vec::with_capacity(block_size);

        while current_row < n && col < n && block_pivots.len() < block_size {
            // No pivot in this column means the matrix is singular, so `?`
            // returns `None`.
            let pivot_row = find_block_pivot_invert(
                &mut aug,
                block_row_start,
                current_row,
                col,
                &block_pivots,
            )?;

            if pivot_row != current_row {
                aug.swap_rows(current_row, pivot_row);
            }
            // Earlier pivot rows of this block may still carry a bit in the
            // new pivot column; clear it so the block's pivot square is the
            // identity.
            for (offset, &prev_pivot_col) in block_pivots.iter().enumerate() {
                let prev_row = block_row_start + offset;
                debug_assert!(
                    !aug.get(current_row, prev_pivot_col),
                    "block pivot reduction left an earlier pivot bit set"
                );
                if aug.get(prev_row, col) {
                    aug.row_xor_from(prev_row, current_row, col / 64);
                }
            }

            block_pivots.push(col);
            current_row += 1;
            col += 1;
        }

        if block_pivots.is_empty() {
            break;
        }

        eliminate_block_full(
            &mut aug,
            n,
            block_row_start,
            &block_pivots,
            first_block_word,
            stride_words,
            xor,
        );
    }

    if current_row < n {
        // Unreachable: the inner loop returns `None` on the first singular
        // column.
        return None;
    }

    let mut inv = BitMatrix::zeros(n, n);
    let inv_stride = inv.stride_words();
    let right_half_first_word = n / 64;
    let right_half_bit_offset = n & 63;

    for r in 0..n {
        let src = aug.row_words(r);
        let dst = inv.row_words_mut(r);

        if right_half_bit_offset == 0 {
            dst.copy_from_slice(&src[right_half_first_word..right_half_first_word + inv_stride]);
        } else {
            let shift = right_half_bit_offset;
            let inv_shift = 64 - shift;
            for w in 0..inv_stride {
                let lo = src[right_half_first_word + w] >> shift;
                let hi_word_idx = right_half_first_word + w + 1;
                let hi = if hi_word_idx < src.len() {
                    src[hi_word_idx] << inv_shift
                } else {
                    0
                };
                dst[w] = lo | hi;
            }
        }
    }
    let tail_bits = inv.cols() % 64;
    if tail_bits != 0 {
        let mask = (1u64 << tail_bits) - 1;
        let last_word = inv.stride_words() - 1;
        for r in 0..n {
            let words = inv.row_words_mut(r);
            words[last_word] &= mask;
        }
    }

    Some(inv)
}

/// Find a pivot row at or below `start_row` for column `col`, reducing
/// scanned rows by the pivots collected so far in the block so that
/// their column-`col` bit reflects the post-elimination state.
fn find_block_pivot_invert(
    aug: &mut BitMatrix,
    block_row_start: usize,
    start_row: usize,
    col: usize,
    block_pivots: &[usize],
) -> Option<usize> {
    for row in start_row..aug.rows() {
        for (offset, &pivot_col) in block_pivots.iter().enumerate() {
            if aug.get(row, pivot_col) {
                aug.row_xor_from(row, block_row_start + offset, pivot_col / 64);
            }
        }
        if aug.get(row, col) {
            return Some(row);
        }
    }
    None
}

/// Eliminate a `k`-pivot block from every row outside the pivot stripe
/// using a Gray-code table of pivot-row XOR combinations.
#[allow(clippy::too_many_arguments)]
fn eliminate_block_full(
    aug: &mut BitMatrix,
    rows: usize,
    block_row_start: usize,
    block_pivots: &[usize],
    first_word: usize,
    stride_words: usize,
    xor: XorInplaceFn,
) {
    let block_rows = block_pivots.len();
    debug_assert!(block_rows > 0);
    let suffix_words = stride_words - first_word;
    if suffix_words == 0 {
        return;
    }

    // table[g] = XOR of the pivot rows selected by the bits of g, over the
    // full row width; the lookup slices each entry from `first_word` on.
    let table_rows = 1usize << block_rows;
    let aug_cols = aug.cols();
    let mut table = vec![0u64; table_rows * stride_words];
    build_gray_table_flat(aug, block_row_start, block_rows, aug_cols, &mut table, xor);

    let stripe_end = block_row_start + block_rows;
    for row in 0..rows {
        if (block_row_start..stripe_end).contains(&row) {
            continue;
        }
        let table_idx = block_table_index_invert(aug, row, block_pivots);
        if table_idx != 0 {
            let entry_start = table_idx * stride_words + first_word;
            aug.row_xor_slice_from(
                row,
                first_word,
                &table[entry_start..entry_start + suffix_words],
            );
        }
    }
}

/// Extract the k-bit index into the Gray table for a single row.
///
/// When the pivots are contiguous within a single word, a single mask-and-shift
/// reads the index; otherwise the index is rebuilt bit-by-bit.
fn block_table_index_invert(aug: &BitMatrix, row: usize, block_pivots: &[usize]) -> usize {
    if let (Some(&first), Some(&last)) = (block_pivots.first(), block_pivots.last()) {
        let width = block_pivots.len();
        if last + 1 == first + width && first / 64 == last / 64 {
            let mask = (1usize << width) - 1;
            return ((aug.row_words(row)[first / 64] >> (first & 63)) as usize) & mask;
        }
    }

    let mut table_idx = 0usize;
    for (bit, &pivot_col) in block_pivots.iter().enumerate() {
        if aug.get(row, pivot_col) {
            table_idx |= 1usize << bit;
        }
    }
    table_idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alg::m4rm::multiply;

    fn random_matrix(n: usize, seed: u64) -> BitMatrix {
        BitMatrix::random_seeded(n, n, seed)
    }

    #[test]
    fn test_invert_m4ri_matches_scalar_identity() {
        for n in [0usize, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129] {
            let id = BitMatrix::identity(n);
            let m4ri = invert_m4ri(&id).expect("identity should invert");
            let scalar = invert_scalar(&id).expect("identity should invert");
            assert_eq!(m4ri, scalar, "identity n={n} mismatch");
            assert_eq!(m4ri, id, "inverse of identity is identity");
        }
    }

    #[test]
    fn test_invert_m4ri_matches_scalar_random() {
        for &n in &[1usize, 7, 8, 9, 63, 64, 65, 127, 128, 129, 200, 256] {
            let mut matched = false;
            for seed in 0..16u64 {
                let m = random_matrix(n, 0x00a8_47cf_0000 ^ (seed << 8) ^ (n as u64));
                let scalar = match invert_scalar(&m) {
                    Some(inv) => inv,
                    None => continue,
                };
                let m4ri = invert_m4ri(&m).expect("matched scalar should also invert");
                assert_eq!(m4ri, scalar, "random n={n} seed={seed} mismatch");
                matched = true;
                break;
            }
            assert!(
                matched,
                "no invertible matrix found at n={n} across 16 seeds"
            );
        }
    }

    #[test]
    fn test_invert_m4ri_round_trips_via_multiply() {
        for &n in &[1usize, 8, 9, 64, 65, 128, 129] {
            for seed in 0..16u64 {
                let m = random_matrix(n, 0x9981 ^ (seed << 11) ^ (n as u64 * 17));
                if let Some(inv) = invert_m4ri(&m) {
                    let product = multiply(&m, &inv);
                    let id = BitMatrix::identity(n);
                    assert_eq!(product, id, "m × m^-1 ≠ I at n={n} seed={seed}");
                    break;
                }
            }
        }
    }

    #[test]
    fn test_invert_m4ri_singular_returns_none() {
        for &n in &[1usize, 8, 64, 128] {
            let z = BitMatrix::zeros(n, n);
            assert!(invert_m4ri(&z).is_none(), "zero n={n}");
            assert!(invert_scalar(&z).is_none(), "scalar zero n={n}");
        }
        let mut m = BitMatrix::identity(8);
        let row4 = m.row_words(4).to_vec();
        m.row_words_mut(5).copy_from_slice(&row4);
        assert!(invert_m4ri(&m).is_none(), "duplicate-row n=8");
        assert!(invert_scalar(&m).is_none(), "scalar duplicate-row n=8");
    }

    #[test]
    fn test_invert_dispatch_matches_explicit_paths() {
        let small = BitMatrix::identity(3);
        let dispatch = invert(&small).unwrap();
        let scalar = invert_scalar(&small).unwrap();
        assert_eq!(dispatch, scalar, "dispatch < threshold should match scalar");
        for &n in &[INVERT_M4RI_THRESHOLD, 64, 129] {
            let id = BitMatrix::identity(n);
            let dispatch = invert(&id).unwrap();
            let m4ri = invert_m4ri(&id).unwrap();
            assert_eq!(
                dispatch, m4ri,
                "dispatch ≥ threshold should match m4ri (n={n})"
            );
        }
    }

    #[test]
    fn test_invert_non_square_returns_none() {
        let m = BitMatrix::zeros(3, 4);
        assert!(invert(&m).is_none());
        assert!(invert_m4ri(&m).is_none());
        assert!(invert_scalar(&m).is_none());
    }
}
