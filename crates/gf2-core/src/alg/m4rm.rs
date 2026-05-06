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
const M4RM_DEFAULT_TABLE_BYTES: usize = 64 * 1024;
const M4RM_DEFAULT_MAX_K: usize = 8;

/// Row accumulators held by the M4RM C-tile update.
const M4RM_TILE_ROWS: usize = 8;
/// Column words per register tile: four u64 lanes fit exactly in one YMM.
const M4RM_TILE_WORDS: usize = 4;
/// Keep sub-crossover sizes on the original row-XOR schedule.
const M4RM_TILED_MIN_STRIDE_WORDS: usize = 16;
type M4rmTile8xNFn = fn(&mut [u64], usize, &[u64], &[usize; M4RM_TILE_ROWS]);

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
    choose_k_block_with_limit(k, n, M4RM_DEFAULT_TABLE_BYTES, M4RM_DEFAULT_MAX_K)
}

fn choose_k_block_with_limit(
    k: usize,
    n: usize,
    target_table_bytes: usize,
    max_k_block: usize,
) -> usize {
    // Each table entry is a row of n bits, stored in stride_words u64s
    let stride_words = if n == 0 { 0 } else { n.div_ceil(64) };
    let bytes_per_entry = stride_words * 8;

    if k == 0 || max_k_block == 0 {
        return 0;
    }

    let largest_k = max_k_block.min(k).min(usize::BITS as usize - 1);
    if bytes_per_entry == 0 {
        return largest_k;
    }

    let max_table_entries = target_table_bytes / bytes_per_entry;
    for k_block in (1..=largest_k).rev() {
        let table_entries = 1usize << k_block;

        if table_entries <= max_table_entries {
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
            for (t, tile_acc) in acc.iter_mut().enumerate().take(TILES) {
                let base = t * B2_GRAY_TILE_WORDS;
                let src = &row_words[base..base + B2_GRAY_TILE_WORDS];
                tile_acc[0] ^= src[0];
                tile_acc[1] ^= src[1];
                tile_acc[2] ^= src[2];
                tile_acc[3] ^= src[3];
                tile_acc[4] ^= src[4];
                tile_acc[5] ^= src[5];
                tile_acc[6] ^= src[6];
                tile_acc[7] ^= src[7];
            }
        }

        let entry_start = curr_gray * stride_words;
        for (t, tile_acc) in acc.iter().enumerate().take(TILES) {
            let base = entry_start + t * B2_GRAY_TILE_WORDS;
            buffer[base..base + B2_GRAY_TILE_WORDS].copy_from_slice(tile_acc);
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
    let stride_words = n.div_ceil(64);
    let xor = resolve_xor_inplace(stride_words);

    if k_block == 1 {
        return a.mul_row_xor_dispatch(b);
    }

    if use_register_tiled_schedule(m, stride_words) {
        if let Some(tile8xn) = resolve_m4rm_tile8xn() {
            return multiply_register_tiled(a, b, k_block, stride_words, xor, tile8xn);
        }
    }

    multiply_rowwise_panels(a, b, k_block, stride_words, xor)
}

/// Multiplies two matrices with an experimental M4RI-style table schedule.
///
/// This is a test-support harness, not the production M4RM schedule-selection
/// policy. It is intended to stay bit-exact with [`multiply`] while tests and
/// benchmarks vary the Gray-table byte budget and maximum panel width.
///
/// # Arguments
///
/// * `a` - Left-hand matrix with dimensions `m × k`.
/// * `b` - Right-hand matrix with dimensions `k × n`.
/// * `target_table_bytes` - Maximum byte budget for one Gray-code table panel.
/// * `max_k_block` - Maximum candidate panel width in bits. Oversized values
///   are safely ignored when the implied table would exceed the byte budget.
///
/// # Panics
///
/// Panics if `a.cols() != b.rows()`, if `target_table_bytes` is zero, or if
/// `max_k_block` is zero.
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub fn multiply_with_table_schedule_for_test(
    a: &BitMatrix,
    b: &BitMatrix,
    target_table_bytes: usize,
    max_k_block: usize,
) -> BitMatrix {
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
    assert!(
        target_table_bytes > 0,
        "target_table_bytes must be positive"
    );
    assert!(max_k_block > 0, "max_k_block must be positive");

    if m == 0 || k == 0 || n == 0 {
        return BitMatrix::zeros(m, n);
    }

    let k_block = choose_k_block_with_limit(k, n, target_table_bytes, max_k_block);
    let stride_words = n.div_ceil(64);
    let xor = resolve_xor_inplace(stride_words);

    if k_block == 1 {
        return a.mul_row_xor_dispatch(b);
    }

    if use_register_tiled_schedule(m, stride_words) {
        if let Some(tile8xn) = resolve_m4rm_tile8xn() {
            return multiply_register_tiled(a, b, k_block, stride_words, xor, tile8xn);
        }
    }

    multiply_rowwise_panels(a, b, k_block, stride_words, xor)
}
#[doc(hidden)]
#[cfg(any(test, feature = "test-support"))]
pub fn multiply_rowwise_for_test(a: &BitMatrix, b: &BitMatrix) -> BitMatrix {
    let k = a.cols();
    let n = b.cols();
    assert_eq!(
        k,
        b.rows(),
        "incompatible dimensions: A is {}×{} but B is {}×{}",
        a.rows(),
        k,
        b.rows(),
        n
    );

    if a.rows() == 0 || k == 0 || n == 0 {
        return BitMatrix::zeros(a.rows(), n);
    }

    let k_block = choose_k_block(k, n);
    if k_block == 1 {
        return a.mul_row_xor_dispatch(b);
    }

    let stride_words = n.div_ceil(64);
    let xor = resolve_xor_inplace(stride_words);
    multiply_rowwise_panels(a, b, k_block, stride_words, xor)
}

#[inline]
fn use_register_tiled_schedule(m: usize, stride_words: usize) -> bool {
    m >= M4RM_TILE_ROWS && stride_words >= M4RM_TILED_MIN_STRIDE_WORDS
}

#[inline]
fn use_next_panel_prefetch(stride_words: usize) -> bool {
    // Keeping both 1024-column tables resident evicts the just-built current
    // table from L1 on the pinned B3 design size; enable V6 for wider tiles.
    stride_words > M4RM_TILED_MIN_STRIDE_WORDS
}

#[inline]
fn resolve_m4rm_tile8xn() -> Option<M4rmTile8xNFn> {
    #[cfg(feature = "simd")]
    {
        crate::simd::maybe_simd().map(|backend| backend.m4rm_tile8xn_fn)
    }
    #[cfg(not(feature = "simd"))]
    {
        None
    }
}

fn multiply_rowwise_panels(
    a: &BitMatrix,
    b: &BitMatrix,
    k_block: usize,
    stride_words: usize,
    xor: XorInplaceFn,
) -> BitMatrix {
    let m = a.rows();
    let k = a.cols();
    let n = b.cols();
    let table_size = 1usize << k_block;
    let mut c = BitMatrix::zeros(m, n);

    // Pre-allocate flat buffer for Gray code table (reused across all panels).
    let mut table_buffer = vec![0u64; table_size * stride_words];

    let mut panel_start = 0;
    while panel_start < k {
        let panel_size = k_block.min(k - panel_start);
        build_gray_table_flat(b, panel_start, panel_size, n, &mut table_buffer, xor);
        update_panel_rowwise(
            a,
            &mut c,
            panel_start,
            panel_size,
            &table_buffer,
            stride_words,
            xor,
        );
        panel_start += k_block;
    }

    c
}

fn multiply_register_tiled(
    a: &BitMatrix,
    b: &BitMatrix,
    k_block: usize,
    stride_words: usize,
    xor: XorInplaceFn,
    tile8xn: M4rmTile8xNFn,
) -> BitMatrix {
    let m = a.rows();
    let k = a.cols();
    let n = b.cols();
    let table_size = 1usize << k_block;
    let mut c = BitMatrix::zeros(m, n);

    if !use_next_panel_prefetch(stride_words) {
        let mut table_buffer = vec![0u64; table_size * stride_words];

        let mut panel_start = 0usize;
        while panel_start < k {
            let panel_size = k_block.min(k - panel_start);
            build_gray_table_flat(b, panel_start, panel_size, n, &mut table_buffer, xor);
            update_panel_register_tiled(
                a,
                &mut c,
                panel_start,
                panel_size,
                &table_buffer,
                None,
                stride_words,
                xor,
                tile8xn,
            );
            panel_start += k_block;
        }

        return c;
    }

    let mut table_buffers = [
        vec![0u64; table_size * stride_words],
        vec![0u64; table_size * stride_words],
    ];

    let mut current_slot = 0usize;
    let mut panel_start = 0usize;
    let mut panel_size = k_block.min(k - panel_start);
    build_gray_table_flat(
        b,
        panel_start,
        panel_size,
        n,
        &mut table_buffers[current_slot],
        xor,
    );

    loop {
        let next_panel_start = panel_start + k_block;
        let next_meta = (next_panel_start < k).then(|| {
            let next_slot = current_slot ^ 1;
            let next_panel_size = k_block.min(k - next_panel_start);
            build_gray_table_flat(
                b,
                next_panel_start,
                next_panel_size,
                n,
                &mut table_buffers[next_slot],
                xor,
            );
            (next_slot, next_panel_start, next_panel_size)
        });

        let next_table =
            next_meta.map(|(slot, start, size)| (&table_buffers[slot][..], start, size));
        update_panel_register_tiled(
            a,
            &mut c,
            panel_start,
            panel_size,
            &table_buffers[current_slot],
            next_table,
            stride_words,
            xor,
            tile8xn,
        );

        if let Some((slot, start, size)) = next_meta {
            current_slot = slot;
            panel_start = start;
            panel_size = size;
        } else {
            break;
        }
    }

    c
}

fn update_panel_rowwise(
    a: &BitMatrix,
    c: &mut BitMatrix,
    panel_start: usize,
    panel_size: usize,
    table_buffer: &[u64],
    stride_words: usize,
    xor: XorInplaceFn,
) {
    for i in 0..a.rows() {
        let idx = extract_bits_from_row_words(a.row_words(i), panel_start, panel_size);
        let entry_start = idx * stride_words;
        let table_entry = &table_buffer[entry_start..entry_start + stride_words];
        xor(c.row_words_mut(i), table_entry);
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn update_panel_register_tiled(
    a: &BitMatrix,
    c: &mut BitMatrix,
    panel_start: usize,
    panel_size: usize,
    table_buffer: &[u64],
    next_table: Option<(&[u64], usize, usize)>,
    stride_words: usize,
    xor: XorInplaceFn,
    tile8xn: M4rmTile8xNFn,
) {
    let full_rows = a.rows() / M4RM_TILE_ROWS * M4RM_TILE_ROWS;

    for row_start in (0..full_rows).step_by(M4RM_TILE_ROWS) {
        let idx = row_tile_indices::<M4RM_TILE_ROWS>(a, row_start, panel_start, panel_size);
        let next_idx = next_table.map(|(_, next_start, next_size)| {
            row_tile_indices::<M4RM_TILE_ROWS>(a, row_start, next_start, next_size)
        });
        if let (Some((next, _, _)), Some(next_idx)) = (next_table, next_idx.as_ref()) {
            prefetch_next_tile8x4(next, stride_words, 0, next_idx);
        }

        let c_block = c.row_words_block_mut(row_start, M4RM_TILE_ROWS);
        tile8xn(c_block, stride_words, table_buffer, &idx);

        let full_words = stride_words / M4RM_TILE_WORDS * M4RM_TILE_WORDS;
        if full_words < stride_words {
            update_row_tile_tail(
                c,
                row_start,
                full_words,
                stride_words,
                table_buffer,
                next_table.map(|(next, _, _)| next),
                &idx,
                next_idx.as_ref(),
            );
        }
    }

    for row in full_rows..a.rows() {
        let idx = extract_bits_from_row_words(a.row_words(row), panel_start, panel_size);
        let entry_start = idx * stride_words;
        let table_entry = &table_buffer[entry_start..entry_start + stride_words];
        xor(c.row_words_mut(row), table_entry);
    }
}

#[inline(always)]
fn row_tile_indices<const ROWS: usize>(
    a: &BitMatrix,
    row_start: usize,
    panel_start: usize,
    panel_size: usize,
) -> [usize; ROWS] {
    core::array::from_fn(|r| {
        extract_bits_from_row_words(a.row_words(row_start + r), panel_start, panel_size)
    })
}

#[inline(always)]
fn prefetch_next_tile8x4(
    next_table: &[u64],
    stride_words: usize,
    word_start: usize,
    idx: &[usize; M4RM_TILE_ROWS],
) {
    for &entry in idx {
        let offset = entry * stride_words + word_start;
        gf2_kernels_simd::prefetch_read_l1(next_table[offset..].as_ptr().cast());
    }
}

#[cfg(test)]
#[inline(never)]
fn xor_tile8xn_scalar_block(
    c_block: &mut [u64],
    stride_words: usize,
    table_buffer: &[u64],
    idx: &[usize; M4RM_TILE_ROWS],
) {
    let mut word_start = 0usize;
    while word_start + M4RM_TILE_WORDS <= stride_words {
        xor_tile8x4_scalar_block(c_block, stride_words, word_start, table_buffer, idx);
        word_start += M4RM_TILE_WORDS;
    }
}

#[cfg(test)]
#[inline(never)]
fn xor_tile8x4_scalar_block(
    c_block: &mut [u64],
    stride_words: usize,
    word_start: usize,
    table_buffer: &[u64],
    idx: &[usize; M4RM_TILE_ROWS],
) {
    let mut acc0 = load_block4(c_block, stride_words, 0, word_start);
    let mut acc1 = load_block4(c_block, stride_words, 1, word_start);
    let mut acc2 = load_block4(c_block, stride_words, 2, word_start);
    let mut acc3 = load_block4(c_block, stride_words, 3, word_start);
    let mut acc4 = load_block4(c_block, stride_words, 4, word_start);
    let mut acc5 = load_block4(c_block, stride_words, 5, word_start);
    let mut acc6 = load_block4(c_block, stride_words, 6, word_start);
    let mut acc7 = load_block4(c_block, stride_words, 7, word_start);

    xor_acc4(
        &mut acc0,
        table_entry4(table_buffer, stride_words, idx[0], word_start),
    );
    xor_acc4(
        &mut acc1,
        table_entry4(table_buffer, stride_words, idx[1], word_start),
    );
    xor_acc4(
        &mut acc2,
        table_entry4(table_buffer, stride_words, idx[2], word_start),
    );
    xor_acc4(
        &mut acc3,
        table_entry4(table_buffer, stride_words, idx[3], word_start),
    );
    xor_acc4(
        &mut acc4,
        table_entry4(table_buffer, stride_words, idx[4], word_start),
    );
    xor_acc4(
        &mut acc5,
        table_entry4(table_buffer, stride_words, idx[5], word_start),
    );
    xor_acc4(
        &mut acc6,
        table_entry4(table_buffer, stride_words, idx[6], word_start),
    );
    xor_acc4(
        &mut acc7,
        table_entry4(table_buffer, stride_words, idx[7], word_start),
    );

    store_block4(c_block, stride_words, 0, word_start, acc0);
    store_block4(c_block, stride_words, 1, word_start, acc1);
    store_block4(c_block, stride_words, 2, word_start, acc2);
    store_block4(c_block, stride_words, 3, word_start, acc3);
    store_block4(c_block, stride_words, 4, word_start, acc4);
    store_block4(c_block, stride_words, 5, word_start, acc5);
    store_block4(c_block, stride_words, 6, word_start, acc6);
    store_block4(c_block, stride_words, 7, word_start, acc7);
}

#[cfg(test)]
#[inline(always)]
fn table_entry4(
    table_buffer: &[u64],
    stride_words: usize,
    idx: usize,
    word_start: usize,
) -> &[u64] {
    let start = idx * stride_words + word_start;
    &table_buffer[start..start + M4RM_TILE_WORDS]
}

#[cfg(test)]
#[inline(always)]
fn load_block4(
    c_block: &[u64],
    stride_words: usize,
    row: usize,
    word_start: usize,
) -> [u64; M4RM_TILE_WORDS] {
    let start = row * stride_words + word_start;
    [
        c_block[start],
        c_block[start + 1],
        c_block[start + 2],
        c_block[start + 3],
    ]
}

#[cfg(test)]
#[inline(always)]
fn store_block4(
    c_block: &mut [u64],
    stride_words: usize,
    row: usize,
    word_start: usize,
    acc: [u64; M4RM_TILE_WORDS],
) {
    let start = row * stride_words + word_start;
    c_block[start..start + M4RM_TILE_WORDS].copy_from_slice(&acc);
}

#[cfg(test)]
#[inline(always)]
fn xor_acc4(acc: &mut [u64; M4RM_TILE_WORDS], src: &[u64]) {
    acc[0] ^= src[0];
    acc[1] ^= src[1];
    acc[2] ^= src[2];
    acc[3] ^= src[3];
}

#[allow(clippy::too_many_arguments)]
fn update_row_tile_tail(
    c: &mut BitMatrix,
    row_start: usize,
    word_start: usize,
    stride_words: usize,
    table_buffer: &[u64],
    next_table: Option<&[u64]>,
    idx: &[usize; M4RM_TILE_ROWS],
    next_idx: Option<&[usize; M4RM_TILE_ROWS]>,
) {
    for r in 0..M4RM_TILE_ROWS {
        if let (Some(next), Some(next_idx)) = (next_table, next_idx) {
            gf2_kernels_simd::prefetch_read_l1(
                next[next_idx[r] * stride_words + word_start..]
                    .as_ptr()
                    .cast(),
            );
        }
        let start = idx[r] * stride_words + word_start;
        let table_entry = &table_buffer[start..idx[r] * stride_words + stride_words];
        let c_tail = &mut c.row_words_mut(row_start + r)[word_start..stride_words];
        for (dst, &src) in c_tail.iter_mut().zip(table_entry) {
            *dst ^= src;
        }
    }
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
    fn test_m4ri_style_schedule_selects_wider_panels_when_cache_budget_allows() {
        assert_eq!(choose_k_block_with_limit(4096, 4096, 64 * 1024, 8), 7);
        assert_eq!(choose_k_block_with_limit(4096, 4096, 128 * 1024, 10), 8);
        assert_eq!(choose_k_block_with_limit(4096, 4096, 256 * 1024, 10), 9);
        assert_eq!(choose_k_block_with_limit(4096, 4096, 512 * 1024, 10), 10);
    }

    #[test]
    fn test_m4ri_style_schedule_rejects_overflow_impossible_panel_width() {
        let selected = choose_k_block_with_limit(
            usize::BITS as usize - 1,
            1,
            usize::MAX,
            usize::BITS as usize - 1,
        );
        let bytes_per_entry = 8;
        let max_table_entries = usize::MAX / bytes_per_entry;

        assert_eq!(selected, usize::BITS as usize - 4);
        assert!((1usize << selected) <= max_table_entries);
        assert!((1usize << (selected + 1)) > max_table_entries);
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
    fn test_register_tiled_schedule_threshold_preserves_small_sizes() {
        assert!(!use_register_tiled_schedule(7, 16));
        assert!(!use_register_tiled_schedule(64, 8));
        assert!(use_register_tiled_schedule(8, 16));
        assert!(use_register_tiled_schedule(8, 32));
        assert!(!use_next_panel_prefetch(16));
        assert!(use_next_panel_prefetch(32));
    }

    #[test]
    fn test_register_tiled_multiply_matches_naive_with_wide_remainders() {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(0x19bc_3199);
        let (m, k, n) = (9, 17, 2048);
        let mut a = BitMatrix::zeros(m, k);
        let mut b = BitMatrix::zeros(k, n);

        for r in 0..m {
            for c in 0..k {
                if rng.gen_bool(0.5) {
                    a.set(r, c, true);
                }
            }
        }
        for r in 0..k {
            for c in 0..n {
                if rng.gen_bool(0.5) {
                    b.set(r, c, true);
                }
            }
        }

        let k_block = choose_k_block(k, n);
        let stride_words = n.div_ceil(64);
        let xor = resolve_xor_inplace(stride_words);
        let tiled =
            multiply_register_tiled(&a, &b, k_block, stride_words, xor, xor_tile8xn_scalar_block);

        for r in 0..m {
            for c in 0..n {
                let expected = (0..k).fold(false, |acc, kk| acc ^ (a.get(r, kk) && b.get(kk, c)));
                assert_eq!(
                    tiled.get(r, c),
                    expected,
                    "register-tiled mismatch at ({r}, {c})"
                );
            }
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

    #[test]
    fn test_m4ri_style_schedule_matches_production_on_boundary_shapes() {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        fn random_matrix(rows: usize, cols: usize, rng: &mut StdRng) -> BitMatrix {
            let mut matrix = BitMatrix::zeros(rows, cols);
            for r in 0..rows {
                for c in 0..cols {
                    if rng.gen_bool(0.5) {
                        matrix.set(r, c, true);
                    }
                }
            }
            matrix
        }

        let mut rng = StdRng::seed_from_u64(0x380e_041a);
        for (m, k, n) in [
            (0, 13, 65),
            (1, 1, 1),
            (7, 63, 64),
            (8, 64, 65),
            (9, 65, 129),
            (16, 257, 512),
        ] {
            let a = random_matrix(m, k, &mut rng);
            let b = random_matrix(k, n, &mut rng);

            let production = multiply(&a, &b);
            let wider_gray_table = multiply_with_table_schedule_for_test(&a, &b, 256 * 1024, 10);

            assert_eq!(
                wider_gray_table, production,
                "m4ri-style schedule mismatch for {m}×{k} times {k}×{n}"
            );
        }
    }
}
