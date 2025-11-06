//! M4RM (Method of the Four Russians for Multiplication) algorithm.
//!
//! This module implements fast matrix multiplication over GF(2) using the M4RM algorithm.
//! The algorithm processes the multiplication in blocks, precomputing Gray code tables
//! of linear combinations to reduce the number of row operations.

use crate::kernels::ops::xor_inplace;
use crate::matrix::BitMatrix;

/// Chooses an appropriate block size k for M4RM based on matrix dimensions.
///
/// The block size determines the size of the Gray code table (2^k entries).
/// We aim to keep the table size reasonable for cache efficiency (target ~64 KiB).
///
/// # Arguments
///
/// * `k` - Inner dimension (A is m×k, B is k×n)
/// * `n` - Output width (number of columns in result)
///
/// # Returns
///
/// Block size k_block (typically 6-8)
fn choose_k_block(k: usize, n: usize) -> usize {
    // Each table entry is a row of n bits, stored in stride_words u64s
    let stride_words = if n == 0 { 0 } else { n.div_ceil(64) };
    let bytes_per_entry = stride_words * 8;

    // Try different block sizes and pick the largest that fits in cache
    const TARGET_TABLE_BYTES: usize = 64 * 1024; // 64 KiB target

    for k_block in (1..=8).rev() {
        let table_entries = 1usize << k_block;
        let table_bytes = table_entries * bytes_per_entry;

        if table_bytes <= TARGET_TABLE_BYTES && k_block <= k {
            return k_block;
        }
    }

    // Fallback to smallest block size
    1.min(k)
}

/// Builds a lookup table for all linear combinations of k_block consecutive rows from matrix B.
///
/// table[i] = XOR of rows indicated by the binary representation of i
/// For example, if k_block=3:
///   table[0b000] = zero vector
///   table[0b001] = row 0
///   table[0b010] = row 1
///   table[0b011] = row 0 XOR row 1
///   table[0b100] = row 2
///   ...
///
/// # Arguments
///
/// * `b` - Input matrix B
/// * `row_start` - Starting row index in B
/// * `k_block` - Number of rows to include in the table
/// * `n` - Number of columns in B
///
/// # Returns
///
/// A vector of 2^k_block entries, indexed by binary representation
fn build_gray_table(b: &BitMatrix, row_start: usize, k_block: usize, n: usize) -> Vec<Vec<u64>> {
    let table_size = 1usize << k_block;
    let stride_words = if n == 0 { 0 } else { n.div_ceil(64) };

    let mut table = vec![vec![0u64; stride_words]; table_size];

    // Build each table entry by XORing the rows indicated by the binary representation
    for (idx, entry) in table.iter_mut().enumerate().take(table_size) {
        for bit in 0..k_block {
            if (idx & (1 << bit)) != 0 {
                // Bit is set, so include this row
                if row_start + bit < b.rows() {
                    let row_words = b.row_words(row_start + bit);
                    xor_inplace(entry, row_words);
                }
            }
        }
    }

    table
}

/// Extracts k_block consecutive bits from a row of matrix A starting at column col_start.
///
/// Returns an index into the Gray code table (0..2^k_block).
fn extract_bits(a: &BitMatrix, row: usize, col_start: usize, k_block: usize) -> usize {
    let mut result = 0usize;
    let max_col = a.cols();

    for bit_idx in 0..k_block {
        let col = col_start + bit_idx;
        if col < max_col && a.get(row, col) {
            result |= 1usize << bit_idx;
        }
    }

    result
}

/// Multiplies two matrices over GF(2) using the M4RM algorithm.
///
/// Computes C = A × B where A is m×k and B is k×n, producing C which is m×n.
/// All arithmetic is performed over GF(2) (binary field).
///
/// # Arguments
///
/// * `a` - Left matrix (m × k)
/// * `b` - Right matrix (k × n)
///
/// # Returns
///
/// Result matrix C = A × B (m × n)
///
/// # Panics
///
/// Panics if the number of columns in A doesn't match the number of rows in B.
///
/// # Examples
///
/// ```
/// use gf2_core::matrix::BitMatrix;
/// use gf2_core::alg::m4rm::multiply;
///
/// let a = BitMatrix::identity(3);
/// let mut b = BitMatrix::zeros(3, 4);
/// b.set(0, 1, true);
/// b.set(1, 2, true);
///
/// let c = multiply(&a, &b);
/// assert_eq!(c.rows(), 3);
/// assert_eq!(c.cols(), 4);
/// assert_eq!(c.get(0, 1), true);
/// ```
pub fn multiply(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    let m = a.rows();
    let k = a.cols();
    let n = b.cols();

    assert_eq!(
        k,
        b.rows(),
        "incompatible dimensions: A is {}×{} but B is {}×{}",
        m,
        k,
        b.rows(),
        n
    );

    let mut c = BitMatrix::zeros(m, n);

    if m == 0 || k == 0 || n == 0 {
        return c;
    }

    let k_block = choose_k_block(k, n);

    // Process B in panels of k_block rows
    let mut panel_start = 0;
    while panel_start < k {
        let panel_size = k_block.min(k - panel_start);

        // Build Gray code table for this panel
        let table = build_gray_table(b, panel_start, panel_size, n);

        // For each row of A
        for i in 0..m {
            // Extract k_block bits from row i of A starting at panel_start
            let idx = extract_bits(a, i, panel_start, panel_size);

            // XOR the corresponding table entry into row i of C
            let c_row = c.row_words_mut(i);
            xor_inplace(c_row, &table[idx]);
        }

        panel_start += k_block;
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choose_k_block() {
        // Small dimensions should allow larger block sizes
        let k1 = choose_k_block(100, 100);
        assert!((6..=8).contains(&k1));

        // Very large output width should reduce block size
        let k2 = choose_k_block(100, 10000);
        assert!(k2 >= 1);
    }

    #[test]
    fn test_extract_bits() {
        let mut a = BitMatrix::zeros(1, 8);
        a.set(0, 1, true);
        a.set(0, 3, true);

        // Extract bits 0-3: should get binary 1010 = 10
        let bits = extract_bits(&a, 0, 0, 4);
        assert_eq!(bits, 0b1010);
    }

    #[test]
    fn test_multiply_identity() {
        let a = BitMatrix::identity(4);
        let b = BitMatrix::identity(4);
        let c = multiply(&a, &b);

        // I × I = I
        for i in 0..4 {
            for j in 0..4 {
                assert_eq!(c.get(i, j), i == j);
            }
        }
    }

    #[test]
    fn test_multiply_simple() {
        let mut a = BitMatrix::zeros(2, 2);
        a.set(0, 0, true);
        a.set(1, 1, true);

        let mut b = BitMatrix::zeros(2, 2);
        b.set(0, 1, true);
        b.set(1, 0, true);

        let c = multiply(&a, &b);

        assert!(c.get(0, 1));
        assert!(c.get(1, 0));
        assert!(!c.get(0, 0));
        assert!(!c.get(1, 1));
    }
}
