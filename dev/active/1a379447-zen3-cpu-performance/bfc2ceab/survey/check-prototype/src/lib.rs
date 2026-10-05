//! Checked-form prototypes of the caller-trusted kernel entry points of
//! `gf2-kernels-simd` (jit:bfc2ceab). For each interface `*_current` forwards
//! its arguments to the published pointer and `*_checked` runs first the
//! release-mode check a safe entry point would hold, in the shape of
//! `gf2_kernels_simd::m4rm::assert_gray_build_lengths`: a named assertion
//! with checked products. Nothing here is on a production path.

#![allow(clippy::too_many_arguments)]

use gf2_kernels_simd::bipedal::packed5::{F5BinaryKernelFn, F5UnaryKernelFn};
use gf2_kernels_simd::bipedal::packed7::{F7BinaryKernelFn, F7UnaryKernelFn};
use gf2_kernels_simd::bipedal::{BipedalBinaryKernelFn, BipedalUnaryKernelFn};
use gf2_kernels_simd::fp_medium::{MediumPrimeGemmPanelFn, MediumPrimeSpmmRowFn};
use gf2_kernels_simd::fp_medium_f64::FpMediumF64GemmFn;
use gf2_kernels_simd::fp_medium_ple::MediumPrimePlePanelBaseFn;
use gf2_kernels_simd::fp_small::{SmallPrimeGemmRowPanelFn, SmallPrimeSpmmRowFn};
use gf2_kernels_simd::fp_small_f32::SmallPrimeF32GemmFn;
use gf2_kernels_simd::fp_small_panel::SmallPrimePanelGemmFn;
use gf2_kernels_simd::fp_small_ple::SmallPrimePlePanelBaseFn;
use gf2_kernels_simd::modem::{PamSqDistancesF32Fn, PamSqDistancesF64Fn};

/// Panics unless every `a_cols[h] * b_stride + n` stays inside a `b` of
/// `b_len` elements. One division per call hoists the product out of the
/// scan, which compares each index once.
#[inline]
fn assert_spmm_row_columns(
    kernel: &str,
    a_cols: &[usize],
    b_len: usize,
    b_stride: usize,
    n: usize,
) {
    if a_cols.is_empty() {
        return;
    }
    let Some(last_start) = b_len.checked_sub(n) else {
        panic!("{kernel}: b is shorter than one output row");
    };
    // A zero stride starts every row at element 0.
    let Some(max_col) = last_start.checked_div(b_stride) else {
        return;
    };
    assert!(
        a_cols.iter().all(|&col| col <= max_col),
        "{kernel}: a column index reaches outside b"
    );
}

/// Panics unless `window_len == m * win` without overflow.
#[inline]
fn assert_ple_window(kernel: &str, window_len: usize, m: usize, win: usize) {
    let Some(cells) = m.checked_mul(win) else {
        panic!("{kernel}: m * win overflows usize");
    };
    assert!(window_len == cells, "{kernel}: window is not m * win cells");
}

/// Panics unless every slice length equals the first and is a multiple of
/// four.
#[inline]
fn assert_packed_lengths<const SLICES: usize>(kernel: &str, lengths: [usize; SLICES]) {
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

/// Panics unless `length == rows * cols` without overflow.
#[inline]
fn assert_shape(kernel: &str, operand: &str, length: usize, rows: usize, cols: usize) {
    let Some(cells) = rows.checked_mul(cols) else {
        panic!("{kernel}: shape of {operand} overflows usize");
    };
    assert!(length == cells, "{kernel}: {operand} shape");
}

#[inline(never)]
pub fn spmm_small_current(
    kernel: SmallPrimeSpmmRowFn,
    a_vals: &[u8],
    a_cols: &[usize],
    b: &[u8],
    b_stride: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    kernel(a_vals, a_cols, b, b_stride, n, p, out)
}

#[inline(never)]
pub fn spmm_small_checked(
    kernel: SmallPrimeSpmmRowFn,
    a_vals: &[u8],
    a_cols: &[usize],
    b: &[u8],
    b_stride: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    assert_spmm_row_columns("fp_small_spmm_row", a_cols, b.len(), b_stride, n);
    kernel(a_vals, a_cols, b, b_stride, n, p, out)
}

#[inline(never)]
pub fn spmm_medium_current(
    kernel: MediumPrimeSpmmRowFn,
    a_vals: &[u16],
    a_cols: &[usize],
    b: &[u16],
    b_stride: usize,
    n: usize,
    p: u16,
    out: &mut [u16],
) {
    kernel(a_vals, a_cols, b, b_stride, n, p, out)
}

#[inline(never)]
pub fn spmm_medium_checked(
    kernel: MediumPrimeSpmmRowFn,
    a_vals: &[u16],
    a_cols: &[usize],
    b: &[u16],
    b_stride: usize,
    n: usize,
    p: u16,
    out: &mut [u16],
) {
    assert_spmm_row_columns("fp_medium_spmm_row", a_cols, b.len(), b_stride, n);
    kernel(a_vals, a_cols, b, b_stride, n, p, out)
}

#[inline(never)]
pub fn ple_small_current(
    kernel: SmallPrimePlePanelBaseFn,
    window: &mut [u8],
    m: usize,
    win: usize,
    p: u8,
    inv_table: &[u8],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    kernel(window, m, win, p, inv_table, row_perm, pivot_cols_local)
}

/// The shape check alone; the inverse-table index is checked where the kernel
/// reads it ([`pivot_inverse_u8_checked`]).
#[inline(never)]
pub fn ple_small_checked(
    kernel: SmallPrimePlePanelBaseFn,
    window: &mut [u8],
    m: usize,
    win: usize,
    p: u8,
    inv_table: &[u8],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    assert_ple_window("ple_panel_base_canonical", window.len(), m, win);
    kernel(window, m, win, p, inv_table, row_perm, pivot_cols_local)
}

/// The shape check with a scan of every window entry against the inverse
/// table: the form a check takes when it sits wholly in the safe entry point.
#[inline(never)]
pub fn ple_small_scan_checked(
    kernel: SmallPrimePlePanelBaseFn,
    window: &mut [u8],
    m: usize,
    win: usize,
    p: u8,
    inv_table: &[u8],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    assert_ple_window("ple_panel_base_canonical", window.len(), m, win);
    let entries = inv_table.len();
    assert!(
        window.iter().all(|&cell| usize::from(cell) < entries),
        "ple_panel_base_canonical: a window entry indexes outside inv_table"
    );
    kernel(window, m, win, p, inv_table, row_perm, pivot_cols_local)
}

#[inline(never)]
pub fn ple_medium_current(
    kernel: MediumPrimePlePanelBaseFn,
    window: &mut [u16],
    m: usize,
    win: usize,
    p: u16,
    inv_table: &[u16],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    kernel(window, m, win, p, inv_table, row_perm, pivot_cols_local)
}

#[inline(never)]
pub fn ple_medium_checked(
    kernel: MediumPrimePlePanelBaseFn,
    window: &mut [u16],
    m: usize,
    win: usize,
    p: u16,
    inv_table: &[u16],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    assert_ple_window("ple_panel_base_canonical_u16", window.len(), m, win);
    kernel(window, m, win, p, inv_table, row_perm, pivot_cols_local)
}

#[inline(never)]
pub fn ple_medium_scan_checked(
    kernel: MediumPrimePlePanelBaseFn,
    window: &mut [u16],
    m: usize,
    win: usize,
    p: u16,
    inv_table: &[u16],
    row_perm: &mut [usize],
    pivot_cols_local: &mut Vec<usize>,
) -> usize {
    assert_ple_window("ple_panel_base_canonical_u16", window.len(), m, win);
    let entries = inv_table.len();
    assert!(
        window.iter().all(|&cell| usize::from(cell) < entries),
        "ple_panel_base_canonical_u16: a window entry indexes outside inv_table"
    );
    kernel(window, m, win, p, inv_table, row_perm, pivot_cols_local)
}

/// The kernel's pivot-inverse read as it stands.
///
/// # Safety
///
/// `pivot` indexes inside `inv_table`.
#[inline(never)]
pub unsafe fn pivot_inverse_u8_current(inv_table: &[u8], pivot: u8) -> u8 {
    // SAFETY: the caller keeps `pivot` inside `inv_table`.
    unsafe { *inv_table.get_unchecked(usize::from(pivot)) }
}

/// The same read with the index checked, once per pivot.
#[inline(never)]
pub fn pivot_inverse_u8_checked(inv_table: &[u8], pivot: u8) -> u8 {
    inv_table[usize::from(pivot)]
}

/// As [`pivot_inverse_u8_current`], for the u16 lane.
///
/// # Safety
///
/// `pivot` indexes inside `inv_table`.
#[inline(never)]
pub unsafe fn pivot_inverse_u16_current(inv_table: &[u16], pivot: u16) -> u16 {
    // SAFETY: the caller keeps `pivot` inside `inv_table`.
    unsafe { *inv_table.get_unchecked(usize::from(pivot)) }
}

#[inline(never)]
pub fn pivot_inverse_u16_checked(inv_table: &[u16], pivot: u16) -> u16 {
    inv_table[usize::from(pivot)]
}

#[inline(never)]
pub fn bipedal_binary_current(
    kernel: BipedalBinaryKernelFn,
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    kernel(mag1, sgn1, mag2, sgn2, out_mag, out_sgn)
}

#[inline(never)]
pub fn bipedal_binary_checked(
    kernel: BipedalBinaryKernelFn,
    mag1: &[u64],
    sgn1: &[u64],
    mag2: &[u64],
    sgn2: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    assert_packed_lengths(
        "bipedal binary kernel",
        [
            mag1.len(),
            sgn1.len(),
            mag2.len(),
            sgn2.len(),
            out_mag.len(),
            out_sgn.len(),
        ],
    );
    kernel(mag1, sgn1, mag2, sgn2, out_mag, out_sgn)
}

#[inline(never)]
pub fn bipedal_unary_current(
    kernel: BipedalUnaryKernelFn,
    mag: &[u64],
    sgn: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    kernel(mag, sgn, out_mag, out_sgn)
}

#[inline(never)]
pub fn bipedal_unary_checked(
    kernel: BipedalUnaryKernelFn,
    mag: &[u64],
    sgn: &[u64],
    out_mag: &mut [u64],
    out_sgn: &mut [u64],
) {
    assert_packed_lengths(
        "bipedal unary kernel",
        [mag.len(), sgn.len(), out_mag.len(), out_sgn.len()],
    );
    kernel(mag, sgn, out_mag, out_sgn)
}

#[inline(never)]
pub fn f5_binary_current(
    kernel: F5BinaryKernelFn,
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    kernel(b0a, b1a, b2a, b0b, b1b, b2b, out_b0, out_b1, out_b2)
}

#[inline(never)]
pub fn f5_binary_checked(
    kernel: F5BinaryKernelFn,
    b0a: &[u64],
    b1a: &[u64],
    b2a: &[u64],
    b0b: &[u64],
    b1b: &[u64],
    b2b: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    assert_packed_lengths(
        "F_5 binary kernel",
        [
            b0a.len(),
            b1a.len(),
            b2a.len(),
            b0b.len(),
            b1b.len(),
            b2b.len(),
            out_b0.len(),
            out_b1.len(),
            out_b2.len(),
        ],
    );
    kernel(b0a, b1a, b2a, b0b, b1b, b2b, out_b0, out_b1, out_b2)
}

#[inline(never)]
pub fn f5_unary_current(
    kernel: F5UnaryKernelFn,
    b0: &[u64],
    b1: &[u64],
    b2: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    kernel(b0, b1, b2, out_b0, out_b1, out_b2)
}

#[inline(never)]
pub fn f5_unary_checked(
    kernel: F5UnaryKernelFn,
    b0: &[u64],
    b1: &[u64],
    b2: &[u64],
    out_b0: &mut [u64],
    out_b1: &mut [u64],
    out_b2: &mut [u64],
) {
    assert_packed_lengths(
        "F_5 unary kernel",
        [
            b0.len(),
            b1.len(),
            b2.len(),
            out_b0.len(),
            out_b1.len(),
            out_b2.len(),
        ],
    );
    kernel(b0, b1, b2, out_b0, out_b1, out_b2)
}

#[inline(never)]
pub fn f7_binary_current(kernel: F7BinaryKernelFn, a: &[u64], b: &[u64], out: &mut [u64]) {
    kernel(a, b, out)
}

#[inline(never)]
pub fn f7_binary_checked(kernel: F7BinaryKernelFn, a: &[u64], b: &[u64], out: &mut [u64]) {
    assert_packed_lengths("F_7 binary kernel", [a.len(), b.len(), out.len()]);
    kernel(a, b, out)
}

#[inline(never)]
pub fn f7_unary_current(kernel: F7UnaryKernelFn, a: &[u64], out: &mut [u64]) {
    kernel(a, out)
}

#[inline(never)]
pub fn f7_unary_checked(kernel: F7UnaryKernelFn, a: &[u64], out: &mut [u64]) {
    assert_packed_lengths("F_7 unary kernel", [a.len(), out.len()]);
    kernel(a, out)
}

#[inline(never)]
pub fn gemm_row_panel_current(
    kernel: SmallPrimeGemmRowPanelFn,
    a: &[u8],
    bt: &[u8],
    k: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    kernel(a, bt, k, n, p, out)
}

#[inline(never)]
pub fn gemm_row_panel_checked(
    kernel: SmallPrimeGemmRowPanelFn,
    a: &[u8],
    bt: &[u8],
    k: usize,
    n: usize,
    p: u8,
    out: &mut [u8],
) {
    assert_shape("fp_small_gemm_row_panel", "bt", bt.len(), n, k);
    kernel(a, bt, k, n, p, out)
}

macro_rules! whole_gemm_prototype {
    ($current:ident, $checked:ident, $kernel:ty, $name:literal, $input:ty, $modulus:ty, $output:ty) => {
        #[inline(never)]
        pub fn $current(
            kernel: $kernel,
            a: &[$input],
            bt: &[$input],
            m: usize,
            k: usize,
            n: usize,
            p: $modulus,
            c: &mut [$output],
        ) {
            kernel(a, bt, m, k, n, p, c)
        }

        #[inline(never)]
        pub fn $checked(
            kernel: $kernel,
            a: &[$input],
            bt: &[$input],
            m: usize,
            k: usize,
            n: usize,
            p: $modulus,
            c: &mut [$output],
        ) {
            assert_shape($name, "a", a.len(), m, k);
            assert_shape($name, "bt", bt.len(), n, k);
            assert_shape($name, "c", c.len(), m, n);
            kernel(a, bt, m, k, n, p, c)
        }
    };
}

whole_gemm_prototype!(
    small_panel_gemm_current,
    small_panel_gemm_checked,
    SmallPrimePanelGemmFn,
    "fp_small_panel_gemm",
    u8,
    u8,
    u8
);
whole_gemm_prototype!(
    small_f32_gemm_current,
    small_f32_gemm_checked,
    SmallPrimeF32GemmFn,
    "fp_small_f32_gemm",
    f32,
    u8,
    u8
);
whole_gemm_prototype!(
    medium_panel_gemm_current,
    medium_panel_gemm_checked,
    MediumPrimeGemmPanelFn,
    "fp_medium_gemm_panel",
    u16,
    u16,
    u16
);
whole_gemm_prototype!(
    medium_f64_gemm_current,
    medium_f64_gemm_checked,
    FpMediumF64GemmFn,
    "fp_medium_f64_gemm",
    f64,
    u16,
    u16
);

macro_rules! pam_prototype {
    ($current:ident, $checked:ident, $kernel:ty, $name:literal, $sample:ty) => {
        #[inline(never)]
        pub fn $current(
            kernel: $kernel,
            z: &[$sample],
            g: &[$sample],
            inv_n0_eq: &[$sample],
            pam_levels: &[$sample],
            out: &mut [$sample],
        ) {
            kernel(z, g, inv_n0_eq, pam_levels, out)
        }

        #[inline(never)]
        pub fn $checked(
            kernel: $kernel,
            z: &[$sample],
            g: &[$sample],
            inv_n0_eq: &[$sample],
            pam_levels: &[$sample],
            out: &mut [$sample],
        ) {
            assert_shape($name, "out", out.len(), z.len(), pam_levels.len());
            kernel(z, g, inv_n0_eq, pam_levels, out)
        }
    };
}

pam_prototype!(
    pam_f32_current,
    pam_f32_checked,
    PamSqDistancesF32Fn,
    "pam_sq_distances_f32_avx2",
    f32
);
pam_prototype!(
    pam_f64_current,
    pam_f64_checked,
    PamSqDistancesF64Fn,
    "pam_sq_distances_f64_avx2",
    f64
);
