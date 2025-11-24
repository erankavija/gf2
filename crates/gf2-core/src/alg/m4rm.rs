//! M4RM (Method of the Four Russians for Multiplication) algorithm.
//!
//! This module implements fast matrix multiplication over GF(2) using the M4RM algorithm.
//! The algorithm processes the multiplication in blocks, precomputing Gray code tables
//! of linear combinations to reduce the number of row operations.
//!
//! When the `simd` feature is enabled, row XOR operations automatically use AVX2
//! vectorization for large matrices, providing significant speedups.

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
/// Uses Gray code ordering for efficient table generation. Each Gray code step differs
/// by exactly one bit from the previous, requiring only a single XOR operation per entry
/// instead of multiple XORs with binary enumeration.
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
#[cfg(test)]
fn build_gray_table(b: &BitMatrix, row_start: usize, k_block: usize, n: usize) -> Vec<Vec<u64>> {
    let table_size = 1usize << k_block;
    let stride_words = if n == 0 { 0 } else { n.div_ceil(64) };

    let mut table = vec![vec![0u64; stride_words]; table_size];

    // Use Gray code ordering for efficient table generation
    // Gray code: G(i) = i XOR (i >> 1)
    // Each step differs by exactly one bit, so we only need one XOR per entry

    let mut current = vec![0u64; stride_words];
    let mut prev_gray = 0usize;

    // First entry (all zeros) is already initialized
    table[0].copy_from_slice(&current);

    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1); // Gray code formula

        // Find which bit flipped between previous and current Gray code
        let diff = prev_gray ^ curr_gray;
        let bit_pos = diff.trailing_zeros() as usize;

        // XOR in (or out) the corresponding row
        if row_start + bit_pos < b.rows() {
            let row_words = b.row_words(row_start + bit_pos);
            xor_inplace(&mut current, row_words);
        }

        // Store in table at the Gray code position
        table[curr_gray].copy_from_slice(&current);

        prev_gray = curr_gray;
    }

    table
}

/// Builds a lookup table into a pre-allocated flat buffer.
///
/// This is a memory-optimized version that writes directly to a flat buffer
/// instead of allocating a Vec<Vec<u64>>. This eliminates allocation overhead
/// when the table is rebuilt multiple times (e.g., for each panel in M4RM).
///
/// Uses Gray code ordering for efficient table generation.
///
/// # Arguments
///
/// * `b` - Input matrix B
/// * `row_start` - Starting row index in B
/// * `k_block` - Number of rows to include in the table
/// * `n` - Number of columns in B
/// * `buffer` - Pre-allocated flat buffer to write table into
///   Must have length >= (2^k_block) * stride_words
///
/// # Layout
///
/// The buffer stores table entries sequentially:
/// - Entry 0: buffer[0..stride_words]
/// - Entry 1: buffer[stride_words..2*stride_words]
/// - Entry i: buffer[i*stride_words..(i+1)*stride_words]
fn build_gray_table_flat(
    b: &BitMatrix,
    row_start: usize,
    k_block: usize,
    n: usize,
    buffer: &mut [u64],
) {
    let table_size = 1usize << k_block;
    let stride_words = if n == 0 { 0 } else { n.div_ceil(64) };

    // Use Gray code ordering for efficient table generation
    let mut current = vec![0u64; stride_words];
    let mut prev_gray = 0usize;

    // First entry (all zeros) - explicitly zero it
    let entry_start = 0;
    buffer[entry_start..entry_start + stride_words].fill(0);

    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1); // Gray code formula

        // Find which bit flipped between previous and current Gray code
        let diff = prev_gray ^ curr_gray;
        let bit_pos = diff.trailing_zeros() as usize;

        // XOR in (or out) the corresponding row
        if row_start + bit_pos < b.rows() {
            let row_words = b.row_words(row_start + bit_pos);
            xor_inplace(&mut current, row_words);
        }

        // Copy to buffer at the correct position
        let entry_start = curr_gray * stride_words;
        buffer[entry_start..entry_start + stride_words].copy_from_slice(&current);

        prev_gray = curr_gray;
    }
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
    let table_size = 1usize << k_block;
    let stride_words = n.div_ceil(64);

    // Pre-allocate flat buffer for Gray code table (reused across all panels)
    // This eliminates ~33 MB of allocation churn for 1024×1024 matrices
    let mut table_buffer = vec![0u64; table_size * stride_words];

    // Process B in panels of k_block rows
    let mut panel_start = 0;
    while panel_start < k {
        let panel_size = k_block.min(k - panel_start);

        // Rebuild table in the flat buffer (no need to clear - gray code overwrites all)
        build_gray_table_flat(b, panel_start, panel_size, n, &mut table_buffer);

        // For each row of A
        for i in 0..m {
            // Extract k_block bits from row i of A starting at panel_start
            let idx = extract_bits(a, i, panel_start, panel_size);

            // XOR the corresponding table entry into row i of C
            let entry_start = idx * stride_words;
            let entry_end = entry_start + stride_words;
            let table_entry = &table_buffer[entry_start..entry_end];

            let c_row = c.row_words_mut(i);
            xor_inplace(c_row, table_entry);
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

    #[test]
    fn test_gray_code_table_correctness() {
        // Verify Gray code table generates correct linear combinations
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(42);
        let mut b = BitMatrix::zeros(8, 64);
        for r in 0..8 {
            for c in 0..64 {
                if rng.gen_bool(0.5) {
                    b.set(r, c, true);
                }
            }
        }

        let table = build_gray_table(&b, 0, 8, 64);

        // Verify each table entry is correct XOR of indicated rows
        for (idx, entry) in table.iter().enumerate().take(256) {
            let mut expected = vec![0u64; 1];
            for bit in 0..8 {
                if (idx & (1 << bit)) != 0 {
                    let row_words = b.row_words(bit);
                    xor_inplace(&mut expected, row_words);
                }
            }

            assert_eq!(&entry[..], &expected[..], "Table entry {} mismatch", idx);
        }
    }

    #[test]
    fn test_flat_buffer_equivalence() {
        // Verify flat buffer version produces same results as Vec<Vec<>>
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(123);
        let mut b = BitMatrix::zeros(8, 128);
        for r in 0..8 {
            for c in 0..128 {
                if rng.gen_bool(0.5) {
                    b.set(r, c, true);
                }
            }
        }

        // Generate with original version
        let table_vec = build_gray_table(&b, 0, 8, 128);

        // Generate with flat buffer version
        let table_size = 256;
        let stride_words = 128_usize.div_ceil(64);
        let mut table_flat = vec![0u64; table_size * stride_words];
        build_gray_table_flat(&b, 0, 8, 128, &mut table_flat);

        // Compare all entries
        for (idx, vec_entry) in table_vec.iter().enumerate() {
            let entry_start = idx * stride_words;
            let entry_end = entry_start + stride_words;
            let flat_entry = &table_flat[entry_start..entry_end];

            assert_eq!(
                flat_entry,
                &vec_entry[..],
                "Entry {} mismatch between flat and Vec<Vec<>> versions",
                idx
            );
        }
    }

    #[test]
    fn test_multiply_with_flat_buffer() {
        // End-to-end test that flat buffer version produces correct results
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(456);

        // Test various sizes
        for size in [64, 128, 256] {
            let mut a = BitMatrix::zeros(size, size);
            let mut b = BitMatrix::zeros(size, size);

            for r in 0..size {
                for c in 0..size {
                    if rng.gen_bool(0.5) {
                        a.set(r, c, true);
                    }
                    if rng.gen_bool(0.5) {
                        b.set(r, c, true);
                    }
                }
            }

            let c = multiply(&a, &b);

            // Verify result dimensions
            assert_eq!(c.rows(), size);
            assert_eq!(c.cols(), size);

            // Verify a few spot checks with naive multiplication
            for i in 0..size.min(10) {
                for j in 0..size.min(10) {
                    let mut expected = false;
                    for k in 0..size {
                        if a.get(i, k) && b.get(k, j) {
                            expected = !expected; // XOR in GF(2)
                        }
                    }
                    assert_eq!(
                        c.get(i, j),
                        expected,
                        "Mismatch at ({}, {}) for size {}",
                        i,
                        j,
                        size
                    );
                }
            }
        }
    }
}
