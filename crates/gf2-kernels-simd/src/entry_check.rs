//! Release-mode argument checks of the safe kernel entry points whose kernels
//! index without one, in the form of [`crate::m4rm::assert_gray_build_lengths`]:
//! a panic under the kernel's name, with checked products.

/// Panics unless every `a_cols[h] * b_stride + n` stays inside a `b` of
/// `b_len` elements. A row without non-zeros reads nothing and passes.
#[inline]
pub(crate) fn assert_spmm_row_columns(
    kernel: &str,
    a_cols: &[usize],
    b_len: usize,
    b_stride: usize,
    n: usize,
) {
    let Some(largest) = a_cols.iter().copied().max() else {
        return;
    };
    let Some(last_start) = b_len.checked_sub(n) else {
        panic!("{kernel}: b is shorter than one output row");
    };
    // A zero stride starts every row at element 0.
    let Some(max_col) = last_start.checked_div(b_stride) else {
        return;
    };
    assert!(
        largest <= max_col,
        "{kernel}: a column index reaches outside b"
    );
}

/// Panics unless `window` is `m * win` cells, without overflow, each below
/// `p`, and `inv_table` has an entry per residue. The kernel keeps every cell
/// below `p`, so each pivot then indexes inside `inv_table`.
#[inline]
pub(crate) fn assert_ple_panel<T: Copy + Ord + Into<usize>>(
    kernel: &str,
    window: &[T],
    m: usize,
    win: usize,
    p: T,
    inv_table_len: usize,
) {
    let Some(cells) = m.checked_mul(win) else {
        panic!("{kernel}: m * win overflows usize");
    };
    assert!(
        window.len() == cells,
        "{kernel}: window is not m * win cells"
    );
    assert!(
        inv_table_len >= p.into(),
        "{kernel}: inv_table is shorter than p"
    );
    assert!(
        window
            .iter()
            .copied()
            .max()
            .is_none_or(|largest| largest < p),
        "{kernel}: a window entry is not below p"
    );
}

/// Panics unless every length equals the first and is a multiple of four.
#[inline]
pub(crate) fn assert_packed_lengths<const SLICES: usize>(kernel: &str, lengths: [usize; SLICES]) {
    let n = lengths[0];
    assert!(
        n.is_multiple_of(4),
        "{kernel}: slice length must be a multiple of four"
    );
    assert!(
        lengths.iter().all(|&length| length == n),
        "{kernel}: slice lengths differ"
    );
}

/// Panics unless `length == rows * cols`, without overflow.
#[inline]
pub(crate) fn assert_shape(kernel: &str, operand: &str, length: usize, rows: usize, cols: usize) {
    let Some(cells) = rows.checked_mul(cols) else {
        panic!("{kernel}: shape of {operand} overflows usize");
    };
    assert!(length == cells, "{kernel}: {operand} shape");
}

#[cfg(test)]
mod tests {
    use super::assert_shape;

    #[test]
    fn shape_accepts_the_exact_product() {
        assert_shape("kernel", "out", 12, 3, 4);
        assert_shape("kernel", "out", 0, usize::MAX, 0);
    }

    #[test]
    #[should_panic(expected = "kernel: shape of out overflows usize")]
    fn shape_rejects_a_product_that_wraps_to_the_length() {
        assert_shape("kernel", "out", 0, 1 << 32, 1 << 32);
    }

    #[test]
    #[should_panic(expected = "kernel: out shape")]
    fn shape_rejects_another_length() {
        assert_shape("kernel", "out", 11, 3, 4);
    }
}
