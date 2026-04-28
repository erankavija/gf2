//! M4RM (Method of the Four Russians for Multiplication) algorithm.
//!
//! This module implements fast matrix multiplication over GF(2) using the M4RM algorithm.
//! The algorithm processes the multiplication in blocks, precomputing Gray code tables
//! of linear combinations to reduce the number of row operations.
//!
//! When the `simd` feature is enabled, row XOR operations automatically use AVX2
//! vectorization for large matrices, providing significant speedups.
//!
//! # Gray-code table build (kernel B2)
//!
//! [`build_gray_table_flat`] is the B2 hot loop in the M4RM PPC spiral.  The
//! optimized path tiles each table row into 8-word chunks and keeps one running
//! accumulator per chunk, exposing independent XOR chains while preserving the
//! hoisted `XorInplaceFn` dispatch used by the surrounding M4RM row updates.

use crate::kernels::ops::{resolve_xor_inplace, XorInplaceFn};
use crate::matrix::BitMatrix;

/// Column-word tile width for the V2 ILP Gray-table builder.
///
/// Eight `u64` words are 512 bits: enough independent scalar accumulators to
/// break the single-vector dependency chain and a natural size for AVX2 codegen
/// when LLVM batches the XOR/store loops.
const B2_GRAY_TILE_WORDS: usize = 8;
const B2_GRAY_MAX_TILES: usize = 4;

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
            crate::kernels::ops::xor_inplace(&mut current, row_words);
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
/// * `xor` - Pre-resolved row XOR operation for `stride_words`-wide rows
///
/// # Layout
///
/// The buffer stores table entries sequentially:
/// - Entry 0: buffer[0..stride_words]
/// - Entry 1: buffer[stride_words..2*stride_words]
/// - Entry i: buffer[i*stride_words..(i+1)*stride_words]
#[doc(hidden)]
#[inline(never)]
pub fn build_gray_table_flat(
    b: &BitMatrix,
    row_start: usize,
    k_block: usize,
    n: usize,
    buffer: &mut [u64],
    xor: XorInplaceFn,
) {
    let table_size = 1usize << k_block;
    let stride_words = if n == 0 { 0 } else { n.div_ceil(64) };

    if stride_words == 0 || table_size == 0 {
        return;
    }

    debug_assert!(
        buffer.len() >= table_size * stride_words,
        "build_gray_table_flat: buffer too small ({} < {} × {})",
        buffer.len(),
        table_size,
        stride_words
    );

    #[cfg(feature = "simd")]
    if stride_words == 2 * B2_GRAY_TILE_WORDS {
        if let Some(fns) = crate::simd::maybe_simd() {
            gray_walk_stride16_simd(
                b,
                row_start,
                table_size,
                stride_words,
                buffer,
                fns.m4rm_gray_xor16_fn,
            );
            return;
        }
    }

    if stride_words <= B2_GRAY_MAX_TILES * B2_GRAY_TILE_WORDS {
        let tile_count = stride_words.div_ceil(B2_GRAY_TILE_WORDS);
        let last_tile_words = stride_words - (tile_count - 1) * B2_GRAY_TILE_WORDS;

        match (tile_count, last_tile_words) {
            (1, B2_GRAY_TILE_WORDS) => {
                gray_walk_full::<1>(b, row_start, table_size, stride_words, buffer)
            }
            (2, B2_GRAY_TILE_WORDS) => {
                gray_walk_full::<2>(b, row_start, table_size, stride_words, buffer)
            }
            (3, B2_GRAY_TILE_WORDS) => {
                gray_walk_full::<3>(b, row_start, table_size, stride_words, buffer)
            }
            (4, B2_GRAY_TILE_WORDS) => {
                gray_walk_full::<4>(b, row_start, table_size, stride_words, buffer)
            }
            _ => gray_walk_partial(
                b,
                row_start,
                table_size,
                stride_words,
                tile_count,
                last_tile_words,
                buffer,
            ),
        }
    } else {
        build_gray_table_flat_v0(b, row_start, table_size, stride_words, buffer, xor);
    }
}

#[cfg(feature = "simd")]
#[inline]
fn gray_walk_stride16_simd(
    b: &BitMatrix,
    row_start: usize,
    table_size: usize,
    stride_words: usize,
    buffer: &mut [u64],
    xor16: fn(&mut [[u64; 8]; 2], &[u64]),
) {
    debug_assert_eq!(stride_words, 2 * B2_GRAY_TILE_WORDS);

    let mut acc = [[0u64; B2_GRAY_TILE_WORDS]; 2];
    buffer[..stride_words].fill(0);

    let mut prev_gray = 0usize;
    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1);
        let diff = prev_gray ^ curr_gray;
        let bit_pos = diff.trailing_zeros() as usize;

        let abs_row = row_start + bit_pos;
        if abs_row < b.rows() {
            xor16(&mut acc, b.row_words(abs_row));
        }

        let entry_start = curr_gray * stride_words;
        buffer[entry_start..entry_start + B2_GRAY_TILE_WORDS].copy_from_slice(&acc[0]);
        buffer[entry_start + B2_GRAY_TILE_WORDS..entry_start + stride_words]
            .copy_from_slice(&acc[1]);

        prev_gray = curr_gray;
    }
}

#[inline]
fn gray_walk_full<const TILES: usize>(
    b: &BitMatrix,
    row_start: usize,
    table_size: usize,
    stride_words: usize,
    buffer: &mut [u64],
) {
    debug_assert_eq!(stride_words, TILES * B2_GRAY_TILE_WORDS);

    let mut acc = [[0u64; B2_GRAY_TILE_WORDS]; TILES];
    buffer[..stride_words].fill(0);

    let mut prev_gray = 0usize;
    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1);
        let diff = prev_gray ^ curr_gray;
        let bit_pos = diff.trailing_zeros() as usize;

        let abs_row = row_start + bit_pos;
        if abs_row < b.rows() {
            let row_words = b.row_words(abs_row);
            for t in 0..TILES {
                let base = t * B2_GRAY_TILE_WORDS;
                let src = &row_words[base..base + B2_GRAY_TILE_WORDS];
                acc[t][0] ^= src[0];
                acc[t][1] ^= src[1];
                acc[t][2] ^= src[2];
                acc[t][3] ^= src[3];
                acc[t][4] ^= src[4];
                acc[t][5] ^= src[5];
                acc[t][6] ^= src[6];
                acc[t][7] ^= src[7];
            }
        }

        let entry_start = curr_gray * stride_words;
        for t in 0..TILES {
            let base = entry_start + t * B2_GRAY_TILE_WORDS;
            buffer[base..base + B2_GRAY_TILE_WORDS].copy_from_slice(&acc[t]);
        }

        prev_gray = curr_gray;
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn gray_walk_partial(
    b: &BitMatrix,
    row_start: usize,
    table_size: usize,
    stride_words: usize,
    tile_count: usize,
    last_tile_words: usize,
    buffer: &mut [u64],
) {
    debug_assert!(tile_count > 0 && tile_count <= B2_GRAY_MAX_TILES);
    debug_assert!((1..=B2_GRAY_TILE_WORDS).contains(&last_tile_words));

    let mut acc = [[0u64; B2_GRAY_TILE_WORDS]; B2_GRAY_MAX_TILES];
    buffer[..stride_words].fill(0);

    let full_tiles = tile_count - 1;
    let mut prev_gray = 0usize;
    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1);
        let diff = prev_gray ^ curr_gray;
        let bit_pos = diff.trailing_zeros() as usize;

        let abs_row = row_start + bit_pos;
        if abs_row < b.rows() {
            let row_words = b.row_words(abs_row);
            for (t, tile_acc) in acc.iter_mut().take(full_tiles).enumerate() {
                let base = t * B2_GRAY_TILE_WORDS;
                let src = &row_words[base..base + B2_GRAY_TILE_WORDS];
                for w in 0..B2_GRAY_TILE_WORDS {
                    tile_acc[w] ^= src[w];
                }
            }

            let last_base = full_tiles * B2_GRAY_TILE_WORDS;
            let src = &row_words[last_base..last_base + last_tile_words];
            for (dst, &src) in acc[full_tiles][..last_tile_words].iter_mut().zip(src) {
                *dst ^= src;
            }
        }

        let entry_start = curr_gray * stride_words;
        for (t, tile_acc) in acc.iter().take(full_tiles).enumerate() {
            let base = entry_start + t * B2_GRAY_TILE_WORDS;
            buffer[base..base + B2_GRAY_TILE_WORDS].copy_from_slice(tile_acc);
        }

        let last_base = entry_start + full_tiles * B2_GRAY_TILE_WORDS;
        buffer[last_base..last_base + last_tile_words]
            .copy_from_slice(&acc[full_tiles][..last_tile_words]);

        prev_gray = curr_gray;
    }
}

fn build_gray_table_flat_v0(
    b: &BitMatrix,
    row_start: usize,
    table_size: usize,
    stride_words: usize,
    buffer: &mut [u64],
    xor: XorInplaceFn,
) {
    let mut current = vec![0u64; stride_words];
    let mut prev_gray = 0usize;

    buffer[..stride_words].fill(0);

    for i in 1..table_size {
        let curr_gray = i ^ (i >> 1);

        let diff = prev_gray ^ curr_gray;
        let bit_pos = diff.trailing_zeros() as usize;

        if row_start + bit_pos < b.rows() {
            let row_words = b.row_words(row_start + bit_pos);
            xor(&mut current, row_words);
        }

        let entry_start = curr_gray * stride_words;
        buffer[entry_start..entry_start + stride_words].copy_from_slice(&current);

        prev_gray = curr_gray;
    }
}

/// Extracts k_block consecutive bits from a bit-packed row starting at column col_start.
///
/// Returns an index into the Gray code table (0..2^k_block).
fn extract_bits_from_row_words(row_words: &[u64], col_start: usize, k_block: usize) -> usize {
    debug_assert!(k_block <= usize::BITS as usize);

    if k_block == 0 {
        return 0;
    }

    let word_idx = col_start >> 6;
    let bit_offset = col_start & 63;
    let mut bits = row_words.get(word_idx).copied().unwrap_or(0) >> bit_offset;

    if bit_offset + k_block > 64 {
        bits |= row_words.get(word_idx + 1).copied().unwrap_or(0) << (64 - bit_offset);
    }

    let mask = if k_block == 64 {
        u64::MAX
    } else {
        (1u64 << k_block) - 1
    };
    (bits & mask) as usize
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

    if m == 0 || k == 0 || n == 0 {
        return BitMatrix::zeros(m, n);
    }

    let k_block = choose_k_block(k, n);
    let table_size = 1usize << k_block;
    let stride_words = n.div_ceil(64);
    let xor = resolve_xor_inplace(stride_words);

    if k_block == 1 {
        return a.mul_row_xor_dispatch(b);
    }

    let mut c = BitMatrix::zeros(m, n);

    // Pre-allocate flat buffer for Gray code table (reused across all panels)
    // This eliminates ~33 MB of allocation churn for 1024×1024 matrices
    let mut table_buffer = vec![0u64; table_size * stride_words];

    // Process B in panels of k_block rows
    let mut panel_start = 0;
    while panel_start < k {
        let panel_size = k_block.min(k - panel_start);

        // Rebuild table in the flat buffer (no need to clear - gray code overwrites all)
        build_gray_table_flat(b, panel_start, panel_size, n, &mut table_buffer, xor);

        // For each row of A
        for i in 0..m {
            // Extract k_block bits from row i of A starting at panel_start
            let idx = extract_bits_from_row_words(a.row_words(i), panel_start, panel_size);

            // XOR the corresponding table entry into row i of C
            let entry_start = idx * stride_words;
            let entry_end = entry_start + stride_words;
            let table_entry = &table_buffer[entry_start..entry_end];

            let c_row = c.row_words_mut(i);
            xor(c_row, table_entry);
        }

        panel_start += k_block;
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernels::ops::xor_inplace;

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
        let bits = extract_bits_from_row_words(a.row_words(0), 0, 4);
        assert_eq!(bits, 0b1010);
    }

    #[test]
    fn test_extract_bits_crosses_word_boundary() {
        let mut a = BitMatrix::zeros(1, 130);
        a.set(0, 63, true);
        a.set(0, 64, true);
        a.set(0, 66, true);

        let bits = extract_bits_from_row_words(a.row_words(0), 63, 5);
        assert_eq!(bits, 0b01011);
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
        let xor = resolve_xor_inplace(stride_words);
        build_gray_table_flat(&b, 0, 8, 128, &mut table_flat, xor);

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
