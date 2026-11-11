//! GF(2^8) region multiply-add and matrix-product blocks over bytes, with
//! multiplication by a constant split into two 16-entry nibble tables. The
//! kernels take the tables as data, so they serve every GF(2^8) modulus.

#![forbid(unsafe_code)]

/// Multiplication by one constant `c`, split by nibble:
/// `c · v = low[v & 15] ^ high[v >> 4]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(32))]
pub struct SplitTable {
    /// `c · i` for `i` in `0..16`.
    pub low: [u8; 16],
    /// `c · (i << 4)` for `i` in `0..16`.
    pub high: [u8; 16],
}

impl SplitTable {
    /// The product `c · value` the table encodes.
    #[inline]
    #[must_use]
    pub fn apply(&self, value: u8) -> u8 {
        self.low[usize::from(value & 15)] ^ self.high[usize::from(value >> 4)]
    }
}

/// One block of a byte matrix product: `rows × depth` coefficients of `a`,
/// row `r` at `a[r * a_stride..]`, and `depth × width` values of `b`, row `p`
/// at `b[p * b_stride..]`; coefficient `v` multiplies through `tables[v]`.
#[derive(Clone, Copy, Debug)]
pub struct GemmBlock<'a> {
    /// The split table of every constant of the field.
    pub tables: &'a [SplitTable; 256],
    /// Left-hand coefficients.
    pub a: &'a [u8],
    /// Distance between consecutive rows of `a`.
    pub a_stride: usize,
    /// Right-hand values.
    pub b: &'a [u8],
    /// Distance between consecutive rows of `b`.
    pub b_stride: usize,
    /// Rows of the block.
    pub rows: usize,
    /// Inner dimension of the block.
    pub depth: usize,
    /// Columns of the block.
    pub width: usize,
}

/// `y[i] ^= table.apply(x[i])` for every `i`. Takes O(`x.len()`) time.
///
/// # Panics
///
/// Panics unless `x.len() == y.len()`.
pub type MulAddFn = fn(&SplitTable, &[u8], &mut [u8]);

/// `c[r * c_stride + j] ^= XOR over p of tables[a[r * a_stride + p]]
/// .apply(b[p * b_stride + j])` for `r < rows`, `j < width`. Takes
/// O(`rows * depth * width`) time.
///
/// # Panics
///
/// Panics unless every stride covers its row (`a_stride >= depth`,
/// `b_stride >= width`, `c_stride >= width`) and the three slices hold every
/// row the block addresses, when the block is not empty.
pub type GemmBlockFn = fn(&GemmBlock<'_>, &mut [u8], usize);

/// The kernels of one backend.
#[derive(Clone, Copy, Debug)]
pub struct Gf256RegionFns {
    /// Region multiply-add.
    pub mul_add: MulAddFn,
    /// Matrix-product block.
    pub gemm_block: GemmBlockFn,
    /// Columns one step of `gemm_block` carries across the inner dimension
    /// of a block.
    pub gemm_strip: usize,
    /// The backend, for diagnostics.
    pub name: &'static str,
}

/// The portable kernels, on every target.
#[must_use]
pub const fn portable() -> Gf256RegionFns {
    Gf256RegionFns {
        mul_add: mul_add_portable,
        gemm_block: gemm_block_portable,
        gemm_strip: 1,
        name: "portable",
    }
}

/// The fastest kernels the host supports, `None` when only the portable
/// ones apply.
#[must_use]
pub fn detect() -> Option<Gf256RegionFns> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return crate::x86::gf256_region::detect();
    }
    #[allow(unreachable_code)]
    None
}

fn mul_add_portable(table: &SplitTable, x: &[u8], y: &mut [u8]) {
    check_mul_add(x, y);
    mul_add_scalar(table, x, y);
}

fn gemm_block_portable(block: &GemmBlock<'_>, c: &mut [u8], c_stride: usize) {
    if check_gemm_block(block, c, c_stride) {
        gemm_block_scalar(block, c, c_stride, 0..block.width);
    }
}

pub(crate) fn check_mul_add(x: &[u8], y: &[u8]) {
    assert_eq!(x.len(), y.len(), "region lengths differ");
}

/// Whether the block has work, after checking that every row it addresses
/// lies inside its slice.
pub(crate) fn check_gemm_block(block: &GemmBlock<'_>, c: &[u8], c_stride: usize) -> bool {
    let GemmBlock {
        a,
        a_stride,
        b,
        b_stride,
        rows,
        depth,
        width,
        ..
    } = *block;
    if rows == 0 || depth == 0 || width == 0 {
        return false;
    }
    let span = |count: usize, stride: usize, len: usize| {
        (count - 1)
            .checked_mul(stride)
            .and_then(|start| start.checked_add(len))
            .expect("block span overflows")
    };
    assert!(
        a_stride >= depth && b_stride >= width && c_stride >= width,
        "a stride shorter than its row"
    );
    assert!(a.len() >= span(rows, a_stride, depth), "a too short");
    assert!(b.len() >= span(depth, b_stride, width), "b too short");
    assert!(c.len() >= span(rows, c_stride, width), "c too short");
    true
}

pub(crate) fn mul_add_scalar(table: &SplitTable, x: &[u8], y: &mut [u8]) {
    for (y, &x) in y.iter_mut().zip(x) {
        *y ^= table.apply(x);
    }
}

/// The block restricted to the columns in `columns`, on checked bounds.
pub(crate) fn gemm_block_scalar(
    block: &GemmBlock<'_>,
    c: &mut [u8],
    c_stride: usize,
    columns: core::ops::Range<usize>,
) {
    for r in 0..block.rows {
        let row = &mut c[r * c_stride..][columns.clone()];
        for p in 0..block.depth {
            let table = &block.tables[usize::from(block.a[r * block.a_stride + p])];
            let source = &block.b[p * block.b_stride..][columns.clone()];
            mul_add_scalar(table, source, row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(state: &mut u64) -> u8 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*state >> 56) as u8
    }

    /// Shift-and-add multiplication modulo `x^8 + low`.
    fn multiply(mut a: u8, mut b: u8, low: u8) -> u8 {
        let mut product = 0;
        while b != 0 {
            if b & 1 == 1 {
                product ^= a;
            }
            a = (a << 1) ^ if a & 0x80 != 0 { low } else { 0 };
            b >>= 1;
        }
        product
    }

    fn tables(low: u8) -> Box<[SplitTable; 256]> {
        let mut tables = Box::new(
            [SplitTable {
                low: [0; 16],
                high: [0; 16],
            }; 256],
        );
        for (c, table) in tables.iter_mut().enumerate() {
            for i in 0..16u8 {
                table.low[usize::from(i)] = multiply(c as u8, i, low);
                table.high[usize::from(i)] = multiply(c as u8, i << 4, low);
            }
        }
        tables
    }

    fn backends() -> Vec<Gf256RegionFns> {
        let mut backends = vec![portable()];
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert_eq!(
            detect().is_some(),
            std::arch::is_x86_feature_detected!("avx2")
        );
        backends.extend(detect());
        backends
    }

    const LENGTHS: [usize; 18] = [
        0, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 95, 127, 129, 1000,
    ];

    #[test]
    fn split_tables_encode_the_product() {
        for low in [0x1d, 0x1b] {
            let tables = tables(low);
            for c in 0..=255u8 {
                for v in 0..=255u8 {
                    assert_eq!(tables[usize::from(c)].apply(v), multiply(c, v, low));
                }
            }
        }
    }

    #[test]
    fn every_backend_multiplies_regions_by_the_definition() {
        let mut state = 0x6a09_e667;
        for low in [0x1d, 0x1b] {
            let tables = tables(low);
            for fns in backends() {
                for len in LENGTHS {
                    for offset in [0, 1, 5] {
                        for c in [0u8, 1, 2, 0x53, 0x80, 0xff, lcg(&mut state)] {
                            let x: Vec<u8> = (0..len + offset).map(|_| lcg(&mut state)).collect();
                            let y: Vec<u8> = (0..len + offset).map(|_| lcg(&mut state)).collect();
                            let mut got = y.clone();
                            (fns.mul_add)(
                                &tables[usize::from(c)],
                                &x[offset..],
                                &mut got[offset..],
                            );
                            let expected: Vec<u8> = (0..len + offset)
                                .map(|i| {
                                    if i < offset {
                                        y[i]
                                    } else {
                                        y[i] ^ multiply(c, x[i], low)
                                    }
                                })
                                .collect();
                            assert_eq!(got, expected, "{} len {len} offset {offset}", fns.name);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_backend_multiplies_regions_at_every_vector_phase() {
        let mut state = 0x3c6e_f372;
        let tables = tables(0x1d);
        let table = &tables[0x8e];
        for fns in backends() {
            for len in [0usize, 1, 31, 32, 33, 63, 64, 65, 159, 160, 161, 300] {
                for y_phase in 0..=32 {
                    for x_phase in [0usize, 16, 31] {
                        let x: Vec<u8> = (0..len + 64).map(|_| lcg(&mut state)).collect();
                        let y: Vec<u8> = (0..len + 64).map(|_| lcg(&mut state)).collect();
                        let mut got = y.clone();
                        // The destination window starts at each byte of a
                        // 32-byte vector, whatever the allocation's alignment.
                        let y_at = (y_phase + 32 - got.as_ptr() as usize % 32) % 32;
                        (fns.mul_add)(
                            table,
                            &x[x_phase..x_phase + len],
                            &mut got[y_at..y_at + len],
                        );
                        let mut expected = y.clone();
                        for i in 0..len {
                            expected[y_at + i] ^= multiply(0x8e, x[x_phase + i], 0x1d);
                        }
                        assert_eq!(
                            got, expected,
                            "{} len {len} y phase {y_phase} x phase {x_phase}",
                            fns.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_backend_multiplies_blocks_by_the_definition() {
        let mut state = 0xbb67_ae85;
        let tables = tables(0x1d);
        for fns in backends() {
            for rows in [1usize, 2, 3, 4, 5, 9] {
                for depth in [1usize, 2, 7, 33] {
                    for width in [1usize, 15, 31, 32, 33, 64, 97] {
                        let (a_stride, b_stride, c_stride) = (depth + 3, width + 5, width + 2);
                        let a: Vec<u8> = (0..rows * a_stride).map(|_| lcg(&mut state)).collect();
                        let b: Vec<u8> = (0..depth * b_stride).map(|_| lcg(&mut state)).collect();
                        let c: Vec<u8> = (0..rows * c_stride).map(|_| lcg(&mut state)).collect();
                        let block = GemmBlock {
                            tables: &tables,
                            a: &a,
                            a_stride,
                            b: &b,
                            b_stride,
                            rows,
                            depth,
                            width,
                        };
                        let mut got = c.clone();
                        (fns.gemm_block)(&block, &mut got, c_stride);
                        let mut expected = c.clone();
                        for r in 0..rows {
                            for j in 0..width {
                                for p in 0..depth {
                                    expected[r * c_stride + j] ^=
                                        multiply(a[r * a_stride + p], b[p * b_stride + j], 0x1d);
                                }
                            }
                        }
                        assert_eq!(
                            got, expected,
                            "{} rows {rows} depth {depth} width {width}",
                            fns.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_backend_rejects_mismatched_regions_and_short_blocks() {
        let tables = tables(0x1d);
        for fns in backends() {
            let table = tables[3];
            assert!(
                std::panic::catch_unwind(|| (fns.mul_add)(&table, &[0; 33], &mut [0; 32])).is_err()
            );
            let (a, b) = (vec![0u8; 8], vec![0u8; 64]);
            for (rows, depth, width, c_len, c_stride) in [
                (2, 4, 32, 63, 32),
                (2, 5, 32, 64, 32),
                (3, 4, 16, 64, 32),
                (1, 1, 33, 33, 32),
            ] {
                let block = GemmBlock {
                    tables: &tables,
                    a: &a,
                    a_stride: 4,
                    b: &b,
                    b_stride: 32,
                    rows,
                    depth,
                    width,
                };
                let result = std::panic::catch_unwind(|| {
                    (fns.gemm_block)(&block, &mut vec![0; c_len], c_stride)
                });
                assert!(
                    result.is_err(),
                    "{} {rows} {depth} {width} {c_len}",
                    fns.name
                );
            }
            let empty = GemmBlock {
                tables: &tables,
                a: &[],
                a_stride: 0,
                b: &[],
                b_stride: 0,
                rows: 0,
                depth: 0,
                width: 0,
            };
            (fns.gemm_block)(&empty, &mut [], 0);
        }
    }
}
