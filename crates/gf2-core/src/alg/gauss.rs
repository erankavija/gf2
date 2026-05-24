//! Matrix inversion over GF(2).
//!
//! This module implements matrix inversion using Gauss–Jordan elimination on
//! the augmented matrix `[A | I]`. Two paths are exposed:
//!
//! - The public [`invert`] selects the M4RM Gray-table path
//!   ([`invert_m4ri`]) for matrices at or above [`INVERT_M4RI_THRESHOLD`]
//!   and the scalar Gauss–Jordan path ([`invert_scalar`]) below it where
//!   table setup is the dominant cost.
//! - [`invert_m4ri`] runs Gauss–Jordan in `k`-column blocks. For each block
//!   it finds `k` pivots in the block, then builds a 2ᵏ-entry Gray-code
//!   table over the corresponding pivot-row suffixes and uses the table to
//!   eliminate the block bits from every other row (both above and below
//!   the pivot stripe — this is the Gauss–Jordan twist relative to the
//!   `rref::eliminate_block` machinery in `crate::alg::rref`).
//!
//! Both paths are bit-exact: they produce the unique inverse of a
//! non-singular GF(2) matrix and return `None` on a singular input.

use crate::kernels::ops::{resolve_xor_inplace, xor_inplace, XorInplaceFn};
use crate::matrix::BitMatrix;

/// Smallest `n` at which the M4RM Gray-table invert path is used by
/// [`invert`]. Below this size the constant overhead of allocating and
/// populating the Gray table is larger than the row traffic that the
/// blocked schedule saves, so the scalar path wins.
pub const INVERT_M4RI_THRESHOLD: usize = 8;

/// Inverts a square matrix over GF(2).
///
/// Dispatches to the M4RM Gray-table path ([`invert_m4ri`]) when the matrix
/// is large enough to amortise the table setup, and to the scalar
/// Gauss–Jordan path ([`invert_scalar`]) otherwise. Both paths produce the
/// same bit-exact result; the dispatch threshold is a perf knob, not a
/// correctness boundary. See [`INVERT_M4RI_THRESHOLD`].
///
/// Returns `None` if the matrix is non-square or singular.
///
/// # Arguments
///
/// * `m` - Square matrix to invert.
///
/// # Returns
///
/// * `Some(inverse)` — the inverse matrix when one exists.
/// * `None` — when the matrix is non-square or singular.
///
/// # Examples
///
/// ```
/// use gf2_core::matrix::BitMatrix;
/// use gf2_core::alg::gauss::invert;
///
/// let id = BitMatrix::identity(3);
/// let inv = invert(&id).unwrap();
/// // inv equals id for the identity matrix
/// for i in 0..3 {
///     for j in 0..3 {
///         assert_eq!(inv.get(i, j), i == j);
///     }
/// }
/// ```
pub fn invert(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    if n != m.cols() {
        return None;
    }
    if n < INVERT_M4RI_THRESHOLD {
        return invert_scalar(m);
    }
    invert_m4ri(m)
}

/// Inverts a square matrix using textbook Gauss–Jordan over the augmented
/// matrix `[A | I]`. Issues one row-XOR per non-pivot row per column.
///
/// This is the V0 scalar path — kept as the correctness oracle for the
/// blocked M4RM path and used for very small matrices where its lower
/// constant factor wins.
///
/// Returns `None` if the matrix is non-square or singular.
pub fn invert_scalar(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    if n != m.cols() {
        return None;
    }
    if n == 0 {
        return Some(BitMatrix::zeros(0, 0));
    }

    // Build [A | I].
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

    // Extract right half.
    let mut inv = BitMatrix::zeros(n, n);
    for r in 0..n {
        for c in 0..n {
            inv.set(r, c, aug.get(r, n + c));
        }
    }
    Some(inv)
}

/// Inverts a square matrix using an M4RM Gray-table-augmented Gauss–Jordan
/// elimination on `[A | I]`.
///
/// For each column block of width `k`, the routine finds `k` pivots inside
/// the block, builds a 2ᵏ-entry Gray-code table of XORs of the pivot rows
/// (over the trailing word suffix), and applies the table to every other
/// row in a single suffix-XOR per row. The asymptotic word-op count drops
/// from O(n³ / 64) to O(n³ / (64 · k)), with `k = O(log₂ n)`.
///
/// Returns `None` if the matrix is non-square or singular.
///
/// # Algorithm reference
///
/// The block-elimination kernel mirrors the M4RI-style block schedule
/// already used by [`crate::alg::rref::rref`]; the only structural difference
/// is that invert eliminates rows **above** the pivot stripe in addition to
/// rows below, which is what turns the row-echelon form into the inverse on
/// the right half of `[A | I]`.
pub fn invert_m4ri(m: &BitMatrix) -> Option<BitMatrix> {
    let n = m.rows();
    if n != m.cols() {
        return None;
    }
    if n == 0 {
        return Some(BitMatrix::zeros(0, 0));
    }

    // Augmented matrix [A | I] with stride sized to fit 2n columns.
    let aug_cols = 2 * n;
    let mut aug = BitMatrix::zeros(n, aug_cols);
    for r in 0..n {
        // Copy A into the left half by word, then set the diagonal bit on
        // the right half.
        let src_words = m.row_words(r);
        let stride_in = m.stride_words();
        let stride_out = aug.stride_words();
        let dst = aug.row_words_mut(r);
        dst[..stride_in].copy_from_slice(src_words);

        // Right-half identity bit at column (n + r).
        let bit = n + r;
        let word = bit / 64;
        let mask = 1u64 << (bit & 63);
        debug_assert!(word < stride_out);
        dst[word] |= mask;
    }

    let stride_words = aug.stride_words();
    let xor = resolve_xor_inplace(stride_words);

    let block_size = default_block_size_invert(n);
    debug_assert!((1..=10).contains(&block_size));

    let mut col = 0usize;
    let mut current_row = 0usize;
    while current_row < n {
        // Establish up to `block_size` pivots starting at (current_row, col).
        let block_row_start = current_row;
        let first_block_word = col / 64;
        let mut block_pivots: Vec<usize> = Vec::with_capacity(block_size);

        while current_row < n && col < n && block_pivots.len() < block_size {
            // Reduce the candidate row by the pivots already collected in this
            // block so its column-`col` bit reflects post-elimination state.
            if let Some(pivot_row) =
                find_block_pivot_invert(&mut aug, block_row_start, current_row, col, &block_pivots)
            {
                if pivot_row != current_row {
                    aug.swap_rows(current_row, pivot_row);
                }
                // The pivots collected so far in this block have their pivot
                // bits cleared in rows below `current_row` (by
                // find_block_pivot_invert). The row we just promoted still has
                // those pivot bits cleared *below* the pivot row, but the
                // previously promoted pivot rows above may still carry bits
                // for *this* new pivot column. We need to clear them so the
                // block stays triangular within its k×k pivot square.
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
            } else {
                // No pivot in this column → singular (square matrix has no
                // free columns to skip when computing a full inverse).
                return None;
            }
        }

        if block_pivots.is_empty() {
            break;
        }

        // Eliminate the block from every row that is **not** part of the
        // pivot stripe — both above (Gauss–Jordan) and below.
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
        // Should not be reachable: the inner loop returns None on the first
        // singular column. Guard against silent miscounts.
        return None;
    }

    // Extract the right half (now A⁻¹) into a fresh n×n BitMatrix.
    let mut inv = BitMatrix::zeros(n, n);
    let inv_stride = inv.stride_words();
    let right_half_first_word = n / 64;
    let right_half_bit_offset = n & 63;

    for r in 0..n {
        let src = aug.row_words(r);
        let dst = inv.row_words_mut(r);

        if right_half_bit_offset == 0 {
            // Aligned case: the right half starts on a word boundary, copy
            // `inv_stride` words directly.
            dst.copy_from_slice(&src[right_half_first_word..right_half_first_word + inv_stride]);
        } else {
            // Unaligned case: shift the source words right by
            // `right_half_bit_offset` bits, joining adjacent words.
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
    // Tail-mask the result rows: the augmented matrix has 2n columns padded
    // to a word boundary, so any padding beyond `n` in the extracted suffix
    // could leak set bits past `cols()` if the unaligned shift pulled in
    // identity bits from beyond the right-half's `n`th column.
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

/// Default M4RM block width for `invert` over GF(2).
///
/// The threshold table mirrors `crate::alg::rref::default_block_size` and
/// the M4RI library's own `m4ri_optk(n)` rule of thumb (M4RI clamps `k` to
/// roughly `log₂(n)` then floors at small `n`). For the SOTA target sizes:
///
/// | n     | k_block |
/// |-------|---------|
/// | ≤ 64  | 4       |
/// | 65–512| 4       |
/// | > 512 | 8       |
///
/// The right column gives a 16-entry table at small n and a 256-entry
/// table at large n. The 256-entry case is the table size M4RI uses at
/// n=1024 in its `mzd_invert_m4ri` default schedule.
fn default_block_size_invert(n: usize) -> usize {
    match n {
        0..=64 => 4,
        65..=512 => 4,
        _ => 8,
    }
}

/// Find a pivot row at or below `start_row` for column `col`, reducing
/// scanned rows by the pivots collected so far in the block so that
/// their column-`col` bit reflects the post-elimination state.
///
/// This mirrors `crate::alg::rref::find_block_pivot` but uses the public
/// `BitMatrix::get` to dodge the `pub(crate)` access boundary on the unsafe
/// helpers.
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
/// using a Gray-code table of pivot-row XOR combinations restricted to the
/// trailing word suffix.
///
/// This is the M4RI "method-of-the-four-Russians for elimination" step
/// generalised to Gauss–Jordan (i.e., the table is applied to rows above
/// the pivot stripe as well as below, not only below as in `rref::eliminate_block`).
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

    // table[g] = XOR of pivot rows selected by the bits of g, restricted to
    // the trailing suffix (first_word..stride_words). Gray-code walk so each
    // step requires one XOR of a single pivot row.
    let table_rows = 1usize << block_rows;
    let mut table = vec![0u64; table_rows * suffix_words];
    for idx in 1..table_rows {
        let bit = idx.trailing_zeros() as usize;
        let prev = idx & !(1usize << bit);

        let (before, after) = table.split_at_mut(idx * suffix_words);
        let prev_slice = &before[prev * suffix_words..prev * suffix_words + suffix_words];
        let dst = &mut after[..suffix_words];
        dst.copy_from_slice(prev_slice);
        xor(
            dst,
            &aug.row_words(block_row_start + bit)[first_word..first_word + suffix_words],
        );
    }

    let stripe_end = block_row_start + block_rows;
    for row in 0..rows {
        if (block_row_start..stripe_end).contains(&row) {
            continue;
        }
        let table_idx = block_table_index_invert(aug, row, block_pivots);
        if table_idx != 0 {
            let src_start = table_idx * suffix_words;
            aug.row_xor_slice_from(row, first_word, &table[src_start..src_start + suffix_words]);
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
        // Use the production `invert_scalar` as the oracle on random
        // invertible inputs. We try a handful of seeds per n and skip
        // singulars, so the test always exercises the invert path.
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
        // Zero matrix
        for &n in &[1usize, 8, 64, 128] {
            let z = BitMatrix::zeros(n, n);
            assert!(invert_m4ri(&z).is_none(), "zero n={n}");
            assert!(invert_scalar(&z).is_none(), "scalar zero n={n}");
        }
        // Duplicate-row singular at n = 8
        let mut m = BitMatrix::identity(8);
        // Force row 5 = row 4.
        let row4 = m.row_words(4).to_vec();
        m.row_words_mut(5).copy_from_slice(&row4);
        assert!(invert_m4ri(&m).is_none(), "duplicate-row n=8");
        assert!(invert_scalar(&m).is_none(), "scalar duplicate-row n=8");
    }

    #[test]
    fn test_invert_dispatch_matches_explicit_paths() {
        // Below threshold → scalar path.
        let small = BitMatrix::identity(3);
        let dispatch = invert(&small).unwrap();
        let scalar = invert_scalar(&small).unwrap();
        assert_eq!(dispatch, scalar, "dispatch < threshold should match scalar");

        // At/above threshold → M4RM path.
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
