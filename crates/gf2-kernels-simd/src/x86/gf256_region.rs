//! AVX2 GF(2^8) region kernels: two `vpshufb` nibble lookups per 32 bytes.
#![deny(unsafe_op_in_unsafe_fn)]

use crate::gf256_region::{
    check_gemm_block, check_mul_add, gemm_block_scalar, mul_add_scalar, mul_scalar, GemmBlock,
    Gf256RegionFns, SplitTable,
};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Rows of `c` a block step holds in registers across the inner dimension.
const ROWS: usize = 4;

pub(crate) fn detect() -> Option<Gf256RegionFns> {
    std::arch::is_x86_feature_detected!("avx2").then_some(Gf256RegionFns {
        mul_add: mul_add_avx2,
        mul: mul_avx2,
        gemm_block: gemm_block_avx2,
        name: "avx2-pshufb",
    })
}

fn mul_add_avx2(table: &SplitTable, x: &[u8], y: &mut [u8]) {
    check_mul_add(x, y);
    // SAFETY: detect publishes this entry only on AVX2 hosts.
    unsafe { mul_add_avx2_unchecked(table, x, y) }
}

fn mul_avx2(table: &SplitTable, x: &[u8], y: &mut [u8]) {
    check_mul_add(x, y);
    // SAFETY: detect publishes this entry only on AVX2 hosts.
    unsafe { mul_avx2_unchecked(table, x, y) }
}

/// # Safety
/// The host must support AVX2 and `x.len() == y.len()`.
#[target_feature(enable = "avx2")]
unsafe fn mul_avx2_unchecked(table: &SplitTable, x: &[u8], y: &mut [u8]) {
    let (low, high) = broadcast(table);
    let (xp, yp) = (x.as_ptr(), y.as_mut_ptr());
    let mut at = 0;
    while at + 32 <= x.len() {
        // SAFETY: at + 32 <= len for both slices; unaligned accesses are
        // permitted and the borrows keep the slices disjoint.
        unsafe {
            let x0 = _mm256_loadu_si256(xp.add(at).cast());
            _mm256_storeu_si256(yp.add(at).cast(), products(low, high, x0));
        }
        at += 32;
    }
    mul_scalar(table, &x[at..], &mut y[at..]);
}

fn gemm_block_avx2(block: &GemmBlock<'_>, c: &mut [u8], c_stride: usize) {
    if check_gemm_block(block, c, c_stride) {
        // SAFETY: detect publishes this entry only on AVX2 hosts, and the
        // check above bounds every row the block addresses.
        unsafe { gemm_block_avx2_unchecked(block, c, c_stride) }
    }
}

/// Both nibble tables of `table`, each in both 128-bit halves.
#[inline]
#[target_feature(enable = "avx2")]
fn broadcast(table: &SplitTable) -> (__m256i, __m256i) {
    // SAFETY: each array holds 16 bytes; unaligned loads are permitted.
    unsafe {
        (
            _mm256_broadcastsi128_si256(_mm_loadu_si128(table.low.as_ptr().cast())),
            _mm256_broadcastsi128_si256(_mm_loadu_si128(table.high.as_ptr().cast())),
        )
    }
}

/// The products of 32 values through one constant's broadcast tables.
#[inline]
#[target_feature(enable = "avx2")]
fn products(low: __m256i, high: __m256i, x: __m256i) -> __m256i {
    let mask = _mm256_set1_epi8(0x0f);
    let lo = _mm256_and_si256(x, mask);
    let hi = _mm256_and_si256(_mm256_srli_epi16::<4>(x), mask);
    _mm256_xor_si256(_mm256_shuffle_epi8(low, lo), _mm256_shuffle_epi8(high, hi))
}

/// # Safety
/// The host must support AVX2 and `x.len() == y.len()`.
#[target_feature(enable = "avx2")]
unsafe fn mul_add_avx2_unchecked(table: &SplitTable, x: &[u8], y: &mut [u8]) {
    let (low, high) = broadcast(table);
    let len = x.len();
    let (xp, yp) = (x.as_ptr(), y.as_mut_ptr());
    let mut at = 0;
    while at + 128 <= len {
        // SAFETY: at + 128 <= len for both slices; unaligned accesses are
        // permitted and the borrows keep the slices disjoint.
        unsafe {
            let x: [__m256i; 4] =
                core::array::from_fn(|v| _mm256_loadu_si256(xp.add(at + 32 * v).cast()));
            let y: [__m256i; 4] =
                core::array::from_fn(|v| _mm256_loadu_si256(yp.add(at + 32 * v).cast()));
            for v in 0..4 {
                _mm256_storeu_si256(
                    yp.add(at + 32 * v).cast(),
                    _mm256_xor_si256(y[v], products(low, high, x[v])),
                );
            }
        }
        at += 128;
    }
    while at + 32 <= len {
        // SAFETY: as above, for one vector.
        unsafe {
            let x0 = _mm256_loadu_si256(xp.add(at).cast());
            let y0 = _mm256_loadu_si256(yp.add(at).cast());
            _mm256_storeu_si256(
                yp.add(at).cast(),
                _mm256_xor_si256(y0, products(low, high, x0)),
            );
        }
        at += 32;
    }
    mul_add_scalar(table, &x[at..], &mut y[at..]);
}

/// # Safety
/// The host must support AVX2, and `check_gemm_block` must have accepted the
/// block with `c` and `c_stride`.
#[target_feature(enable = "avx2")]
unsafe fn gemm_block_avx2_unchecked(block: &GemmBlock<'_>, c: &mut [u8], c_stride: usize) {
    let vector_width = block.width / 32 * 32;
    let mut row = 0;
    while row < block.rows {
        let count = (block.rows - row).min(ROWS);
        // SAFETY: rows row..row + count lie inside the checked block.
        unsafe {
            match count {
                4 => rows_avx2::<4>(block, row, c, c_stride, vector_width),
                3 => rows_avx2::<3>(block, row, c, c_stride, vector_width),
                2 => rows_avx2::<2>(block, row, c, c_stride, vector_width),
                _ => rows_avx2::<1>(block, row, c, c_stride, vector_width),
            }
        }
        row += count;
    }
    if vector_width < block.width {
        gemm_block_scalar(block, c, c_stride, vector_width..block.width);
    }
}

/// Columns `0..vector_width` of rows `first..first + R`, `vector_width` a
/// multiple of 32; each 32-column strip of the `R` rows stays in registers
/// across the inner dimension.
///
/// # Safety
/// The host must support AVX2, the block must be checked against `c` and
/// `c_stride`, `first + R <= rows` and `vector_width <= width`.
#[target_feature(enable = "avx2")]
unsafe fn rows_avx2<const R: usize>(
    block: &GemmBlock<'_>,
    first: usize,
    c: &mut [u8],
    c_stride: usize,
    vector_width: usize,
) {
    let a = &block.a[first * block.a_stride..];
    let (bp, cp) = (block.b.as_ptr(), c.as_mut_ptr());
    let mut column = 0;
    while column < vector_width {
        let mut sums = [_mm256_setzero_si256(); R];
        for p in 0..block.depth {
            // SAFETY: p < depth and column + 32 <= width, inside the checked
            // span of b; unaligned loads are permitted.
            let x = unsafe { _mm256_loadu_si256(bp.add(p * block.b_stride + column).cast()) };
            for (r, sum) in sums.iter_mut().enumerate() {
                let (low, high) = broadcast(&block.tables[usize::from(a[r * block.a_stride + p])]);
                *sum = _mm256_xor_si256(*sum, products(low, high, x));
            }
        }
        for (r, sum) in sums.iter().enumerate() {
            let at = (first + r) * c_stride + column;
            // SAFETY: row first + r and columns column..column + 32 lie inside
            // the checked span of c.
            unsafe {
                let old = _mm256_loadu_si256(cp.add(at).cast());
                _mm256_storeu_si256(cp.add(at).cast(), _mm256_xor_si256(old, *sum));
            }
        }
        column += 32;
    }
}
