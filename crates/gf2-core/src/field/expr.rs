//! Expression-template proxy algebra over [`FieldMatrix<F>`]: operators on
//! matrix references build lazy proxies that [`Evaluate`] writes with one
//! kernel call per fused pattern, as in `(&a * &b + &c).into()`. Converting
//! a subexpression to a [`FieldMatrix<F>`] evaluates it and ends the fusion.

use std::cell::Cell;
use std::ops::{Add, Mul, Neg, Sub};

#[cfg(any(test, feature = "test-support"))]
use crate::field::matrix::GemmTilePair;
use crate::field::matrix::{
    FieldMatrix, GemmTileSite, ObservationPolicy, RecordObservations, Transposed, GEMM_COL_TILE,
    GEMM_ROW_TILE,
};
use crate::field::vec::dot_product_slices;
use crate::field::{ConstField, FieldVec, FiniteField};
use crate::matrix_like::MatrixLike;

/// Snapshot of kernel-call counts across all evaluator paths, read by
/// [`kernel_counts`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KernelCounts {
    /// Calls to the plain `A·B` kernel.
    pub gemm: u64,
    /// Calls to the fused `A·B + β·C` kernel.
    pub gemm_with_beta: u64,
    /// Calls to the fused `Aᵀ·B` kernel.
    pub gemm_trans_a: u64,
    /// Calls to the fused `α·Aᵀ·B + β·C` kernel.
    pub gemm_trans_a_with_beta: u64,
    /// Calls to the fused `α·A + β·B` axpy-linear kernel.
    pub axpy_linear: u64,
    /// Calls to the scalar-scale kernel.
    pub scale_into: u64,
    /// Calls to the element-wise negation kernel.
    pub neg_into: u64,
    /// Calls to the plain copy kernel (includes materialising [`Transposed`]).
    pub copy_into: u64,
}

// Thread-local: `bump` fires on the calling thread at the kernel dispatch
// entry, so a `reset -> op -> kernel_counts` sequence on one thread is exact.
thread_local! {
    static KC_GEMM: Cell<u64> = const { Cell::new(0) };
    static KC_GEMM_BETA: Cell<u64> = const { Cell::new(0) };
    static KC_GEMM_TA: Cell<u64> = const { Cell::new(0) };
    static KC_GEMM_TA_BETA: Cell<u64> = const { Cell::new(0) };
    static KC_AXPY: Cell<u64> = const { Cell::new(0) };
    static KC_SCALE: Cell<u64> = const { Cell::new(0) };
    static KC_NEG: Cell<u64> = const { Cell::new(0) };
    static KC_COPY: Cell<u64> = const { Cell::new(0) };
}

#[inline(always)]
fn bump(key: &'static std::thread::LocalKey<Cell<u64>>) {
    key.with(|c| c.set(c.get() + 1));
}

/// Returns the kernel-call counters of the current thread since its last
/// [`reset_kernel_counts`].
///
/// Each kernel increments its counter on the calling thread at dispatch
/// entry, so a `reset_kernel_counts -> op -> kernel_counts` sequence on one
/// thread is exact while other threads run kernels.
///
/// # Examples
///
/// ```
/// use gf2_core::field::expr::{kernel_counts, reset_kernel_counts};
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// reset_kernel_counts();
/// let a = FieldMatrix::<Fp<7>>::identity(3);
/// let b = FieldMatrix::<Fp<7>>::identity(3);
/// let c = FieldMatrix::<Fp<7>>::identity(3);
/// let before = kernel_counts();
/// let _r: FieldMatrix<Fp<7>> = (&a * &b + &c).into();
/// let after = kernel_counts();
/// // Exactly one fused gemm_with_beta, no plain gemm, no axpy.
/// assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
/// assert_eq!(after.gemm - before.gemm, 0);
/// assert_eq!(after.axpy_linear - before.axpy_linear, 0);
/// ```
pub fn kernel_counts() -> KernelCounts {
    KernelCounts {
        gemm: KC_GEMM.with(Cell::get),
        gemm_with_beta: KC_GEMM_BETA.with(Cell::get),
        gemm_trans_a: KC_GEMM_TA.with(Cell::get),
        gemm_trans_a_with_beta: KC_GEMM_TA_BETA.with(Cell::get),
        axpy_linear: KC_AXPY.with(Cell::get),
        scale_into: KC_SCALE.with(Cell::get),
        neg_into: KC_NEG.with(Cell::get),
        copy_into: KC_COPY.with(Cell::get),
    }
}

/// Resets the current thread's kernel-call counters to zero.
pub fn reset_kernel_counts() {
    KC_GEMM.with(|c| c.set(0));
    KC_GEMM_BETA.with(|c| c.set(0));
    KC_GEMM_TA.with(|c| c.set(0));
    KC_GEMM_TA_BETA.with(|c| c.set(0));
    KC_AXPY.with(|c| c.set(0));
    KC_SCALE.with(|c| c.set(0));
    KC_NEG.with(|c| c.set(0));
    KC_COPY.with(|c| c.set(0));
}

/// Consumer side of the expression-template algebra, implemented by every
/// lazy proxy in this module and by
/// [`Transposed<&FieldMatrix<F>>`](crate::field::matrix::Transposed), but
/// not by `&FieldMatrix<F>` or by `FieldMatrix<F>`, whose impl would make the
/// `From<E: Evaluate<F>> for FieldMatrix<F>` bridge overlap the reflexive
/// `impl<T> From<T> for T`.
///
/// `evaluate_into` overwrites `out`. Implementors must assert
/// `out.shape() == self.shape()`.
pub trait Evaluate<F: FiniteField> {
    /// Consumes the expression and writes its value into `out`.
    ///
    /// # Panics
    ///
    /// Panics if `out.shape() != self.shape()`.
    fn evaluate_into(self, out: &mut FieldMatrix<F>);

    /// Logical shape of the expression, in rows × cols.
    fn shape(&self) -> (usize, usize);
}

/// Kernel `out <- A`. Overwrites.
fn copy_into<F: FiniteField, LA: MatrixLike<F>>(a: &LA, out: &mut FieldMatrix<F>) {
    bump(&KC_COPY);
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "copy_into: shape mismatch ({}×{} -> {}×{})",
        a.rows(),
        a.cols(),
        <FieldMatrix<F> as MatrixLike<F>>::rows(out),
        <FieldMatrix<F> as MatrixLike<F>>::cols(out),
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            out.set(r, c, a.get(r, c));
        }
    }
}

/// Kernel `out <- -A`. Overwrites.
fn neg_into<F: FiniteField, LA: MatrixLike<F>>(a: &LA, out: &mut FieldMatrix<F>) {
    bump(&KC_NEG);
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "neg_into: shape mismatch ({}×{} -> {}×{})",
        a.rows(),
        a.cols(),
        <FieldMatrix<F> as MatrixLike<F>>::rows(out),
        <FieldMatrix<F> as MatrixLike<F>>::cols(out),
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            out.set(r, c, -a.get(r, c));
        }
    }
}

/// Kernel `out <- α · A`. Overwrites.
fn scale_into<F: FiniteField, LA: MatrixLike<F>>(alpha: F, a: &LA, out: &mut FieldMatrix<F>) {
    bump(&KC_SCALE);
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "scale_into: shape mismatch ({}×{} -> {}×{})",
        a.rows(),
        a.cols(),
        <FieldMatrix<F> as MatrixLike<F>>::rows(out),
        <FieldMatrix<F> as MatrixLike<F>>::cols(out),
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            out.set(r, c, alpha.clone() * a.get(r, c));
        }
    }
}

/// Kernel `out <- α · A + β · B` in a single pass. Overwrites.
fn axpy_linear<F, LA, LB>(alpha: F, a: &LA, beta: F, b: &LB, out: &mut FieldMatrix<F>)
where
    F: FiniteField,
    LA: MatrixLike<F>,
    LB: MatrixLike<F>,
{
    bump(&KC_AXPY);
    assert_eq!(
        a.shape(),
        b.shape(),
        "axpy_linear: operand shape mismatch ({}×{} vs {}×{})",
        a.rows(),
        a.cols(),
        b.rows(),
        b.cols()
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        a.shape(),
        "axpy_linear: output shape mismatch"
    );
    for r in 0..a.rows() {
        for c in 0..a.cols() {
            let v = alpha.clone() * a.get(r, c) + beta.clone() * b.get(r, c);
            out.set(r, c, v);
        }
    }
}

/// Kernel `out <- A · B` over generic `MatrixLike` operands;
/// [`gemm_concrete`] handles concrete ones.
fn gemm_matrixlike<F, LA, LB>(a: &LA, b: &LB, out: &mut FieldMatrix<F>)
where
    F: FiniteField,
    LA: MatrixLike<F>,
    LB: MatrixLike<F>,
{
    bump(&KC_GEMM);
    let (m, k1) = a.shape();
    let (k2, n) = b.shape();
    assert_eq!(
        k1, k2,
        "gemm_matrixlike: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_matrixlike: output shape mismatch"
    );
    if m == 0 || n == 0 || k1 == 0 {
        return;
    }
    let zero = a.get(0, 0).zero_like();
    for i in 0..m {
        for j in 0..n {
            let mut acc = zero.zero_like();
            for t in 0..k1 {
                acc += a.get(i, t) * b.get(t, j);
            }
            out.set(i, j, acc);
        }
    }
}

/// Kernel `out <- A · B` for concrete operands, through the blocked gemm.
fn gemm_concrete<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>, out: &mut FieldMatrix<F>) {
    bump(&KC_GEMM);
    let (m, k1) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (k2, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k1, k2,
        "gemm_concrete: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_concrete: output shape mismatch"
    );
    if m == 0 || n == 0 {
        return;
    }
    // `gemm` returns a fresh matrix; the O(m·n) move into `out` is
    // dominated by the O(m·k·n) multiplies.
    let prod = crate::field::matrix::gemm(a, b);
    for r in 0..m {
        for c in 0..n {
            out.set(r, c, <FieldMatrix<F> as MatrixLike<F>>::get(&prod, r, c));
        }
    }
}

/// Kernel `out <- A · B + β · C`. Overwrites.
///
/// Transposes `B` once and walks output tiles of
/// `GEMM_ROW_TILE × GEMM_COL_TILE`; each cell is one `dot_product_slices`
/// call with the `β · C[i, j]` term folded into the same write.
///
/// # Complexity
///
/// O(m · k · n) field multiplies.
fn gemm_with_beta_concrete<F: FiniteField, LC: MatrixLike<F>>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &LC,
    out: &mut FieldMatrix<F>,
) {
    gemm_with_beta_concrete_tiled::<F, LC, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(
        a, b, beta, c, out,
    );
}

fn gemm_with_beta_concrete_tiled<
    F: FiniteField,
    LC: MatrixLike<F>,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &LC,
    out: &mut FieldMatrix<F>,
) {
    bump(&KC_GEMM_BETA);
    let (m, k) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (bk, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k, bk,
        "gemm_with_beta: inner dimensions must match ({} vs {})",
        k, bk
    );
    assert_eq!(
        c.shape(),
        (m, n),
        "gemm_with_beta: C shape must equal A·B shape ({}×{} vs {}×{})",
        c.rows(),
        c.cols(),
        m,
        n,
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_with_beta: output shape mismatch"
    );
    if m == 0 || n == 0 {
        return;
    }
    if k == 0 {
        // Zero inner dim: A·B is the zero matrix, so out <- β·C.
        for r in 0..m {
            for col in 0..n {
                out.set(r, col, beta.clone() * c.get(r, col));
            }
        }
        return;
    }
    let zero: F = <FieldMatrix<F> as MatrixLike<F>>::get(a, 0, 0).zero_like();
    let b_t = b.transpose();
    let out_cols = n;
    for i_blk in (0..m).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(m);
        for j_blk in (0..n).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(n);
            for i in i_blk..i_end {
                let a_row = &a.as_data_slice()[i * k..(i + 1) * k];
                let out_row = &mut out.as_data_mut_slice()[i * out_cols..(i + 1) * out_cols];
                for (j, out_cell) in out_row.iter_mut().enumerate().take(j_end).skip(j_blk) {
                    let b_col = &b_t.as_data_slice()[j * k..(j + 1) * k];
                    debug_assert_eq!(a_row.len(), b_col.len());
                    let prod = dot_product_slices(a_row, b_col, &zero);
                    *out_cell = prod + beta.clone() * c.get(i, j);
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::ExprGemmWithBeta, ROW_TILE, COL_TILE);
}

/// Kernel `out <- Aᵀ · B`. Overwrites.
///
/// Transposes `A` (k×m → m×k) and `B` (k×n → n×k) once; each cell is one
/// `dot_product_slices` call over contiguous rows of `A_t` and `B_t`.
fn gemm_trans_a_concrete<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    gemm_trans_a_concrete_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(a, b, out);
}

fn gemm_trans_a_concrete_tiled<
    F: FiniteField,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    bump(&KC_GEMM_TA);
    let (k1, m) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (k2, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k1, k2,
        "gemm_trans_a: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_trans_a: output shape mismatch"
    );
    if m == 0 || n == 0 || k1 == 0 {
        return;
    }
    let zero: F = <FieldMatrix<F> as MatrixLike<F>>::get(a, 0, 0).zero_like();
    let a_t = a.transpose();
    let b_t = b.transpose();
    let out_cols = n;
    for i_blk in (0..m).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(m);
        for j_blk in (0..n).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(n);
            for i in i_blk..i_end {
                let a_row = &a_t.as_data_slice()[i * k1..(i + 1) * k1];
                let out_row = &mut out.as_data_mut_slice()[i * out_cols..(i + 1) * out_cols];
                for (j, out_cell) in out_row.iter_mut().enumerate().take(j_end).skip(j_blk) {
                    let b_col = &b_t.as_data_slice()[j * k1..(j + 1) * k1];
                    debug_assert_eq!(a_row.len(), b_col.len());
                    *out_cell = dot_product_slices(a_row, b_col, &zero);
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::ExprGemmTransA, ROW_TILE, COL_TILE);
}

/// Kernel `out <- α · Aᵀ · B + β · C`. Overwrites.
fn gemm_trans_a_with_beta_concrete<F: FiniteField, LC: MatrixLike<F>>(
    alpha: F,
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &LC,
    out: &mut FieldMatrix<F>,
) {
    gemm_trans_a_with_beta_concrete_tiled::<F, LC, GEMM_ROW_TILE, GEMM_COL_TILE, RecordObservations>(
        alpha, a, b, beta, c, out,
    );
}

fn gemm_trans_a_with_beta_concrete_tiled<
    F: FiniteField,
    LC: MatrixLike<F>,
    const ROW_TILE: usize,
    const COL_TILE: usize,
    O: ObservationPolicy,
>(
    alpha: F,
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &LC,
    out: &mut FieldMatrix<F>,
) {
    bump(&KC_GEMM_TA_BETA);
    let (k1, m) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(a),
        <FieldMatrix<F> as MatrixLike<F>>::cols(a),
    );
    let (k2, n) = (
        <FieldMatrix<F> as MatrixLike<F>>::rows(b),
        <FieldMatrix<F> as MatrixLike<F>>::cols(b),
    );
    assert_eq!(
        k1, k2,
        "gemm_trans_a_with_beta: inner dimensions must match ({} vs {})",
        k1, k2
    );
    assert_eq!(
        c.shape(),
        (m, n),
        "gemm_trans_a_with_beta: C shape must equal Aᵀ·B shape"
    );
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(out),
        (m, n),
        "gemm_trans_a_with_beta: output shape mismatch"
    );
    if m == 0 || n == 0 {
        return;
    }
    if k1 == 0 {
        for r in 0..m {
            for col in 0..n {
                out.set(r, col, beta.clone() * c.get(r, col));
            }
        }
        return;
    }
    let zero: F = <FieldMatrix<F> as MatrixLike<F>>::get(a, 0, 0).zero_like();
    let a_t = a.transpose();
    let b_t = b.transpose();
    let out_cols = n;
    for i_blk in (0..m).step_by(ROW_TILE) {
        let i_end = (i_blk + ROW_TILE).min(m);
        for j_blk in (0..n).step_by(COL_TILE) {
            let j_end = (j_blk + COL_TILE).min(n);
            for i in i_blk..i_end {
                let a_row = &a_t.as_data_slice()[i * k1..(i + 1) * k1];
                let out_row = &mut out.as_data_mut_slice()[i * out_cols..(i + 1) * out_cols];
                for (j, out_cell) in out_row.iter_mut().enumerate().take(j_end).skip(j_blk) {
                    let b_col = &b_t.as_data_slice()[j * k1..(j + 1) * k1];
                    debug_assert_eq!(a_row.len(), b_col.len());
                    let prod = dot_product_slices(a_row, b_col, &zero);
                    *out_cell = alpha.clone() * prod + beta.clone() * c.get(i, j);
                }
            }
        }
    }
    O::gemm_tiles(GemmTileSite::ExprGemmTransAWithBeta, ROW_TILE, COL_TILE);
}

/// Function-pointer type for a fused expression GEMM tile candidate.
#[cfg(any(test, feature = "test-support"))]
pub type ExprGemmBetaCandidateFn<F> =
    fn(&FieldMatrix<F>, &FieldMatrix<F>, F, &FieldMatrix<F>, &mut FieldMatrix<F>);

/// Function-pointer type for a transposed-left expression GEMM candidate.
#[cfg(any(test, feature = "test-support"))]
pub type ExprGemmTransCandidateFn<F> = fn(&FieldMatrix<F>, &FieldMatrix<F>, &mut FieldMatrix<F>);

/// Function-pointer type for a fused transposed-left GEMM candidate.
#[cfg(any(test, feature = "test-support"))]
pub type ExprGemmTransBetaCandidateFn<F> =
    fn(F, &FieldMatrix<F>, &FieldMatrix<F>, F, &FieldMatrix<F>, &mut FieldMatrix<F>);

#[cfg(any(test, feature = "test-support"))]
fn expr_gemm_beta_candidate<F: FiniteField, const ROW_TILE: usize, const COL_TILE: usize>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    gemm_with_beta_concrete_tiled::<F, _, ROW_TILE, COL_TILE, RecordObservations>(
        a, b, beta, c, out,
    );
}

#[cfg(any(test, feature = "test-support"))]
fn expr_gemm_trans_candidate<F: FiniteField, const ROW_TILE: usize, const COL_TILE: usize>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    gemm_trans_a_concrete_tiled::<F, ROW_TILE, COL_TILE, RecordObservations>(a, b, out);
}

#[cfg(any(test, feature = "test-support"))]
fn expr_gemm_trans_beta_candidate<F: FiniteField, const ROW_TILE: usize, const COL_TILE: usize>(
    alpha: F,
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    beta: F,
    c: &FieldMatrix<F>,
    out: &mut FieldMatrix<F>,
) {
    gemm_trans_a_with_beta_concrete_tiled::<F, _, ROW_TILE, COL_TILE, RecordObservations>(
        alpha, a, b, beta, c, out,
    );
}

#[cfg(any(test, feature = "test-support"))]
impl GemmTilePair {
    /// Resolves this pair to the shared fused expression GEMM body.
    pub fn expr_gemm_beta_fn<F: FiniteField>(self) -> ExprGemmBetaCandidateFn<F> {
        match self {
            Self::R16C32 => expr_gemm_beta_candidate::<F, 16, 32>,
            Self::R16C64 => expr_gemm_beta_candidate::<F, 16, 64>,
            Self::R16C128 => expr_gemm_beta_candidate::<F, 16, 128>,
            Self::R32C32 => expr_gemm_beta_candidate::<F, 32, 32>,
            Self::R32C64 => expr_gemm_beta_candidate::<F, 32, 64>,
            Self::R32C128 => expr_gemm_beta_candidate::<F, 32, 128>,
            Self::R64C32 => expr_gemm_beta_candidate::<F, 64, 32>,
            Self::R64C64 => expr_gemm_beta_candidate::<F, 64, 64>,
            Self::R64C128 => expr_gemm_beta_candidate::<F, 64, 128>,
        }
    }

    /// Resolves this pair to the shared transposed-left GEMM body.
    pub fn expr_gemm_trans_fn<F: FiniteField>(self) -> ExprGemmTransCandidateFn<F> {
        match self {
            Self::R16C32 => expr_gemm_trans_candidate::<F, 16, 32>,
            Self::R16C64 => expr_gemm_trans_candidate::<F, 16, 64>,
            Self::R16C128 => expr_gemm_trans_candidate::<F, 16, 128>,
            Self::R32C32 => expr_gemm_trans_candidate::<F, 32, 32>,
            Self::R32C64 => expr_gemm_trans_candidate::<F, 32, 64>,
            Self::R32C128 => expr_gemm_trans_candidate::<F, 32, 128>,
            Self::R64C32 => expr_gemm_trans_candidate::<F, 64, 32>,
            Self::R64C64 => expr_gemm_trans_candidate::<F, 64, 64>,
            Self::R64C128 => expr_gemm_trans_candidate::<F, 64, 128>,
        }
    }

    /// Resolves this pair to the shared fused transposed-left GEMM body.
    pub fn expr_gemm_trans_beta_fn<F: FiniteField>(self) -> ExprGemmTransBetaCandidateFn<F> {
        match self {
            Self::R16C32 => expr_gemm_trans_beta_candidate::<F, 16, 32>,
            Self::R16C64 => expr_gemm_trans_beta_candidate::<F, 16, 64>,
            Self::R16C128 => expr_gemm_trans_beta_candidate::<F, 16, 128>,
            Self::R32C32 => expr_gemm_trans_beta_candidate::<F, 32, 32>,
            Self::R32C64 => expr_gemm_trans_beta_candidate::<F, 32, 64>,
            Self::R32C128 => expr_gemm_trans_beta_candidate::<F, 32, 128>,
            Self::R64C32 => expr_gemm_trans_beta_candidate::<F, 64, 32>,
            Self::R64C64 => expr_gemm_trans_beta_candidate::<F, 64, 64>,
            Self::R64C128 => expr_gemm_trans_beta_candidate::<F, 64, 128>,
        }
    }
}

/// Deferred matrix multiplication `A · B`.
///
/// # Panics
///
/// The `Mul` impl panics at construction if `a.cols() != b.rows()`.
/// `evaluate_into` panics if `out.shape() != (a.rows(), b.cols())`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "Product is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct Product<A, B>(pub A, pub B);

/// Deferred element-wise addition `A + B`.
///
/// # Panics
///
/// Construction panics if `a.shape() != b.shape()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "Sum is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct Sum<A, B>(pub A, pub B);

/// Deferred scalar-times-matrix `α · M`.
///
/// Built by `alpha * &a` or `&a * alpha`; a scaled [`Product`] is also a
/// `Scale`.
///
/// # Panics
///
/// Evaluation asserts `out.shape() == self.shape()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "Scale is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct Scale<F: FiniteField, M>(pub F, pub M);

/// Deferred element-wise negation `-M`.
///
/// Named `NegProxy` rather than `Neg` to avoid colliding with
/// [`std::ops::Neg`]. Built by `-&a` or `-a`.
///
/// # Panics
///
/// Evaluation asserts `out.shape() == self.shape()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "NegProxy is a lazy expression; call `.into()` or bind without an annotation to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct NegProxy<M>(pub M);

/// Canonical fusion: `A · B + C` (β = 1).
///
/// Built by `Product<A, B> + &c` (and the commuted form). Evaluates in a
/// single `gemm_with_beta` kernel call.
///
/// # Panics
///
/// Construction panics if the product shape differs from the addend shape.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "FusedProductPlus is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct FusedProductPlus<P, C>(pub P, pub C);

/// Canonical fusion: `A · B + β · C`.
///
/// Built by `Product<A, B> + Scale<F, &c>` (and commutations).
///
/// # Panics
///
/// Construction panics if the product shape differs from the addend shape.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "FusedProductPlusScaled is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct FusedProductPlusScaled<P, SS>(pub P, pub SS);

/// Canonical fusion: `α · A + β · B`.
///
/// Built by `Scale<F, A> + Scale<F, B>`.
///
/// # Panics
///
/// Construction panics on shape mismatch.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·n).
#[must_use = "FusedLinear is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct FusedLinear<A, B>(pub A, pub B);

/// Canonical fusion: `Aᵀ · B`.
///
/// Built by `a.t() * &b`. Evaluates in a single `gemm_trans_a` kernel call.
///
/// # Panics
///
/// Construction panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "TransposedProduct is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct TransposedProduct<A, B>(pub A, pub B);

/// Canonical fusion: `α · Aᵀ · B`.
///
/// Built by `(alpha * a.t()) * &b`. A distinct proxy rather than
/// `Scale<F, TransposedProduct<A, B>>`, so that the
/// `Scale<F, X> + Scale<F, Y> → FusedLinear` add impl does not match it.
///
/// # Panics
///
/// Construction panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// Construction is O(1). Evaluation is O(m·k·n).
#[must_use = "ScaledTransposedProduct is a lazy expression; call `.into()` to materialise"]
#[derive(Debug, Clone, Copy)]
pub struct ScaledTransposedProduct<F: FiniteField, A, B>(pub F, pub A, pub B);

impl<A, B> Product<A, B> {
    /// Constructs the proxy after checking that inner dimensions match.
    ///
    /// # Panics
    ///
    /// Panics if `a.cols() != b.rows()` with the standard
    /// `FieldMatrix::mul: inner dimensions must match` message.
    pub fn new<F>(a: A, b: B) -> Self
    where
        F: FiniteField,
        A: MatrixLike<F>,
        B: MatrixLike<F>,
    {
        assert_eq!(
            a.cols(),
            b.rows(),
            "FieldMatrix::mul: inner dimensions must match ({} vs {})",
            a.cols(),
            b.rows()
        );
        Product(a, b)
    }
}

impl<A, B> Sum<A, B> {
    /// Constructs the proxy after checking that shapes match.
    ///
    /// # Panics
    ///
    /// Panics if `a.shape() != b.shape()`.
    pub fn new<F>(a: A, b: B) -> Self
    where
        F: FiniteField,
        A: MatrixLike<F>,
        B: MatrixLike<F>,
    {
        assert_eq!(
            a.shape(),
            b.shape(),
            "FieldMatrix::add: shape mismatch ({}×{} vs {}×{})",
            a.rows(),
            a.cols(),
            b.rows(),
            b.cols()
        );
        Sum(a, b)
    }
}

impl<A, B> TransposedProduct<A, B> {
    /// Constructs the `Aᵀ·B` proxy from the un-transposed `a` after
    /// checking that inner dims match.
    ///
    /// # Panics
    ///
    /// Panics if `a.rows() != b.rows()`.
    pub fn new<F>(a: A, b: B) -> Self
    where
        F: FiniteField,
        A: MatrixLike<F>,
        B: MatrixLike<F>,
    {
        assert_eq!(
            a.rows(),
            b.rows(),
            "FieldMatrix::mul: inner dimensions must match ({} vs {})",
            a.rows(),
            b.rows()
        );
        TransposedProduct(a, b)
    }
}

// Products and their fused forms have O(k) `get` cost and stay outside
// `MatrixLike`.

impl<F, A, B> MatrixLike<F> for Sum<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.0.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.0.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        self.0.get(r, c) + self.1.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

impl<F, M> MatrixLike<F> for Scale<F, M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.1.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.1.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        self.0.clone() * self.1.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

impl<F, M> MatrixLike<F> for NegProxy<M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.0.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.0.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        -self.0.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

impl<F: FiniteField> MatrixLike<F> for Transposed<&FieldMatrix<F>> {
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        <FieldMatrix<F> as MatrixLike<F>>::cols(self.0)
    }
    #[inline]
    fn cols(&self) -> usize {
        <FieldMatrix<F> as MatrixLike<F>>::rows(self.0)
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        <FieldMatrix<F> as MatrixLike<F>>::get(self.0, c, r)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        self.0.clone()
    }
}

impl<F, A, B> MatrixLike<F> for FusedLinear<Scale<F, A>, Scale<F, B>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Owned = FieldMatrix<F>;
    #[inline]
    fn rows(&self) -> usize {
        self.0.rows()
    }
    #[inline]
    fn cols(&self) -> usize {
        self.0.cols()
    }
    #[inline]
    fn get(&self, r: usize, c: usize) -> F {
        self.0 .0.clone() * self.0 .1.get(r, c) + self.1 .0.clone() * self.1 .1.get(r, c)
    }
    fn transpose(&self) -> FieldMatrix<F> {
        materialise(self).transpose()
    }
}

/// Materialises a `MatrixLike<F>` proxy as an owned [`FieldMatrix<F>`].
fn materialise<F, E>(expr: &E) -> FieldMatrix<F>
where
    F: FiniteField,
    E: MatrixLike<F>,
{
    let (rows, cols) = expr.shape();
    if rows == 0 || cols == 0 {
        let zero = match F::zero_hint() {
            Some(z) => z,
            None => {
                return FieldMatrix::from_raw_parts(rows, cols, FieldVec::<F>::new());
            }
        };
        return FieldMatrix::from_raw_parts(rows, cols, FieldVec::zeros_from(0, &zero));
    }
    let zero = expr.get(0, 0).zero_like();
    let mut data = FieldVec::<F>::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            data.push(expr.get(r, c));
        }
    }
    let _ = zero;
    FieldMatrix::from_raw_parts(rows, cols, data)
}

impl<F: FiniteField> Evaluate<F> for Transposed<&FieldMatrix<F>> {
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let t = <FieldMatrix<F> as MatrixLike<F>>::transpose(self.0);
        copy_into(&t, out);
    }
    fn shape(&self) -> (usize, usize) {
        (
            <FieldMatrix<F> as MatrixLike<F>>::cols(self.0),
            <FieldMatrix<F> as MatrixLike<F>>::rows(self.0),
        )
    }
}

impl<F, A, B> Evaluate<F> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        match (A::as_concrete(&self.0), B::as_concrete(&self.1)) {
            (Some(a), Some(b)) => gemm_concrete(a, b, out),
            _ => gemm_matrixlike(&self.0, &self.1, out),
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0.rows(), self.1.cols())
    }
}

/// Lets a `MatrixLike` operand expose itself as a concrete
/// `&FieldMatrix<F>` so the evaluator can route through the blocked gemm.
/// The default returns `None`; `&FieldMatrix<F>` returns `Some(self)`.
#[doc(hidden)]
pub trait ConcreteRef<F: FiniteField>: MatrixLike<F> {
    /// Returns `Some(self)` when `Self` is `&FieldMatrix<F>`, else `None`.
    fn as_concrete(&self) -> Option<&FieldMatrix<F>> {
        None
    }
}

impl<F: FiniteField> ConcreteRef<F> for &FieldMatrix<F> {
    fn as_concrete(&self) -> Option<&FieldMatrix<F>> {
        Some(self)
    }
}

impl<F, A, B> ConcreteRef<F> for Sum<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
}

impl<F, M> ConcreteRef<F> for Scale<F, M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
}

impl<F, M> ConcreteRef<F> for NegProxy<M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
}

impl<F: FiniteField> ConcreteRef<F> for Transposed<&FieldMatrix<F>> {}

impl<F, A, B> ConcreteRef<F> for FusedLinear<Scale<F, A>, Scale<F, B>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
}

impl<F, A, B> Evaluate<F> for Sum<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        // Sum is axpy_linear with α = β = 1.
        assert_eq!(
            self.0.shape(),
            self.1.shape(),
            "Sum::evaluate_into: operand shape mismatch"
        );
        if self.0.rows() == 0 || self.0.cols() == 0 {
            return;
        }
        let one = self.0.get(0, 0).one_like();
        axpy_linear(one.clone(), &self.0, one, &self.1, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.0.shape()
    }
}

impl<F, M> Evaluate<F> for Scale<F, M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        scale_into(self.0, &self.1, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.1.shape()
    }
}

impl<F, M> Evaluate<F> for NegProxy<M>
where
    F: FiniteField,
    M: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        neg_into(&self.0, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.0.shape()
    }
}

impl<F, A, B, C> Evaluate<F> for FusedProductPlus<Product<A, B>, C>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0 .0.rows(), self.0 .1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlus::evaluate_into: output shape mismatch"
        );
        // Source β = 1 from whichever operand has storage.
        let one = if m > 0 && self.0 .0.cols() > 0 {
            self.0 .0.get(0, 0).one_like()
        } else if self.1.rows() > 0 && self.1.cols() > 0 {
            self.1.get(0, 0).one_like()
        } else {
            // Empty output: nothing to do.
            return;
        };
        let Product(a, b) = self.0;
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => gemm_with_beta_concrete(ar, br, one, &self.1, out),
            _ => {
                bump(&KC_GEMM_BETA);
                let (mm, kk) = a.shape();
                let (_, nn) = b.shape();
                let zero = if mm > 0 && kk > 0 {
                    a.get(0, 0).zero_like()
                } else if self.1.rows() > 0 && self.1.cols() > 0 {
                    self.1.get(0, 0).zero_like()
                } else {
                    return;
                };
                for i in 0..mm {
                    for j in 0..nn {
                        let mut acc = zero.zero_like();
                        for t in 0..kk {
                            acc += a.get(i, t) * b.get(t, j);
                        }
                        acc += self.1.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .0.rows(), self.0 .1.cols())
    }
}

impl<F, A, B, C> Evaluate<F> for FusedProductPlusScaled<Product<A, B>, Scale<F, C>>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0 .0.rows(), self.0 .1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlusScaled::evaluate_into: output shape mismatch"
        );
        let Product(a, b) = self.0;
        let Scale(beta, c) = self.1;
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => gemm_with_beta_concrete(ar, br, beta, &c, out),
            _ => {
                bump(&KC_GEMM_BETA);
                let (mm, kk) = a.shape();
                let (_, nn) = b.shape();
                if mm == 0 || nn == 0 {
                    return;
                }
                let zero = if kk > 0 {
                    a.get(0, 0).zero_like()
                } else if c.rows() > 0 && c.cols() > 0 {
                    c.get(0, 0).zero_like()
                } else {
                    return;
                };
                for i in 0..mm {
                    for j in 0..nn {
                        let mut acc = zero.zero_like();
                        for t in 0..kk {
                            acc += a.get(i, t) * b.get(t, j);
                        }
                        acc += beta.clone() * c.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .0.rows(), self.0 .1.cols())
    }
}

impl<F, A, B> Evaluate<F> for FusedLinear<Scale<F, A>, Scale<F, B>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let Scale(alpha, a) = self.0;
        let Scale(beta, b) = self.1;
        axpy_linear(alpha, &a, beta, &b, out);
    }
    fn shape(&self) -> (usize, usize) {
        self.0 .1.shape()
    }
}

impl<F, A, B> Evaluate<F> for TransposedProduct<A, B>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0.cols(), self.1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "TransposedProduct::evaluate_into: output shape mismatch"
        );
        match (A::as_concrete(&self.0), B::as_concrete(&self.1)) {
            (Some(ar), Some(br)) => gemm_trans_a_concrete(ar, br, out),
            _ => {
                bump(&KC_GEMM_TA);
                if m == 0 || n == 0 {
                    return;
                }
                let k = self.0.rows();
                if k == 0 {
                    return;
                }
                let zero = self.0.get(0, 0).zero_like();
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += self.0.get(t, i) * self.1.get(t, j);
                        }
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0.cols(), self.1.cols())
    }
}

// Compositional fusion: `Aᵀ·B + C` collapses to `gemm_trans_a_with_beta`.
impl<F, A, B, C> Evaluate<F> for FusedProductPlus<TransposedProduct<A, B>, C>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let (m, n) = (self.0 .0.cols(), self.0 .1.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlus<TransposedProduct, C>: output shape mismatch"
        );
        let one = if self.0 .0.rows() > 0 && m > 0 {
            self.0 .0.get(0, 0).one_like()
        } else if self.1.rows() > 0 && self.1.cols() > 0 {
            self.1.get(0, 0).one_like()
        } else {
            return;
        };
        let TransposedProduct(a, b) = self.0;
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => {
                gemm_trans_a_with_beta_concrete(one.clone(), ar, br, one, &self.1, out)
            }
            _ => {
                bump(&KC_GEMM_TA_BETA);
                let k = a.rows();
                let zero = one.zero_like();
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += a.get(t, i) * b.get(t, j);
                        }
                        acc += self.1.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .0.cols(), self.0 .1.cols())
    }
}

impl<F, A, B> Evaluate<F> for ScaledTransposedProduct<F, A, B>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let ScaledTransposedProduct(alpha, a, b) = self;
        let (m, n) = (a.cols(), b.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "ScaledTransposedProduct::evaluate_into: output shape mismatch"
        );
        if m == 0 || n == 0 {
            return;
        }
        let k = a.rows();
        let zero = if k > 0 {
            a.get(0, 0).zero_like()
        } else {
            // k = 0: the output is zero; take the witness from b.
            if b.rows() > 0 && b.cols() > 0 {
                b.get(0, 0).zero_like()
            } else {
                // All operands empty along k — nothing to write.
                return;
            }
        };
        // A zero-valued C adapter keeps this at one kernel call.
        struct ZeroC<'a, F: FiniteField> {
            m: usize,
            n: usize,
            zero: &'a F,
        }
        impl<'a, F: FiniteField> MatrixLike<F> for ZeroC<'a, F> {
            type Owned = FieldMatrix<F>;
            fn rows(&self) -> usize {
                self.m
            }
            fn cols(&self) -> usize {
                self.n
            }
            fn get(&self, _r: usize, _c: usize) -> F {
                self.zero.clone()
            }
            fn transpose(&self) -> FieldMatrix<F> {
                unimplemented!("ZeroC is a kernel-internal witness adapter")
            }
        }
        let c_witness = ZeroC { m, n, zero: &zero };
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => {
                gemm_trans_a_with_beta_concrete(alpha, ar, br, zero.clone(), &c_witness, out)
            }
            _ => {
                bump(&KC_GEMM_TA_BETA);
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += a.get(t, i) * b.get(t, j);
                        }
                        out.set(i, j, alpha.clone() * acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.1.cols(), self.2.cols())
    }
}

// Compositional fusion: `α·Aᵀ·B + β·C` collapses to `gemm_trans_a_with_beta`.
impl<F, A, B, C> Evaluate<F>
    for FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, C>>
where
    F: FiniteField,
    A: MatrixLike<F> + ConcreteRef<F>,
    B: MatrixLike<F> + ConcreteRef<F>,
    C: MatrixLike<F>,
{
    fn evaluate_into(self, out: &mut FieldMatrix<F>) {
        let ScaledTransposedProduct(alpha, a, b) = self.0;
        let Scale(beta, c) = self.1;
        let (m, n) = (a.cols(), b.cols());
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(out),
            (m, n),
            "FusedProductPlusScaled<ScaledTransposedProduct, Scale<C>>: output shape mismatch"
        );
        match (A::as_concrete(&a), B::as_concrete(&b)) {
            (Some(ar), Some(br)) => gemm_trans_a_with_beta_concrete(alpha, ar, br, beta, &c, out),
            _ => {
                bump(&KC_GEMM_TA_BETA);
                if m == 0 || n == 0 {
                    return;
                }
                let k = a.rows();
                if k == 0 {
                    for i in 0..m {
                        for j in 0..n {
                            out.set(i, j, beta.clone() * c.get(i, j));
                        }
                    }
                    return;
                }
                let zero = a.get(0, 0).zero_like();
                for i in 0..m {
                    for j in 0..n {
                        let mut acc = zero.zero_like();
                        for t in 0..k {
                            acc += a.get(t, i) * b.get(t, j);
                        }
                        acc = alpha.clone() * acc + beta.clone() * c.get(i, j);
                        out.set(i, j, acc);
                    }
                }
            }
        }
    }
    fn shape(&self) -> (usize, usize) {
        (self.0 .1.cols(), self.0 .2.cols())
    }
}

// The bridge is sealed through `sealed::ProxyExpr`: a blanket over
// `E: Evaluate<F>` alone is rejected with E0119 because downstream crates
// may implement `Evaluate<F>` for `FieldMatrix<F>`.
mod sealed {
    /// Marker for the `From<E> for FieldMatrix<F>` bridge.
    pub trait ProxyExpr {}
}

impl<A, B> sealed::ProxyExpr for Product<A, B> {}
impl<A, B> sealed::ProxyExpr for Sum<A, B> {}
impl<F: FiniteField, M> sealed::ProxyExpr for Scale<F, M> {}
impl<M> sealed::ProxyExpr for NegProxy<M> {}
impl<F: FiniteField> sealed::ProxyExpr for Transposed<&FieldMatrix<F>> {}
impl<P, C> sealed::ProxyExpr for FusedProductPlus<P, C> {}
impl<P, SS> sealed::ProxyExpr for FusedProductPlusScaled<P, SS> {}
impl<A, B> sealed::ProxyExpr for FusedLinear<A, B> {}
impl<A, B> sealed::ProxyExpr for TransposedProduct<A, B> {}
impl<F: FiniteField, A, B> sealed::ProxyExpr for ScaledTransposedProduct<F, A, B> {}

impl<F, E> From<E> for FieldMatrix<F>
where
    F: ConstField,
    E: Evaluate<F> + sealed::ProxyExpr,
{
    fn from(expr: E) -> Self {
        let (rows, cols) = expr.shape();
        let mut out = FieldMatrix::<F>::zeros(rows, cols);
        expr.evaluate_into(&mut out);
        out
    }
}

impl<F: ConstField> FieldMatrix<F> {
    /// Evaluates a proxy expression into a fresh owned matrix; equivalent
    /// to `expr.into()`.
    ///
    /// # Panics
    ///
    /// Panics if the operand shapes of `expr` are inconsistent.
    ///
    /// # Complexity
    ///
    /// One allocation plus one kernel call per canonical fusion.
    pub fn eval<E>(expr: E) -> FieldMatrix<F>
    where
        E: Evaluate<F> + sealed::ProxyExpr,
    {
        FieldMatrix::<F>::from(expr)
    }
}

impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for &'a FieldMatrix<F> {
    type Output = Product<&'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        Product::new::<F>(self, rhs)
    }
}

impl<'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        crate::field::matrix::gemm(&self, rhs)
    }
}

impl<F: FiniteField> Mul<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: FieldMatrix<F>) -> Self::Output {
        crate::field::matrix::gemm(self, &rhs)
    }
}

impl<F: FiniteField> Mul<FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn mul(self, rhs: FieldMatrix<F>) -> Self::Output {
        crate::field::matrix::gemm(&self, &rhs)
    }
}

impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for Transposed<&'a FieldMatrix<F>> {
    type Output = TransposedProduct<&'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        TransposedProduct::new::<F>(self.0, rhs)
    }
}

impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for Scale<F, Transposed<&'a FieldMatrix<F>>> {
    type Output = ScaledTransposedProduct<F, &'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        // Aᵀ·B requires a.rows() == b.rows().
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::rows(self.1 .0),
            <FieldMatrix<F> as MatrixLike<F>>::rows(rhs),
            "FieldMatrix::mul: inner dimensions must match ({} vs {})",
            <FieldMatrix<F> as MatrixLike<F>>::rows(self.1 .0),
            <FieldMatrix<F> as MatrixLike<F>>::rows(rhs)
        );
        ScaledTransposedProduct(self.0, self.1 .0, rhs)
    }
}

impl<'a, 'b, F: FiniteField> Mul<&'b FieldMatrix<F>> for Scale<F, &'a FieldMatrix<F>> {
    type Output = Scale<F, Product<&'a FieldMatrix<F>, &'b FieldMatrix<F>>>;
    fn mul(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        let p = Product::new::<F>(self.1, rhs);
        Scale(self.0, p)
    }
}

impl<'a, 'b, F: FiniteField> Mul<Scale<F, &'b FieldMatrix<F>>> for &'a FieldMatrix<F> {
    type Output = Scale<F, Product<&'a FieldMatrix<F>, &'b FieldMatrix<F>>>;
    fn mul(self, rhs: Scale<F, &'b FieldMatrix<F>>) -> Self::Output {
        let p = Product::new::<F>(self, rhs.1);
        Scale(rhs.0, p)
    }
}

impl<'a, F: FiniteField> Mul<F> for &'a FieldMatrix<F> {
    type Output = Scale<F, &'a FieldMatrix<F>>;
    fn mul(self, rhs: F) -> Self::Output {
        Scale(rhs, self)
    }
}

impl<F: FiniteField> Mul<F> for FieldMatrix<F> {
    type Output = Scale<F, FieldMatrix<F>>;
    fn mul(self, rhs: F) -> Self::Output {
        Scale(rhs, self)
    }
}

impl<F: FiniteField, A, B> Mul<F> for Product<A, B> {
    type Output = Scale<F, Product<A, B>>;
    fn mul(self, rhs: F) -> Self::Output {
        Scale(rhs, self)
    }
}

/// Stamps out `F * &M`, `F * M` and `F * Transposed<&M>` returning [`Scale`]
/// proxies for one concrete `ConstField` type. The orphan rule forbids a
/// blanket `impl<F: ConstField> Mul<&M<F>> for F`.
macro_rules! impl_left_scalar_mul_proxy {
    ($field_ty:ty $(, $($generics:tt)+)?) => {
        impl<'a $(, $($generics)+)?> Mul<&'a FieldMatrix<$field_ty>> for $field_ty {
            type Output = Scale<$field_ty, &'a FieldMatrix<$field_ty>>;
            #[inline]
            fn mul(self, rhs: &'a FieldMatrix<$field_ty>) -> Self::Output {
                Scale(self, rhs)
            }
        }

        impl$(<$($generics)+>)? Mul<FieldMatrix<$field_ty>> for $field_ty {
            type Output = Scale<$field_ty, FieldMatrix<$field_ty>>;
            #[inline]
            fn mul(self, rhs: FieldMatrix<$field_ty>) -> Self::Output {
                Scale(self, rhs)
            }
        }

        // Makes `alpha * a.t()` a proxy for the `αAᵀ·B + βC` fusion.
        impl<'a $(, $($generics)+)?> Mul<Transposed<&'a FieldMatrix<$field_ty>>> for $field_ty {
            type Output = Scale<$field_ty, Transposed<&'a FieldMatrix<$field_ty>>>;
            #[inline]
            fn mul(self, rhs: Transposed<&'a FieldMatrix<$field_ty>>) -> Self::Output {
                Scale(self, rhs)
            }
        }
    };
}

impl_left_scalar_mul_proxy!(crate::gfp::Fp<P>, const P: u64);
impl_left_scalar_mul_proxy!(crate::gfp::specialized::GoldilocksFp);
impl_left_scalar_mul_proxy!(
    crate::gfpn::QuadraticExt<C>,
    C: crate::gfpn::ExtConfig
);
impl_left_scalar_mul_proxy!(
    crate::gfpn::CubicExt<C>,
    C: crate::gfpn::ExtConfig
);
impl_left_scalar_mul_proxy!(
    crate::gf2m::Gf2mWide<N, Cfg>,
    const N: usize,
    Cfg: crate::gf2m::Gf2mWideConfig<N> + Send + Sync + 'static
);

impl<'a, 'b, F: FiniteField> Add<&'b FieldMatrix<F>> for &'a FieldMatrix<F> {
    type Output = Sum<&'a FieldMatrix<F>, &'b FieldMatrix<F>>;
    fn add(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        Sum::new::<F>(self, rhs)
    }
}

impl<F: FiniteField> Add<FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_add_owned(self, rhs)
    }
}

impl<'b, F: FiniteField> Add<&'b FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: &'b FieldMatrix<F>) -> Self::Output {
        elementwise_add_owned(self, rhs.clone())
    }
}

impl<F: FiniteField> Add<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn add(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_add_owned(self.clone(), rhs)
    }
}

fn elementwise_add_owned<F: FiniteField>(a: FieldMatrix<F>, b: FieldMatrix<F>) -> FieldMatrix<F> {
    assert_eq!(
        <FieldMatrix<F> as MatrixLike<F>>::shape(&a),
        <FieldMatrix<F> as MatrixLike<F>>::shape(&b),
        "FieldMatrix::add: shape mismatch"
    );
    let (rows, cols) = <FieldMatrix<F> as MatrixLike<F>>::shape(&a);
    if rows == 0 || cols == 0 {
        // Empty output: reuse one operand's shape. Its storage is empty.
        return a;
    }
    let mut data = FieldVec::<F>::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            data.push(
                <FieldMatrix<F> as MatrixLike<F>>::get(&a, r, c)
                    + <FieldMatrix<F> as MatrixLike<F>>::get(&b, r, c),
            );
        }
    }
    FieldMatrix::from_raw_parts(rows, cols, data)
}

impl<'c, F, A, B> Add<&'c FieldMatrix<F>> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<Product<A, B>, &'c FieldMatrix<F>>;
    fn add(self, rhs: &'c FieldMatrix<F>) -> Self::Output {
        let m = self.0.rows();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs),
            (m, n),
            "FieldMatrix::add: product·addend shape mismatch"
        );
        FusedProductPlus(self, rhs)
    }
}

impl<'a, F, A, B> Add<Product<A, B>> for &'a FieldMatrix<F>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<Product<A, B>, &'a FieldMatrix<F>>;
    fn add(self, rhs: Product<A, B>) -> Self::Output {
        let m = rhs.0.rows();
        let n = rhs.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(self),
            (m, n),
            "FieldMatrix::add: addend·product shape mismatch"
        );
        FusedProductPlus(rhs, self)
    }
}

impl<'c, F, A, B> Add<Scale<F, &'c FieldMatrix<F>>> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlusScaled<Product<A, B>, Scale<F, &'c FieldMatrix<F>>>;
    fn add(self, rhs: Scale<F, &'c FieldMatrix<F>>) -> Self::Output {
        let m = self.0.rows();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs.1),
            (m, n),
            "FieldMatrix::add: product·scaled-addend shape mismatch"
        );
        FusedProductPlusScaled(self, rhs)
    }
}

impl<'a, F, A, B> Add<Product<A, B>> for Scale<F, &'a FieldMatrix<F>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlusScaled<Product<A, B>, Scale<F, &'a FieldMatrix<F>>>;
    fn add(self, rhs: Product<A, B>) -> Self::Output {
        let m = rhs.0.rows();
        let n = rhs.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(self.1),
            (m, n),
            "FieldMatrix::add: scaled-addend·product shape mismatch"
        );
        FusedProductPlusScaled(rhs, self)
    }
}

impl<F, A, B> Add<Scale<F, B>> for Scale<F, A>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedLinear<Scale<F, A>, Scale<F, B>>;
    fn add(self, rhs: Scale<F, B>) -> Self::Output {
        assert_eq!(
            self.1.shape(),
            rhs.1.shape(),
            "FieldMatrix::add: shape mismatch between scaled operands"
        );
        FusedLinear(self, rhs)
    }
}

impl<'c, F, A, B> Add<&'c FieldMatrix<F>> for TransposedProduct<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<TransposedProduct<A, B>, &'c FieldMatrix<F>>;
    fn add(self, rhs: &'c FieldMatrix<F>) -> Self::Output {
        let m = self.0.cols();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs),
            (m, n),
            "FieldMatrix::add: Aᵀ·B shape mismatch"
        );
        FusedProductPlus(self, rhs)
    }
}

impl<'c, F, A, B> Add<Scale<F, &'c FieldMatrix<F>>> for ScaledTransposedProduct<F, A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output =
        FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, &'c FieldMatrix<F>>>;
    fn add(self, rhs: Scale<F, &'c FieldMatrix<F>>) -> Self::Output {
        let m = self.1.cols();
        let n = self.2.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs.1),
            (m, n),
            "FieldMatrix::add: scaled-Aᵀ·B · scaled-addend shape mismatch"
        );
        FusedProductPlusScaled(self, rhs)
    }
}

impl<'a, F, A, B> Add<ScaledTransposedProduct<F, A, B>> for Scale<F, &'a FieldMatrix<F>>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output =
        FusedProductPlusScaled<ScaledTransposedProduct<F, A, B>, Scale<F, &'a FieldMatrix<F>>>;
    fn add(self, rhs: ScaledTransposedProduct<F, A, B>) -> Self::Output {
        let m = rhs.1.cols();
        let n = rhs.2.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(self.1),
            (m, n),
            "FieldMatrix::add: scaled-addend · scaled-Aᵀ·B shape mismatch"
        );
        FusedProductPlusScaled(rhs, self)
    }
}

impl<F: FiniteField> Sub<&FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_sub(self, rhs)
    }
}

impl<F: FiniteField> Sub<&FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: &FieldMatrix<F>) -> Self::Output {
        elementwise_sub(&self, rhs)
    }
}

impl<F: FiniteField> Sub<FieldMatrix<F>> for &FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_sub(self, &rhs)
    }
}

impl<F: FiniteField> Sub<FieldMatrix<F>> for FieldMatrix<F> {
    type Output = FieldMatrix<F>;
    fn sub(self, rhs: FieldMatrix<F>) -> Self::Output {
        elementwise_sub(&self, &rhs)
    }
}

fn elementwise_sub<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    let (ar, ac) = <FieldMatrix<F> as MatrixLike<F>>::shape(a);
    let (br, bc) = <FieldMatrix<F> as MatrixLike<F>>::shape(b);
    assert_eq!(
        (ar, ac),
        (br, bc),
        "FieldMatrix::sub: shape mismatch ({}×{} vs {}×{})",
        ar,
        ac,
        br,
        bc
    );
    if ar == 0 || ac == 0 {
        return a.clone();
    }
    let mut data = FieldVec::<F>::with_capacity(ar * ac);
    for r in 0..ar {
        for c in 0..ac {
            data.push(
                <FieldMatrix<F> as MatrixLike<F>>::get(a, r, c)
                    - <FieldMatrix<F> as MatrixLike<F>>::get(b, r, c),
            );
        }
    }
    FieldMatrix::from_raw_parts(ar, ac, data)
}

impl<'c, F, A, B> Sub<&'c FieldMatrix<F>> for Product<A, B>
where
    F: FiniteField,
    A: MatrixLike<F>,
    B: MatrixLike<F>,
{
    type Output = FusedProductPlus<Product<A, B>, NegProxy<&'c FieldMatrix<F>>>;
    fn sub(self, rhs: &'c FieldMatrix<F>) -> Self::Output {
        let m = self.0.rows();
        let n = self.1.cols();
        assert_eq!(
            <FieldMatrix<F> as MatrixLike<F>>::shape(rhs),
            (m, n),
            "FieldMatrix::sub: product·addend shape mismatch"
        );
        FusedProductPlus(self, NegProxy(rhs))
    }
}

impl<'a, F: FiniteField> Neg for &'a FieldMatrix<F> {
    type Output = NegProxy<&'a FieldMatrix<F>>;
    fn neg(self) -> Self::Output {
        NegProxy(self)
    }
}

impl<F: FiniteField> Neg for FieldMatrix<F> {
    type Output = NegProxy<FieldMatrix<F>>;
    fn neg(self) -> Self::Output {
        NegProxy(self)
    }
}

impl<M> Neg for NegProxy<M> {
    type Output = M;
    fn neg(self) -> Self::Output {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    const MERSENNE_31: u64 = 2_147_483_647;
    type M31 = Fp<MERSENNE_31>;

    // GF(2^8) with the AES polynomial (`@/citation/Nist2001`).
    struct Gf2m8AesCfg;
    impl Gf2mWideConfig<1> for Gf2m8AesCfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "Gf2m8AesCfg";
    }
    type Gf2m8 = Gf2mWide<1, Gf2m8AesCfg>;

    fn rand_m31(rows: usize, cols: usize, seed: u64) -> FieldMatrix<M31> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<M31>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, M31::new(rng.gen::<u64>() % MERSENNE_31));
            }
        }
        m
    }

    fn rand_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    #[test]
    fn test_fusion_product_plus_one_gemm_with_beta_call() {
        let a = rand_m31(8, 6, 0x101);
        let b = rand_m31(6, 7, 0x102);
        let c = rand_m31(8, 7, 0x103);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (&a * &b + &c).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
        assert_eq!(after.gemm - before.gemm, 0);
        assert_eq!(after.axpy_linear - before.axpy_linear, 0);
    }

    #[test]
    fn test_fusion_product_plus_scaled_one_gemm_with_beta_call() {
        let a = rand_m31(5, 4, 0x201);
        let b = rand_m31(4, 6, 0x202);
        let c = rand_m31(5, 6, 0x203);
        let beta = M31::new(42);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (&a * &b + beta * &c).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
        assert_eq!(after.gemm - before.gemm, 0);
        assert_eq!(after.axpy_linear - before.axpy_linear, 0);
    }

    #[test]
    fn test_fusion_product_plus_scaled_right_one_gemm_with_beta_call() {
        let a = rand_m31(5, 4, 0x301);
        let b = rand_m31(4, 6, 0x302);
        let c = rand_m31(5, 6, 0x303);
        let beta = M31::new(17);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (&a * &b + &c * beta).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 1);
        assert_eq!(after.gemm - before.gemm, 0);
    }

    #[test]
    fn test_fusion_linear_one_axpy_call() {
        let a = rand_m31(6, 9, 0x401);
        let b = rand_m31(6, 9, 0x402);
        let alpha = M31::new(3);
        let beta = M31::new(5);
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (alpha * &a + beta * &b).into();
        let after = kernel_counts();
        assert_eq!(after.axpy_linear - before.axpy_linear, 1);
        assert_eq!(after.gemm - before.gemm, 0);
        assert_eq!(after.gemm_with_beta - before.gemm_with_beta, 0);
    }

    #[test]
    fn test_fusion_transposed_product_one_gemm_trans_a_call() {
        let a = rand_m31(6, 5, 0x501); // k=6, m=5 → Aᵀ is 5×6
        let b = rand_m31(6, 7, 0x502); // Aᵀ·B ⇒ 5×7
        reset_kernel_counts();
        let before = kernel_counts();
        let _r: FieldMatrix<M31> = (a.t() * &b).into();
        let after = kernel_counts();
        assert_eq!(after.gemm_trans_a - before.gemm_trans_a, 1);
        assert_eq!(after.gemm - before.gemm, 0);
    }

    #[test]
    fn test_fusion_alpha_transposed_product_plus_beta_c_one_call() {
        let a = rand_m31(6, 5, 0x601); // a is 6×5 → a.t() is 5×6
        let b = rand_m31(6, 7, 0x602); // Aᵀ·B is 5×7
        let c = rand_m31(5, 7, 0x603);
        let alpha = M31::new(3);
        let beta = M31::new(5);
        reset_kernel_counts();
        let _r: FieldMatrix<M31> = ((alpha * a.t()) * &b + beta * &c).into();
        let kc = kernel_counts();
        assert_eq!(kc.gemm_trans_a_with_beta, 1);
        assert_eq!(kc.gemm_trans_a, 0);
        assert_eq!(kc.gemm_with_beta, 0);
        assert_eq!(kc.gemm, 0);
        assert_eq!(kc.axpy_linear, 0);
        assert_eq!(kc.scale_into, 0);
    }

    #[test]
    fn test_fusion_alpha_transposed_product_plus_beta_c_commuted_one_call() {
        let a = rand_m31(6, 5, 0x611);
        let b = rand_m31(6, 7, 0x612);
        let c = rand_m31(5, 7, 0x613);
        let alpha = M31::new(4);
        let beta = M31::new(9);
        reset_kernel_counts();
        let _r: FieldMatrix<M31> = (beta * &c + (alpha * a.t()) * &b).into();
        let kc = kernel_counts();
        assert_eq!(kc.gemm_trans_a_with_beta, 1);
        assert_eq!(kc.gemm_trans_a, 0);
        assert_eq!(kc.gemm_with_beta, 0);
        assert_eq!(kc.gemm, 0);
    }

    #[test]
    fn test_alpha_transposed_product_plus_beta_c_bit_exact() {
        let a = rand_m31(6, 5, 0x621);
        let b = rand_m31(6, 7, 0x622);
        let c = rand_m31(5, 7, 0x623);
        let alpha = M31::new(3);
        let beta = M31::new(5);

        let fused: FieldMatrix<M31> = ((alpha * a.t()) * &b + beta * &c).into();

        let at_b: FieldMatrix<M31> = (a.t() * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(5, 7);
        for i in 0..5 {
            for j in 0..7 {
                expected.set(i, j, alpha * at_b.get(i, j) + beta * c.get(i, j));
            }
        }
        assert_eq!(fused, expected);
    }

    fn bit_exact_fused_vs_eager_m31(n: usize) {
        let a = rand_m31(n, n, 0x700 ^ n as u64);
        let b = rand_m31(n, n, 0x701 ^ n as u64);
        let c = rand_m31(n, n, 0x702 ^ n as u64);
        let fused: FieldMatrix<M31> = (&a * &b + &c).into();
        let t: FieldMatrix<M31> = (&a * &b).into();
        let eager: FieldMatrix<M31> = (&t + &c).into();
        assert_eq!(fused, eager, "fused != eager at n={}", n);
    }

    fn bit_exact_fused_vs_eager_gf2m8(n: usize) {
        let a = rand_gf2m8(n, n, 0x800 ^ n as u64);
        let b = rand_gf2m8(n, n, 0x801 ^ n as u64);
        let c = rand_gf2m8(n, n, 0x802 ^ n as u64);
        let fused: FieldMatrix<Gf2m8> = (&a * &b + &c).into();
        let t: FieldMatrix<Gf2m8> = (&a * &b).into();
        let eager: FieldMatrix<Gf2m8> = (&t + &c).into();
        assert_eq!(fused, eager, "fused != eager at n={}", n);
    }

    #[test]
    fn test_fused_bit_exact_m31_n7() {
        bit_exact_fused_vs_eager_m31(7);
    }

    #[test]
    fn test_fused_bit_exact_m31_n64() {
        bit_exact_fused_vs_eager_m31(64);
    }

    #[test]
    #[ignore = "slow: fused-vs-eager bit-exact at n=256 (Mersenne-31)"]
    fn test_fused_bit_exact_m31_n256() {
        bit_exact_fused_vs_eager_m31(256);
    }

    #[test]
    fn test_fused_bit_exact_gf2m8_n7() {
        bit_exact_fused_vs_eager_gf2m8(7);
    }

    #[test]
    fn test_fused_bit_exact_gf2m8_n64() {
        bit_exact_fused_vs_eager_gf2m8(64);
    }

    #[test]
    #[ignore = "slow: fused-vs-eager bit-exact at n=256 (GF(2^8))"]
    fn test_fused_bit_exact_gf2m8_n256() {
        bit_exact_fused_vs_eager_gf2m8(256);
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::mul: inner dimensions must match")]
    fn test_product_construction_panics_on_dim_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(5, 6);
        let _p = &a * &b;
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::add: shape mismatch")]
    fn test_sum_construction_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 3);
        let _s = &a + &b;
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::mul: inner dimensions must match")]
    fn test_transposed_product_construction_panics_on_dim_mismatch() {
        // The inner dimension of Aᵀ·B is a.rows() vs b.rows().
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(5, 6);
        let _p = a.t() * &b;
    }

    #[test]
    #[should_panic(expected = "FieldMatrix::add")]
    fn test_fused_product_plus_construction_panics_on_shape_mismatch() {
        // Product is 3×5, addend is 3×6 → shape mismatch.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let c = FieldMatrix::<M31>::zeros(3, 6);
        let _f = &a * &b + &c;
    }

    #[test]
    #[should_panic(expected = "copy_into: shape mismatch")]
    fn test_evaluate_into_copy_panics_on_shape_mismatch() {
        // Transposed<&M>::evaluate_into routes through `copy_into`.
        let a = FieldMatrix::<M31>::zeros(3, 4); // transpose shape: 4×3
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        a.t().evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "scale_into: shape mismatch")]
    fn test_evaluate_into_scale_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        let s = M31::new(7) * &a;
        s.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "neg_into: shape mismatch")]
    fn test_evaluate_into_neg_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        let n = -&a;
        n.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "axpy_linear: output shape mismatch")]
    fn test_evaluate_into_axpy_linear_panics_on_shape_mismatch() {
        // `Sum::evaluate_into` calls `axpy_linear`.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(3, 4);
        let mut out = FieldMatrix::<M31>::zeros(5, 6);
        let s = &a + &b;
        s.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "gemm_concrete: output shape mismatch")]
    fn test_evaluate_into_gemm_panics_on_shape_mismatch() {
        // Concrete operands route through `gemm_concrete`.
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let p = &a * &b;
        p.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "FusedProductPlus::evaluate_into: output shape mismatch")]
    fn test_evaluate_into_gemm_with_beta_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let c = FieldMatrix::<M31>::zeros(3, 5);
        let mut out = FieldMatrix::<M31>::zeros(6, 6);
        let f = &a * &b + &c;
        f.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "TransposedProduct::evaluate_into: output shape mismatch")]
    fn test_evaluate_into_gemm_trans_a_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(6, 5); // a.t() is 5×6
        let b = FieldMatrix::<M31>::zeros(6, 7); // Aᵀ·B is 5×7
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let tp = a.t() * &b;
        tp.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "FusedProductPlus<TransposedProduct, C>: output shape mismatch")]
    fn test_evaluate_into_gemm_trans_a_with_beta_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(6, 5);
        let b = FieldMatrix::<M31>::zeros(6, 7);
        let c = FieldMatrix::<M31>::zeros(5, 7);
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let fused = a.t() * &b + &c;
        fused.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(
        expected = "FusedProductPlusScaled<ScaledTransposedProduct, Scale<C>>: output shape mismatch"
    )]
    fn test_evaluate_into_alpha_trans_a_with_beta_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(6, 5);
        let b = FieldMatrix::<M31>::zeros(6, 7);
        let c = FieldMatrix::<M31>::zeros(5, 7);
        let alpha = M31::new(3);
        let beta = M31::new(4);
        let mut out = FieldMatrix::<M31>::zeros(4, 4);
        let fused = (alpha * a.t()) * &b + beta * &c;
        fused.evaluate_into(&mut out);
    }

    #[test]
    #[should_panic(expected = "FusedProductPlusScaled::evaluate_into: output shape mismatch")]
    fn test_evaluate_into_product_plus_scaled_panics_on_shape_mismatch() {
        let a = FieldMatrix::<M31>::zeros(3, 4);
        let b = FieldMatrix::<M31>::zeros(4, 5);
        let c = FieldMatrix::<M31>::zeros(3, 5);
        let beta = M31::new(2);
        let mut out = FieldMatrix::<M31>::zeros(6, 6);
        let f = &a * &b + beta * &c;
        f.evaluate_into(&mut out);
    }

    #[test]
    fn test_fused_path_allocates_fewer_matrices_than_eager() {
        let a = rand_m31(16, 16, 0x1101);
        let b = rand_m31(16, 16, 0x1102);
        let c = rand_m31(16, 16, 0x1103);

        reset_kernel_counts();
        let _fused: FieldMatrix<M31> = (&a * &b + &c).into();
        let fused_counts = kernel_counts();
        let fused_owned_matrices = fused_counts.gemm
            + fused_counts.gemm_with_beta
            + fused_counts.gemm_trans_a
            + fused_counts.gemm_trans_a_with_beta
            + fused_counts.axpy_linear
            + fused_counts.scale_into
            + fused_counts.neg_into
            + fused_counts.copy_into;

        reset_kernel_counts();
        let t: FieldMatrix<M31> = (&a * &b).into();
        let _eager: FieldMatrix<M31> = (&t + &c).into();
        let eager_counts = kernel_counts();
        let eager_owned_matrices = eager_counts.gemm
            + eager_counts.gemm_with_beta
            + eager_counts.gemm_trans_a
            + eager_counts.gemm_trans_a_with_beta
            + eager_counts.axpy_linear
            + eager_counts.scale_into
            + eager_counts.neg_into
            + eager_counts.copy_into;

        assert_eq!(
            fused_owned_matrices, 1,
            "fused path must produce exactly one owned FieldMatrix (one kernel call)"
        );
        assert!(
            eager_owned_matrices >= 2,
            "eager path must produce at least two owned FieldMatrices; got {}",
            eager_owned_matrices
        );
        assert!(
            fused_owned_matrices < eager_owned_matrices,
            "fused ({}) must allocate fewer owned matrices than eager ({})",
            fused_owned_matrices,
            eager_owned_matrices
        );
    }

    #[test]
    fn test_scale_into_matches_eager_scalar_mul() {
        let a = rand_m31(5, 7, 0x900);
        let alpha = M31::new(11);
        let lazy: FieldMatrix<M31> = (alpha * &a).into();
        let mut eager = FieldMatrix::<M31>::zeros(5, 7);
        for r in 0..5 {
            for c in 0..7 {
                eager.set(r, c, alpha * a.get(r, c));
            }
        }
        assert_eq!(lazy, eager);
    }

    #[test]
    fn test_neg_proxy_into_matches_eager_neg() {
        let a = rand_m31(4, 6, 0xA00);
        let lazy: FieldMatrix<M31> = (-&a).into();
        let mut eager = FieldMatrix::<M31>::zeros(4, 6);
        for r in 0..4 {
            for c in 0..6 {
                eager.set(r, c, -a.get(r, c));
            }
        }
        assert_eq!(lazy, eager);
    }

    #[test]
    fn test_double_negation_normalises() {
        // `-(-&a)` unwraps the `NegProxy` back to `&FieldMatrix<F>`.
        let a = rand_m31(3, 3, 0xB00);
        let twice: &FieldMatrix<M31> = -(-&a);
        assert_eq!(twice.clone(), a);
    }

    #[test]
    fn test_sum_matrix_like_get_is_operand_sum() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0xC00);
        let b = rand_m31(3, 4, 0xC01);
        let s = &a + &b;
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(
                    <_ as MatrixLike<M31>>::get(&s, r, c),
                    a.get(r, c) + b.get(r, c)
                );
            }
        }
        assert_eq!(<_ as MatrixLike<M31>>::shape(&s), (3, 4));
    }

    #[test]
    fn test_scale_matrix_like_get_is_scalar_times_operand() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0xD00);
        let alpha = M31::new(5);
        let s = alpha * &a;
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(<_ as MatrixLike<M31>>::get(&s, r, c), alpha * a.get(r, c));
            }
        }
    }

    #[test]
    fn test_neg_proxy_matrix_like_get_is_negated_operand() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0xE00);
        let n = -&a;
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(<_ as MatrixLike<M31>>::get(&n, r, c), -a.get(r, c));
            }
        }
    }

    #[test]
    fn test_fused_linear_matrix_like_get_matches_alpha_a_plus_beta_b() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 5, 0xF00);
        let b = rand_m31(3, 5, 0xF01);
        let alpha = M31::new(2);
        let beta = M31::new(3);
        let fl = alpha * &a + beta * &b;
        for r in 0..3 {
            for c in 0..5 {
                assert_eq!(
                    <_ as MatrixLike<M31>>::get(&fl, r, c),
                    alpha * a.get(r, c) + beta * b.get(r, c)
                );
            }
        }
    }

    #[test]
    fn test_field_matrix_eval_sugar_equals_into() {
        let a = rand_m31(5, 5, 0x1234);
        let b = rand_m31(5, 5, 0x5678);
        let c = rand_m31(5, 5, 0x9ABC);
        let via_eval = FieldMatrix::<M31>::eval(&a * &b + &c);
        let via_into: FieldMatrix<M31> = (&a * &b + &c).into();
        assert_eq!(via_eval, via_into);
    }

    #[test]
    fn test_product_minus_matrix_fuses_with_neg_proxy() {
        let a = rand_m31(4, 3, 0x2001);
        let b = rand_m31(3, 5, 0x2002);
        let c = rand_m31(4, 5, 0x2003);
        let fused: FieldMatrix<M31> = (&a * &b - &c).into();
        let t: FieldMatrix<M31> = (&a * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(4, 5);
        for r in 0..4 {
            for cc in 0..5 {
                expected.set(r, cc, t.get(r, cc) - c.get(r, cc));
            }
        }
        assert_eq!(fused, expected);
    }

    #[test]
    fn test_transposed_proxy_evaluates_to_transpose() {
        let a = rand_m31(3, 5, 0x3001);
        let t = a.t();
        let t_mat: FieldMatrix<M31> = t.into();
        assert_eq!(t_mat, a.transpose());
    }

    #[test]
    fn test_sum_proxy_transpose_via_matrixlike() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0x4001);
        let b = rand_m31(3, 4, 0x4002);
        let s = &a + &b;
        let t = MatrixLike::transpose(&s);
        let eager: FieldMatrix<M31> = (&a + &b).into();
        assert_eq!(t, eager.transpose());
    }

    #[test]
    fn test_scale_proxy_transpose_via_matrixlike() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0x4101);
        let alpha = M31::new(5);
        let s = alpha * &a;
        let t = MatrixLike::transpose(&s);
        let eager: FieldMatrix<M31> = (alpha * &a).into();
        assert_eq!(t, eager.transpose());
    }

    #[test]
    fn test_neg_proxy_transpose_via_matrixlike() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 4, 0x4201);
        let n = -&a;
        let t = MatrixLike::transpose(&n);
        let eager: FieldMatrix<M31> = (-&a).into();
        assert_eq!(t, eager.transpose());
    }

    #[test]
    fn test_transposed_ref_proxy_transpose_returns_clone() {
        use crate::matrix_like::MatrixLike;
        let a = rand_m31(3, 5, 0x4301);
        let t = a.t(); // Transposed<&FM>
        let tt = MatrixLike::transpose(&t);
        assert_eq!(tt, a);
    }

    // `Scale<F, &FM>` returns `None` from `as_concrete`, which routes
    // evaluation through the generic `MatrixLike` loops.

    #[test]
    fn test_product_scale_operand_uses_gemm_matrixlike() {
        let a = rand_m31(4, 3, 0x5001);
        let b = rand_m31(3, 5, 0x5002);
        let scale_a = M31::new(2) * &a; // Scale<M31, &FM>
        let p = Product(scale_a, &b);
        let result: FieldMatrix<M31> = p.into();
        // Reference: 2·(A·B)
        let ab: FieldMatrix<M31> = (&a * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(4, 5);
        for r in 0..4 {
            for c in 0..5 {
                expected.set(r, c, M31::new(2) * ab.get(r, c));
            }
        }
        assert_eq!(result, expected);
    }

    #[test]
    fn test_transposed_product_scale_a_generic_path() {
        // a is (k=4)×(m=3), b is (k=4)×(n=5), the output is (m=3)×(n=5).
        let a = rand_m31(4, 3, 0x6001);
        let b = rand_m31(4, 5, 0x6002);
        let scale_a = M31::new(3) * &a; // Scale<M31, &FM>, shape (4, 3)
        let tp = TransposedProduct(scale_a, &b);
        let result: FieldMatrix<M31> = tp.into();
        // Reference: (3·A)ᵀ · B = 3 · (Aᵀ · B)
        let at_b: FieldMatrix<M31> = (a.t() * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(3, 5);
        for r in 0..3 {
            for c in 0..5 {
                expected.set(r, c, M31::new(3) * at_b.get(r, c));
            }
        }
        assert_eq!(result, expected);
    }

    #[test]
    fn test_fused_product_plus_transposed_generic_path() {
        let a = rand_m31(4, 3, 0x7001);
        let b = rand_m31(4, 5, 0x7002);
        let c = rand_m31(3, 5, 0x7003);
        let scale_a = M31::new(2) * &a;
        let tp = TransposedProduct(scale_a, &b);
        let fused = FusedProductPlus(tp, &c);
        let result: FieldMatrix<M31> = fused.into();
        // Reference: (2·Aᵀ)·B + C = 2·(Aᵀ·B) + C
        let at_b: FieldMatrix<M31> = (a.t() * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(3, 5);
        for r in 0..3 {
            for c_idx in 0..5 {
                expected.set(r, c_idx, M31::new(2) * at_b.get(r, c_idx) + c.get(r, c_idx));
            }
        }
        assert_eq!(result, expected);
    }

    #[test]
    fn test_scaled_transposed_product_generic_path() {
        let a = rand_m31(4, 3, 0x8001);
        let b = rand_m31(4, 5, 0x8002);
        let inner_scale = M31::new(2) * &a; // Scale<M31, &FM>
        let alpha = M31::new(5);
        let stp = ScaledTransposedProduct(alpha, inner_scale, &b);
        let result: FieldMatrix<M31> = stp.into();
        // Reference: 5 · ((2·Aᵀ)·B) = 10 · (Aᵀ·B)
        let at_b: FieldMatrix<M31> = (a.t() * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(3, 5);
        for r in 0..3 {
            for c in 0..5 {
                expected.set(r, c, M31::new(10) * at_b.get(r, c));
            }
        }
        assert_eq!(result, expected);
    }

    #[test]
    fn test_gemm_matrixlike_empty_m_returns_zero_matrix() {
        let a = rand_m31(0, 3, 0x9001); // 0×3
        let b = rand_m31(3, 4, 0x9002);
        let scale_a = M31::new(2) * &a; // Scale<M31, &FM(0×3)>
        let p = Product(scale_a, &b);
        let result: FieldMatrix<M31> = p.into();
        assert_eq!(result.shape(), (0, 4));
    }

    #[test]
    fn test_gemm_concrete_empty_m_returns_zero_matrix() {
        let a = rand_m31(0, 3, 0x9101); // 0×3
        let b = rand_m31(3, 5, 0x9102);
        let p = Product(&a, &b);
        let result: FieldMatrix<M31> = p.into();
        assert_eq!(result.shape(), (0, 5));
    }

    #[test]
    fn test_gemm_with_beta_k_zero_path() {
        let a = rand_m31(3, 0, 0x9201); // 3×0
        let b = rand_m31(0, 4, 0x9202); // 0×4
        let c = rand_m31(3, 4, 0x9203);
        let beta = M31::new(3);
        // (A·B) + β·C = 0 + β·C = β·C  (A·B is zero because k=0)
        let fused: FieldMatrix<M31> = (&a * &b + beta * &c).into();
        let mut expected = FieldMatrix::<M31>::zeros(3, 4);
        for r in 0..3 {
            for c_idx in 0..4 {
                expected.set(r, c_idx, beta * c.get(r, c_idx));
            }
        }
        assert_eq!(fused, expected);
    }

    #[test]
    fn test_sum_evaluate_into_empty_rows_is_noop() {
        let a = rand_m31(0, 4, 0x9301);
        let b = rand_m31(0, 4, 0x9302);
        let s = &a + &b;
        let result: FieldMatrix<M31> = s.into();
        assert_eq!(result.shape(), (0, 4));
    }

    #[test]
    fn test_transposed_matrixlike_rows_cols_get() {
        let a = rand_m31(4, 3, 0x9401); // a.t() is 3×4
        let _b = rand_m31(3, 5, 0x9402);
        let t = a.t();
        let rows = <_ as crate::matrix_like::MatrixLike<M31>>::rows(&t);
        let cols = <_ as crate::matrix_like::MatrixLike<M31>>::cols(&t);
        let v = <_ as crate::matrix_like::MatrixLike<M31>>::get(&t, 0, 0);
        assert_eq!(rows, 3);
        assert_eq!(cols, 4);
        assert_eq!(v, a.get(0, 0));
        let _tt: FieldMatrix<M31> = <_ as crate::matrix_like::MatrixLike<M31>>::transpose(&t);
    }

    #[test]
    fn test_fused_linear_matrixlike_rows_cols_get_transpose() {
        let a = rand_m31(3, 4, 0x9501);
        let b = rand_m31(3, 4, 0x9502);
        let fl = M31::new(2) * &a + M31::new(3) * &b;
        let rows = <_ as crate::matrix_like::MatrixLike<M31>>::rows(&fl);
        let cols = <_ as crate::matrix_like::MatrixLike<M31>>::cols(&fl);
        assert_eq!(rows, 3);
        assert_eq!(cols, 4);
        let v = <_ as crate::matrix_like::MatrixLike<M31>>::get(&fl, 1, 2);
        assert_eq!(v, M31::new(2) * a.get(1, 2) + M31::new(3) * b.get(1, 2));
        let t: FieldMatrix<M31> = <_ as crate::matrix_like::MatrixLike<M31>>::transpose(&fl);
        assert_eq!(t.shape(), (4, 3));
    }

    #[test]
    fn test_fused_product_plus_product_scale_generic_path() {
        let a = rand_m31(4, 3, 0x9601);
        let b = rand_m31(3, 5, 0x9602);
        let c = rand_m31(4, 5, 0x9603);
        let scale_a = M31::new(2) * &a; // Scale<M31, &FM>, as_concrete = None
        let p = Product(scale_a, &b); // Product<Scale<M31,&FM>, &FM>
        let fused = FusedProductPlus(p, &c);
        let result: FieldMatrix<M31> = fused.into();
        // Reference: 2·(A·B) + C
        let ab: FieldMatrix<M31> = (&a * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(4, 5);
        for r in 0..4 {
            for cc in 0..5 {
                expected.set(r, cc, M31::new(2) * ab.get(r, cc) + c.get(r, cc));
            }
        }
        assert_eq!(result, expected);
    }

    #[test]
    fn test_fused_product_plus_scaled_product_scale_generic_path() {
        let a = rand_m31(4, 3, 0x9701);
        let b = rand_m31(3, 5, 0x9702);
        let c = rand_m31(4, 5, 0x9703);
        let scale_a = M31::new(2) * &a; // Scale<M31, &FM>, as_concrete = None
        let beta = M31::new(3);
        let p = Product(scale_a, &b); // Product<Scale<M31,&FM>, &FM>
        let fused = FusedProductPlusScaled(p, beta * &c);
        let result: FieldMatrix<M31> = fused.into();
        // Reference: 2·(A·B) + 3·C
        let ab: FieldMatrix<M31> = (&a * &b).into();
        let mut expected = FieldMatrix::<M31>::zeros(4, 5);
        for r in 0..4 {
            for cc in 0..5 {
                expected.set(
                    r,
                    cc,
                    M31::new(2) * ab.get(r, cc) + M31::new(3) * c.get(r, cc),
                );
            }
        }
        assert_eq!(result, expected);
    }

    #[test]
    fn test_transposed_product_generic_empty_output() {
        // k = 0: Aᵀ·B has shape (3, 5) and is zero.
        let a = rand_m31(0, 3, 0x9801); // 0×3 → a.t() is 3×0 (k=0)
        let b = rand_m31(0, 5, 0x9802); // 0×5
        let scale_a = M31::new(2) * &a; // Scale<M31, &FM(0×3)>, as_concrete=None
        let tp = TransposedProduct(scale_a, &b);
        let result: FieldMatrix<M31> = tp.into();
        assert_eq!(result.shape(), (3, 5));
        for r in 0..3 {
            for c in 0..5 {
                assert_eq!(result.get(r, c), M31::new(0));
            }
        }
    }
}
