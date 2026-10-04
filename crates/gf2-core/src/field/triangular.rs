//! Block-recursive triangular primitives over matrix views: `trsm`, `trmm`,
//! `trtri` and `trtrm` (`@/citation/DumasPernet2012` §2.1, alg. 2.1–2.4),
//! each operating in place as a reduction to the GEMM kernels of
//! [`crate::field::matrix`]. A dimension at or below the active
//! `triangular.base_case_max_dim()` takes a direct loop. The blocked solves
//! [`trsm_upper_blocked`] and [`trsm_lower_blocked`] follow
//! `@/citation/Higham2002` §14.1.

use crate::field::matrix::{
    gemm_axpy_into_view, gemm_axpy_into_view_diag, gemm_axpy_into_view_tiled, gemm_into_view,
    FieldMatrix, MatView, MatViewMut, ObservationPolicy, RecordObservations, UnitDiag,
    GEMM_COL_TILE, GEMM_ROW_TILE,
};
use crate::field::FiniteField;
use crate::tuning;

#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Conservative default for the triangular profile's
/// `trsm_blocked_min_dim` and `trsm_panel_rows` fields.
pub const TRSM_BLOCKED_PANEL_SIZE: usize = 64;

/// The selected arm of the triangular-solve dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrsmRoute {
    /// Use the recursive triangular solve.
    Recursive,
    /// Use the blocked triangular solve.
    Blocked,
}

/// Reports the triangular-solve arm for a matrix of dimension `n`.
///
/// The comparison uses the active `triangular.trsm_blocked_min_dim()` profile
/// field. Dimensions below it take the recursive path; dimensions at or above
/// it take the blocked path. The conservative default for that field is
/// [`TRSM_BLOCKED_PANEL_SIZE`].
#[must_use]
pub fn trsm_route(n: usize) -> TrsmRoute {
    trsm_route_resolved(tuning::active().triangular().trsm_blocked_min_dim(), n)
}

pub(crate) fn trsm_route_resolved(trsm_blocked_min_dim: usize, n: usize) -> TrsmRoute {
    if n >= trsm_blocked_min_dim {
        TrsmRoute::Blocked
    } else {
        TrsmRoute::Recursive
    }
}

#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_TRSM_PANEL_ROWS: AtomicUsize = AtomicUsize::new(0);

/// Clears the test-support observation of the last blocked TRSM panel width.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_effective_trsm_panel_rows() {
    LAST_EFFECTIVE_TRSM_PANEL_ROWS.store(0, Ordering::Relaxed);
}

/// Returns the panel width observed at the last blocked TRSM callee entry.
///
/// Route tests use it to verify that `triangular.trsm_panel_rows()` reaches
/// the blocked callees, which an equal result does not show.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_effective_trsm_panel_rows() -> Option<usize> {
    match LAST_EFFECTIVE_TRSM_PANEL_ROWS.load(Ordering::Relaxed) {
        0 => None,
        rows => Some(rows),
    }
}

/// Conservative default for `triangular.base_case_max_dim()`: the largest
/// triangular dimension handled by a direct loop rather than another
/// recursive split.
pub(crate) const TRI_BASE_MAX_DIM_DEFAULT: usize = 8;

/// The selected arm of a triangular primitive's base-case dispatcher.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriangularRoute {
    /// Use the direct base-case loop.
    BaseCase,
    /// Continue the recursive triangular algorithm.
    Recursive,
}

/// Reports the triangular base-case arm for dimension `m`.
///
/// The comparison uses the active `triangular.base_case_max_dim()` profile
/// field. Dimensions at or below it take the direct base case; larger
/// dimensions recurse. The conservative default is
/// `TRI_BASE_MAX_DIM_DEFAULT`.
#[must_use]
pub fn triangular_route(m: usize) -> TriangularRoute {
    triangular_route_resolved(tuning::active().triangular().base_case_max_dim(), m)
}

fn triangular_route_resolved(base_case_max_dim: usize, m: usize) -> TriangularRoute {
    if m <= base_case_max_dim {
        TriangularRoute::BaseCase
    } else {
        TriangularRoute::Recursive
    }
}

#[inline(always)]
const fn triangular_route_code(route: TriangularRoute) -> usize {
    match route {
        TriangularRoute::BaseCase => 1,
        TriangularRoute::Recursive => 2,
    }
}

#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_TRIANGULAR_BASE_MAX_DIM: AtomicUsize = AtomicUsize::new(0);

#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_TRIANGULAR_ROUTE: AtomicUsize = AtomicUsize::new(0);

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn record_triangular_route(base_case_max_dim: usize, route: usize) {
    LAST_EFFECTIVE_TRIANGULAR_BASE_MAX_DIM.store(base_case_max_dim, Ordering::Relaxed);
    LAST_EFFECTIVE_TRIANGULAR_ROUTE.store(route, Ordering::Relaxed);
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn record_trsm_panel_rows(rows: usize) {
    LAST_EFFECTIVE_TRSM_PANEL_ROWS.store(rows, Ordering::Relaxed);
}

/// Clears the test-support observation of the last public triangular route.
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_effective_triangular_route() {
    LAST_EFFECTIVE_TRIANGULAR_ROUTE.store(0, Ordering::Relaxed);
    LAST_EFFECTIVE_TRIANGULAR_BASE_MAX_DIM.store(0, Ordering::Relaxed);
}

/// Returns the resolved base-case threshold and top-level route observed by
/// the most recent public triangular operation after the last reset.
///
/// Route reporters remain pure. Each public operation publishes once at its
/// outer boundary, so recursive splits do not add candidate-dependent writes.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_effective_triangular_route() -> Option<(usize, TriangularRoute)> {
    let base_case_max_dim = LAST_EFFECTIVE_TRIANGULAR_BASE_MAX_DIM.load(Ordering::Relaxed);
    if base_case_max_dim == 0 {
        return None;
    }
    let route = match LAST_EFFECTIVE_TRIANGULAR_ROUTE.load(Ordering::Relaxed) {
        1 => TriangularRoute::BaseCase,
        2 => TriangularRoute::Recursive,
        _ => return None,
    };
    Some((base_case_max_dim, route))
}

/// Solves the upper-triangular system `A · X = B` in place, overwriting `b`
/// with `X` (`@/citation/DumasPernet2012` §2.1, alg. 2.1).
///
/// `a` is `m × m` and `b` is `m × n`. Cells strictly below the diagonal of
/// `a` are not read.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
/// * Panics on a zero diagonal element of `a` when `b` has at least one
///   column.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
pub fn trsm_upper<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    trsm_upper_with_policy::<F, RecordObservations>(a, b);
}

pub(crate) fn trsm_upper_with_policy<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_upper: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_upper: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trsm_upper_inner::<F, O>(a, b, base_case_max_dim);
    O::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// Solves the lower-triangular system `A · X = B` in place, overwriting `b`
/// with `X` (`@/citation/DumasPernet2012` §2.1, alg. 2.1).
///
/// `a` is `m × m` and `b` is `m × n`. Cells strictly above the diagonal of
/// `a` are not read.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
/// * Panics on a zero diagonal element of `a` when `b` has at least one
///   column.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
pub fn trsm_lower<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    trsm_lower_with_policy::<F, RecordObservations>(a, b);
}

pub(crate) fn trsm_lower_with_policy<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_lower: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_lower: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trsm_lower_inner::<F, O>(a, b, base_case_max_dim);
    O::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// [`trsm_upper`] as a row-panel blocked solve (`@/citation/Higham2002`
/// §14.1): `a` is tiled into panels of `block_size` rows, processed last to
/// first; each diagonal block is solved recursively and the rows above it
/// are updated by one GEMM.
///
/// `block_size == 0` or `block_size >= m` runs the recursive solve. The
/// profile dispatcher passes `triangular.trsm_panel_rows()`, whose
/// conservative default is [`TRSM_BLOCKED_PANEL_SIZE`].
///
/// # Panics
///
/// Panics under the conditions of [`trsm_upper`].
///
/// # Complexity
///
/// `O(m² · n)` field operations.
pub fn trsm_upper_blocked<F: FiniteField>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    block_size: usize,
) {
    trsm_upper_blocked_with_policy::<F, RecordObservations>(a, b, block_size);
}

pub(crate) fn trsm_upper_blocked_with_policy<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    block_size: usize,
) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_upper_blocked: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_upper_blocked: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trsm_upper_blocked_inner::<F, O>(a, b, block_size, base_case_max_dim);
    O::trsm_panel_rows(block_size);
    O::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// [`trsm_lower`] as a row-panel blocked solve (`@/citation/Higham2002`
/// §14.1): the mirror of [`trsm_upper_blocked`], processing the panels first
/// to last and updating the rows below each.
///
/// # Panics
///
/// Panics under the conditions of [`trsm_lower`].
///
/// # Complexity
///
/// `O(m² · n)` field operations.
pub fn trsm_lower_blocked<F: FiniteField>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    block_size: usize,
) {
    trsm_lower_blocked_with_policy::<F, RecordObservations>(a, b, block_size);
}

pub(crate) fn trsm_lower_blocked_with_policy<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    block_size: usize,
) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trsm_lower_blocked: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trsm_lower_blocked: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trsm_lower_blocked_inner::<F, O>(a, b, block_size, base_case_max_dim);
    O::trsm_panel_rows(block_size);
    O::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// Multiplies `B ← A · B` in place for upper-triangular `A`
/// (`@/citation/DumasPernet2012` §2.1, alg. 2.2).
///
/// `a` is `m × m` and `b` is `m × n`. Cells strictly below the diagonal of
/// `a` are not read.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
pub fn trmm_upper<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trmm_upper: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trmm_upper: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trmm_upper_inner(a, b, base_case_max_dim);
    RecordObservations::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// Multiplies `B ← A · B` in place for lower-triangular `A`
/// (`@/citation/DumasPernet2012` §2.1, alg. 2.2).
///
/// `a` is `m × m` and `b` is `m × n`. Cells strictly above the diagonal of
/// `a` are not read.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics if `a.rows() != b.rows()`.
///
/// # Complexity
///
/// `O(m² · n)` field operations.
pub fn trmm_lower<F: FiniteField>(a: MatView<'_, F>, b: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trmm_lower: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    assert_eq!(
        a.rows(),
        b.rows(),
        "trmm_lower: A.rows ({}) must equal B.rows ({})",
        a.rows(),
        b.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trmm_lower_inner(a, b, base_case_max_dim);
    RecordObservations::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// Inverts an upper-triangular matrix in place
/// (`@/citation/DumasPernet2012` §2.1, alg. 2.3). Cells strictly below the
/// diagonal are not touched.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics on a zero diagonal element of `a`.
///
/// # Complexity
///
/// `O(m³)` field operations.
pub fn trtri_upper<F: FiniteField>(a: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trtri_upper: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trtri_upper_inner(a, base_case_max_dim);
    RecordObservations::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// Inverts a lower-triangular matrix in place
/// (`@/citation/DumasPernet2012` §2.1, alg. 2.3). Cells strictly above the
/// diagonal are not touched.
///
/// # Panics
///
/// * Panics if `a.rows() != a.cols()`.
/// * Panics on a zero diagonal element of `a`.
///
/// # Complexity
///
/// `O(m³)` field operations.
pub fn trtri_lower<F: FiniteField>(a: MatViewMut<'_, F>) {
    assert_eq!(
        a.rows(),
        a.cols(),
        "trtri_lower: A must be square ({}×{})",
        a.rows(),
        a.cols()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, a.rows());
    trtri_lower_inner(a, base_case_max_dim);
    RecordObservations::triangular_route(base_case_max_dim, triangular_route_code(route));
}

/// In-place product `U · L` of an upper-triangular `U` with a unit
/// lower-triangular `L` (`@/citation/DumasPernet2012` §2.1, alg. 2.4),
/// written over the `l` view.
///
/// `l` holds the strictly lower part of `L`: its diagonal is implicitly `1`
/// and is not read, and its strictly upper cells must be zero.
///
/// # Panics
///
/// * Panics if `l.rows() != l.cols()`, `u.rows() != u.cols()`, or
///   `l.rows() != u.rows()`.
///
/// # Complexity
///
/// `O(m³)` field operations.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::triangular::trtrm;
/// use gf2_core::gfp::Fp;
///
/// // L = [[1, 0], [4, 1]]  (unit lower; the diagonal is implicit so we
/// // store 0 in the cell — trtrm will not read it).
/// let mut l_compressed = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// l_compressed.set(1, 0, Fp::<7>::new(4));
/// // U = [[2, 3], [0, 5]].
/// let mut u = FieldMatrix::<Fp<7>>::zeros(2, 2);
/// u.set(0, 0, Fp::<7>::new(2));
/// u.set(0, 1, Fp::<7>::new(3));
/// u.set(1, 1, Fp::<7>::new(5));
///
/// // Compute U · L the slow way for the cross-check.
/// let l_dense = {
///     let mut m = FieldMatrix::<Fp<7>>::identity(2);
///     m.set(1, 0, Fp::<7>::new(4));
///     m
/// };
/// let expected = gemm(&u, &l_dense);
///
/// trtrm(l_compressed.submat_mut(.., ..), u.submat(.., ..));
/// assert_eq!(l_compressed, expected);
/// ```
pub fn trtrm<F: FiniteField>(l: MatViewMut<'_, F>, u: MatView<'_, F>) {
    assert_eq!(
        l.rows(),
        l.cols(),
        "trtrm: L must be square ({}×{})",
        l.rows(),
        l.cols()
    );
    assert_eq!(
        u.rows(),
        u.cols(),
        "trtrm: U must be square ({}×{})",
        u.rows(),
        u.cols()
    );
    assert_eq!(
        l.rows(),
        u.rows(),
        "trtrm: L and U must have equal size ({} vs {})",
        l.rows(),
        u.rows()
    );
    let base_case_max_dim = tuning::active().triangular().base_case_max_dim();
    let route = triangular_route_resolved(base_case_max_dim, l.rows());
    trtrm_inner(l, u, base_case_max_dim);
    RecordObservations::triangular_route(base_case_max_dim, triangular_route_code(route));
}

fn trsm_upper_inner<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    base_case_max_dim: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut b_mut = b;
        trsm_upper_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    let (mut b1_mut, mut b2_mut) = b.split_rows_mut(h);
    trsm_upper_inner::<F, O>(a.submat(h..m, h..m), b2_mut.reborrow(), base_case_max_dim);
    // B1 ← B1 − A12 · X2.
    {
        let a12 = a.submat(0..h, h..m);
        let b2 = b2_mut.as_view();
        let one = a.get(0, 0).one_like();
        let neg_one = -one.clone();
        gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
            neg_one,
            &a12,
            &b2,
            one,
            b1_mut.reborrow(),
        );
    }
    trsm_upper_inner::<F, O>(a.submat(0..h, 0..h), b1_mut, base_case_max_dim);
}

fn trsm_upper_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    for i in (0..m).rev() {
        let pivot = a.get(i, i);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!(
                "trsm_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            )
        });
        for j in 0..n {
            let v = b.get(i, j) * inv.clone();
            b.set(i, j, v);
        }
        for k in 0..i {
            let aki = a.get(k, i);
            for j in 0..n {
                let v = b.get(k, j) - aki.clone() * b.get(i, j);
                b.set(k, j, v);
            }
        }
    }
}

fn trsm_lower_inner<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    base_case_max_dim: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut b_mut = b;
        trsm_lower_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    let (mut b1_mut, mut b2_mut) = b.split_rows_mut(h);
    trsm_lower_inner::<F, O>(a.submat(0..h, 0..h), b1_mut.reborrow(), base_case_max_dim);
    // B2 ← B2 − A21 · X1.
    {
        let a21 = a.submat(h..m, 0..h);
        let b1 = b1_mut.as_view();
        let one = a.get(0, 0).one_like();
        let neg_one = -one.clone();
        gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
            neg_one,
            &a21,
            &b1,
            one,
            b2_mut.reborrow(),
        );
    }
    trsm_lower_inner::<F, O>(a.submat(h..m, h..m), b2_mut, base_case_max_dim);
}

fn trsm_lower_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    for i in 0..m {
        let pivot = a.get(i, i);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!(
                "trsm_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            )
        });
        for j in 0..n {
            let v = b.get(i, j) * inv.clone();
            b.set(i, j, v);
        }
        for k in (i + 1)..m {
            let aki = a.get(k, i);
            for j in 0..n {
                let v = b.get(k, j) - aki.clone() * b.get(i, j);
                b.set(k, j, v);
            }
        }
    }
}

fn trsm_upper_blocked_inner<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    mut b: MatViewMut<'_, F>,
    block_size: usize,
    base_case_max_dim: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if block_size == 0 || m <= block_size {
        trsm_upper_inner::<F, O>(a, b, base_case_max_dim);
        return;
    }
    let bs = block_size;
    let num_blocks = m.div_ceil(bs);
    let one = a.get(0, 0).one_like();
    let neg_one = -one.clone();
    for k in (0..num_blocks).rev() {
        let row_start = k * bs;
        let row_end = m.min((k + 1) * bs);
        let bs_k = row_end - row_start;
        let (mut b_top, mut b_bot) = b.reborrow().split_rows_mut(row_start);
        trsm_upper_inner::<F, O>(
            a.submat(row_start..row_end, row_start..row_end),
            b_bot.submat_mut(0..bs_k, ..),
            base_case_max_dim,
        );
        // B_top -= A[0..row_start, row_start..row_end] · X_panel.
        if row_start > 0 {
            let a_off = a.submat(0..row_start, row_start..row_end);
            let x_panel = b_bot.submat(0..bs_k, ..);
            gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
                neg_one.clone(),
                &a_off,
                &x_panel,
                one.clone(),
                b_top.reborrow(),
            );
        }
    }
}

fn trsm_lower_blocked_inner<F: FiniteField, O: ObservationPolicy>(
    a: MatView<'_, F>,
    mut b: MatViewMut<'_, F>,
    block_size: usize,
    base_case_max_dim: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if block_size == 0 || m <= block_size {
        trsm_lower_inner::<F, O>(a, b, base_case_max_dim);
        return;
    }
    let bs = block_size;
    let num_blocks = m.div_ceil(bs);
    let one = a.get(0, 0).one_like();
    let neg_one = -one.clone();
    for k in 0..num_blocks {
        let row_start = k * bs;
        let row_end = m.min((k + 1) * bs);
        let (mut b_top, mut b_bot) = b.reborrow().split_rows_mut(row_end);
        trsm_lower_inner::<F, O>(
            a.submat(row_start..row_end, row_start..row_end),
            b_top.submat_mut(row_start..row_end, ..),
            base_case_max_dim,
        );
        // B_bot -= A[row_end..m, row_start..row_end] · X_panel.
        if row_end < m {
            let a_off = a.submat(row_end..m, row_start..row_end);
            let x_panel = b_top.submat(row_start..row_end, ..);
            gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
                neg_one.clone(),
                &a_off,
                &x_panel,
                one.clone(),
                b_bot.reborrow(),
            );
        }
    }
}

fn trmm_upper_inner<F: FiniteField>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    base_case_max_dim: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut b_mut = b;
        trmm_upper_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    // B2 stays unmodified until the final recursion, so the fold reads its
    // original value.
    let (mut b1_mut, b2_mut) = b.split_rows_mut(h);
    trmm_upper_inner(a.submat(0..h, 0..h), b1_mut.reborrow(), base_case_max_dim);
    // B1 ← B1 + A12 · B2.
    {
        let a12 = a.submat(0..h, h..m);
        let b2 = b2_mut.as_view();
        let one = a.get(0, 0).one_like();
        gemm_axpy_into_view(one.clone(), &a12, &b2, one, b1_mut.reborrow());
    }
    trmm_upper_inner(a.submat(h..m, h..m), b2_mut, base_case_max_dim);
}

fn trmm_upper_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 {
        return;
    }
    // For upper triangular A and B ← A·B in place: row i depends on
    // rows ≥ i, so rows must be processed top-down. For each (i, j) we
    // accumulate ∑_{k=i}^{m-1} A[i,k]·B[k,j] in a scalar, then write.
    let zero: F = a.get(0, 0).zero_like();
    for i in 0..m {
        for j in 0..n {
            let mut acc = zero.clone();
            for k in i..m {
                acc += a.get(i, k) * b.get(k, j);
            }
            b.set(i, j, acc);
        }
    }
}

fn trmm_lower_inner<F: FiniteField>(
    a: MatView<'_, F>,
    b: MatViewMut<'_, F>,
    base_case_max_dim: usize,
) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 || n == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut b_mut = b;
        trmm_lower_base(&a, &mut b_mut);
        return;
    }
    let h = m / 2;
    // B1 stays unmodified until the final recursion, so the fold reads its
    // original value.
    let (b1_mut, mut b2_mut) = b.split_rows_mut(h);
    trmm_lower_inner(a.submat(h..m, h..m), b2_mut.reborrow(), base_case_max_dim);
    // B2 ← B2 + A21 · B1.
    {
        let a21 = a.submat(h..m, 0..h);
        let b1 = b1_mut.as_view();
        let one = a.get(0, 0).one_like();
        gemm_axpy_into_view(one.clone(), &a21, &b1, one, b2_mut.reborrow());
    }
    trmm_lower_inner(a.submat(0..h, 0..h), b1_mut, base_case_max_dim);
}

fn trmm_lower_base<F: FiniteField>(a: &MatView<'_, F>, b: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    let n = b.cols();
    if m == 0 {
        return;
    }
    // Lower triangular A: row i depends on rows ≤ i, so process rows
    // bottom-up to keep in-place semantics correct.
    let zero: F = a.get(0, 0).zero_like();
    for i in (0..m).rev() {
        for j in 0..n {
            let mut acc = zero.clone();
            for k in 0..=i {
                acc += a.get(i, k) * b.get(k, j);
            }
            b.set(i, j, acc);
        }
    }
}

fn trtri_upper_inner<F: FiniteField>(a: MatViewMut<'_, F>, base_case_max_dim: usize) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut a_mut = a;
        trtri_upper_base(&mut a_mut);
        return;
    }
    let h = m / 2;
    // Split A horizontally at row h. `top` holds rows 0..h (containing
    // A11 in the left h columns and A12 in the right m-h columns).
    // `bot` holds rows h..m (containing A22 in the right m-h columns).
    let (mut top, mut bot) = a.split_rows_mut(h);
    trtri_upper_inner(top.submat_mut(0..h, 0..h), base_case_max_dim);
    trtri_upper_inner(bot.submat_mut(0..(m - h), h..m), base_case_max_dim);
    // A12 ← −A11⁻¹ · A12 · A22⁻¹, through one `h × (m − h)` scratch for the
    // intermediate `A11⁻¹ · A12`.
    let zero: F = top.as_view().get(0, 0).zero_like();
    let mut tmp = FieldMatrix::<F>::new(h, m - h, zero);
    {
        let a11_inv = top.submat(0..h, 0..h);
        let a12_old = top.submat(0..h, h..m);
        gemm_into_view(&a11_inv, &a12_old, tmp.submat_mut(.., ..));
    }
    {
        let a22_inv = bot.submat(0..(m - h), h..m);
        let a12_dst = top.submat_mut(0..h, h..m);
        gemm_into_view(&tmp, &a22_inv, a12_dst);
    }
    let mut a12_dst = top.submat_mut(0..h, h..m);
    for r in 0..h {
        for c in 0..(m - h) {
            let v = a12_dst.get(r, c);
            a12_dst.set(r, c, -v);
        }
    }
}

fn trtri_upper_base<F: FiniteField>(a: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    // For an upper triangular A:
    //   A⁻¹[i, i] = 1 / A[i, i]
    //   A⁻¹[i, j] = − (1 / A[i, i]) · ∑_{k=i+1}^{j} A[i, k] · A⁻¹[k, j]   (i < j)
    // computed from the last column back, so each row's dependencies are
    // already in `inv`. `a` is written only in the final pass.
    let zero: F = a.get(0, 0).zero_like();
    let mut inv = FieldMatrix::<F>::new(m, m, zero.clone());

    // Validate diagonals up front for a clear panic message.
    for i in 0..m {
        if a.get(i, i).is_zero() {
            panic!(
                "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            );
        }
    }
    for j in (0..m).rev() {
        let pivot_inv = a.get(j, j).inv().unwrap_or_else(|| {
            panic!(
                "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                j, j
            )
        });
        inv.set(j, j, pivot_inv.clone());
        for i in (0..j).rev() {
            let mut acc = zero.clone();
            for k in (i + 1)..=j {
                acc += a.get(i, k) * inv.get(k, j);
            }
            let aii_inv = a.get(i, i).inv().unwrap_or_else(|| {
                panic!(
                    "trtri_upper: zero pivot at A[{}, {}] = 0 — matrix is singular",
                    i, i
                )
            });
            inv.set(i, j, -(aii_inv * acc));
        }
    }
    for r in 0..m {
        for c in r..m {
            a.set(r, c, inv.get(r, c));
        }
    }
}

fn trtri_lower_inner<F: FiniteField>(a: MatViewMut<'_, F>, base_case_max_dim: usize) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut a_mut = a;
        trtri_lower_base(&mut a_mut);
        return;
    }
    let h = m / 2;
    // Split A horizontally at row h. `top` holds rows 0..h (containing
    // A11 in the left h columns). `bot` holds rows h..m (containing
    // A21 in the left h columns and A22 in the right m-h columns).
    let (mut top, mut bot) = a.split_rows_mut(h);
    trtri_lower_inner(top.submat_mut(0..h, 0..h), base_case_max_dim);
    trtri_lower_inner(bot.submat_mut(0..(m - h), h..m), base_case_max_dim);
    // A21 ← −A22⁻¹ · A21 · A11⁻¹, through one `(m − h) × h` scratch for the
    // intermediate `A22⁻¹ · A21`.
    let zero: F = bot.as_view().get(0, 0).zero_like();
    let mut tmp = FieldMatrix::<F>::new(m - h, h, zero);
    {
        let a22_inv = bot.submat(0..(m - h), h..m);
        let a21_old = bot.submat(0..(m - h), 0..h);
        gemm_into_view(&a22_inv, &a21_old, tmp.submat_mut(.., ..));
    }
    {
        let a11_inv = top.submat(0..h, 0..h);
        let a21_dst = bot.submat_mut(0..(m - h), 0..h);
        gemm_into_view(&tmp, &a11_inv, a21_dst);
    }
    let mut a21_dst = bot.submat_mut(0..(m - h), 0..h);
    for r in 0..(m - h) {
        for c in 0..h {
            let v = a21_dst.get(r, c);
            a21_dst.set(r, c, -v);
        }
    }
}

fn trtri_lower_base<F: FiniteField>(a: &mut MatViewMut<'_, F>) {
    let m = a.rows();
    if m == 0 {
        return;
    }
    // `inv` stages the result; `a` is written only in the final pass.
    let zero: F = a.get(0, 0).zero_like();
    let mut inv = FieldMatrix::<F>::new(m, m, zero.clone());
    for i in 0..m {
        if a.get(i, i).is_zero() {
            panic!(
                "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                i, i
            );
        }
    }
    // Lower triangular inverse: build from the top-left corner down.
    //   A⁻¹[i, i] = 1 / A[i, i]
    //   A⁻¹[i, j] = − (1 / A[i, i]) · ∑_{k=j}^{i-1} A[i, k] · A⁻¹[k, j]   (i > j)
    for j in 0..m {
        let pivot_inv = a.get(j, j).inv().unwrap_or_else(|| {
            panic!(
                "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                j, j
            )
        });
        inv.set(j, j, pivot_inv);
        for i in (j + 1)..m {
            let mut acc = zero.clone();
            for k in j..i {
                acc += a.get(i, k) * inv.get(k, j);
            }
            let aii_inv = a.get(i, i).inv().unwrap_or_else(|| {
                panic!(
                    "trtri_lower: zero pivot at A[{}, {}] = 0 — matrix is singular",
                    i, i
                )
            });
            inv.set(i, j, -(aii_inv * acc));
        }
    }
    for r in 0..m {
        for c in 0..=r {
            a.set(r, c, inv.get(r, c));
        }
    }
}

/// Body of [`trtrm`]. With
///
/// ```text
///     U = [[U11, U12],   L = [[L11,   0],
///          [  0, U22]]        [L21, L22]]
/// ```
///
/// the blocks of `A = U · L` are
///
/// ```text
///     A11 = U11 · L11 + U12 · L21
///     A12 = U12 · L22
///     A21 = U22 · L21
///     A22 = U22 · L22
/// ```
///
/// written in the order `A12 → A11 → A21 → A22`, so each step reads only
/// intact cells of `L`. `A21` overwrites its own operand `L21` and is staged
/// in one `(m-h) × h` scratch per level.
fn trtrm_inner<F: FiniteField>(l: MatViewMut<'_, F>, u: MatView<'_, F>, base_case_max_dim: usize) {
    let m = l.rows();
    if m == 0 {
        return;
    }
    if triangular_route_resolved(base_case_max_dim, m) == TriangularRoute::BaseCase {
        let mut l_mut = l;
        trtrm_base(&mut l_mut, &u);
        return;
    }
    let h = m / 2;
    // Split L horizontally at row h. `top` carries L11 (left h cols)
    // and the to-be-written A12 (right m-h cols). `bot` carries L21
    // (left h cols, future A21) and L22 / future A22 (right m-h cols).
    let (mut top, mut bot) = l.split_rows_mut(h);

    // A12 = U12 · L22 with the diagonal of `L22` implicit; β = 0 overwrites
    // the destination.
    {
        let u12 = u.submat(0..h, h..m);
        let l22_view = bot.submat(0..(m - h), h..m);
        let one: F = u.get(0, 0).one_like();
        let zero: F = u.get(0, 0).zero_like();
        gemm_axpy_into_view_diag(
            UnitDiag::Stored,
            one,
            &u12,
            UnitDiag::Implicit,
            &l22_view,
            zero,
            top.submat_mut(0..h, h..m),
        );
    }

    // A11 = U11 · L11 + U12 · L21: recurse, then fold `U12 · L21` in. `L21`
    // (in `bot`) is still intact.
    trtrm_inner(
        top.submat_mut(0..h, 0..h),
        u.submat(0..h, 0..h),
        base_case_max_dim,
    );
    {
        let u12 = u.submat(0..h, h..m);
        let l21_view = bot.submat(0..(m - h), 0..h);
        let one = u.get(0, 0).one_like();
        gemm_axpy_into_view(
            one.clone(),
            &u12,
            &l21_view,
            one,
            top.submat_mut(0..h, 0..h),
        );
    }

    // A21 = U22 · L21 overwrites its own operand `L21`, so the product is
    // staged in a scratch and copied back.
    let zero: F = bot.as_view().get(0, 0).zero_like();
    let mut scratch = FieldMatrix::<F>::new(m - h, h, zero);
    {
        let u22 = u.submat(h..m, h..m);
        let l21 = bot.submat(0..(m - h), 0..h);
        gemm_into_view(&u22, &l21, scratch.submat_mut(.., ..));
    }
    {
        let mut a21 = bot.submat_mut(0..(m - h), 0..h);
        for r in 0..(m - h) {
            for c in 0..h {
                a21.set(r, c, scratch.get(r, c));
            }
        }
    }

    // A22 = U22 · L22; `L22` is still intact.
    trtrm_inner(
        bot.submat_mut(0..(m - h), h..m),
        u.submat(h..m, h..m),
        base_case_max_dim,
    );
}

fn trtrm_base<F: FiniteField>(l: &mut MatViewMut<'_, F>, u: &MatView<'_, F>) {
    let m = l.rows();
    if m == 0 {
        return;
    }
    // (U · L)[i, j] = ∑_{k=max(i,j)}^{m-1} U[i, k] · L[k, j] with L[j, j] = 1
    // implicit. Columns run left to right and rows top to bottom: column j is
    // unmodified on entry, and within it the read range k ≥ max(i, j) ≥ i
    // excludes the rows 0..i already written, so no snapshot is needed.
    let one: F = u.get(0, 0).one_like();
    let zero: F = u.get(0, 0).zero_like();
    for j in 0..m {
        for i in 0..m {
            let kmin = i.max(j);
            let mut acc = zero.clone();
            for k in kmin..m {
                let l_kj = if k == j { one.clone() } else { l.get(k, j) };
                acc += u.get(i, k) * l_kj;
            }
            l.set(i, j, acc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::gemm;
    use crate::field::test_random_matrix::{random_fp, random_gf2m_wide_1};
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use proptest::prelude::*;

    // GF(2^8) with the AES reduction polynomial (`@/citation/Nist2001`).
    struct TriGf2m8Cfg;
    impl Gf2mWideConfig<1> for TriGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "TriGf2m8Cfg";
    }
    type TriGf2m8 = Gf2mWide<1, TriGf2m8Cfg>;

    const MERSENNE_31: u64 = 2_147_483_647;

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        random_gf2m_wide_1::<TriGf2m8Cfg>(rows, cols, seed)
    }

    /// Returns an upper-triangular `m × m` matrix with non-zero diagonal.
    /// Cells strictly below the diagonal are zero.
    fn random_upper_fp<const P: u64>(m: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut mat = random_fp::<P>(m, m, seed);
        for r in 0..m {
            for c in 0..r {
                mat.set(r, c, Fp::<P>::new(0));
            }
            if mat.get(r, r) == Fp::<P>::new(0) {
                mat.set(r, r, Fp::<P>::new(1));
            }
        }
        mat
    }

    fn random_lower_fp<const P: u64>(m: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut mat = random_fp::<P>(m, m, seed);
        for r in 0..m {
            for c in (r + 1)..m {
                mat.set(r, c, Fp::<P>::new(0));
            }
            if mat.get(r, r) == Fp::<P>::new(0) {
                mat.set(r, r, Fp::<P>::new(1));
            }
        }
        mat
    }

    fn random_upper_gf2m8(m: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        let mut mat = random_gf2m8(m, m, seed);
        for r in 0..m {
            for c in 0..r {
                mat.set(r, c, TriGf2m8::new([0]));
            }
            if mat.get(r, r) == TriGf2m8::new([0]) {
                mat.set(r, r, TriGf2m8::new([1]));
            }
        }
        mat
    }

    fn random_lower_gf2m8(m: usize, seed: u64) -> FieldMatrix<TriGf2m8> {
        let mut mat = random_gf2m8(m, m, seed);
        for r in 0..m {
            for c in (r + 1)..m {
                mat.set(r, c, TriGf2m8::new([0]));
            }
            if mat.get(r, r) == TriGf2m8::new([0]) {
                mat.set(r, r, TriGf2m8::new([1]));
            }
        }
        mat
    }

    fn check_trsm_upper_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_upper_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(1));
        let mut x = b.clone();
        trsm_upper(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_upper round-trip m={} n={}", m, n);
    }

    fn check_trsm_lower_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_lower_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(2));
        let mut x = b.clone();
        trsm_lower(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_lower round-trip m={} n={}", m, n);
    }

    fn check_trsm_upper_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_upper_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(3));
        let mut x = b.clone();
        trsm_upper(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_upper round-trip gf2m m={} n={}", m, n);
    }

    fn check_trsm_lower_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_lower_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(4));
        let mut x = b.clone();
        trsm_lower(a.submat(.., ..), x.submat_mut(.., ..));
        let recon = gemm(&a, &x);
        assert_eq!(recon, b, "trsm_lower round-trip gf2m m={} n={}", m, n);
    }

    #[test]
    fn test_trsm_upper_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trsm_upper_fp::<7>(m, n, 0x1000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trsm_lower_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trsm_lower_fp::<7>(m, n, 0x2000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trsm_upper_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trsm_upper_fp::<MERSENNE_31>(m, 8, 0x3000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_lower_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trsm_lower_fp::<MERSENNE_31>(m, 8, 0x4000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_upper_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trsm_upper_gf2m(m, 4, 0x5000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_lower_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trsm_lower_gf2m(m, 4, 0x6000 + m as u64);
        }
    }

    #[test]
    fn test_trsm_empty() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let mut b = FieldMatrix::<Fp<7>>::zeros(0, 5);
        trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
        trsm_lower(a.submat(.., ..), b.submat_mut(.., ..));
        let a2 = random_upper_fp::<7>(4, 0x77);
        let mut b2 = FieldMatrix::<Fp<7>>::zeros(4, 0);
        trsm_upper(a2.submat(.., ..), b2.submat_mut(.., ..));
    }

    #[test]
    fn test_trsm_n_one() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(1, 1);
        a.set(0, 0, Fp::<7>::new(3));
        let mut b = FieldMatrix::<Fp<7>>::zeros(1, 3);
        b.set(0, 0, Fp::<7>::new(6));
        b.set(0, 1, Fp::<7>::new(3));
        b.set(0, 2, Fp::<7>::new(1));
        trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
        // 6/3=2, 3/3=1, 1/3 = 1·3⁻¹ = 5 mod 7 (since 3·5=15≡1).
        assert_eq!(b.get(0, 0), Fp::<7>::new(2));
        assert_eq!(b.get(0, 1), Fp::<7>::new(1));
        assert_eq!(b.get(0, 2), Fp::<7>::new(5));
    }

    #[test]
    #[should_panic(expected = "trsm_upper: zero pivot")]
    fn test_trsm_upper_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        a.set(0, 0, Fp::<7>::new(1));
        a.set(0, 1, Fp::<7>::new(2));
        // a.set(1, 1, 0) — singular.
        let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
        b.set(0, 0, Fp::<7>::new(1));
        b.set(1, 0, Fp::<7>::new(2));
        trsm_upper(a.submat(.., ..), b.submat_mut(.., ..));
    }

    #[test]
    #[should_panic(expected = "trsm_lower: zero pivot")]
    fn test_trsm_lower_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        // a.set(0, 0, 0) — singular.
        a.set(1, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(3));
        let mut b = FieldMatrix::<Fp<7>>::zeros(2, 1);
        b.set(0, 0, Fp::<7>::new(1));
        b.set(1, 0, Fp::<7>::new(2));
        trsm_lower(a.submat(.., ..), b.submat_mut(.., ..));
    }

    fn check_trmm_upper_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_upper_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(11));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_upper(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_upper m={} n={}", m, n);
    }

    fn check_trmm_lower_fp<const P: u64>(m: usize, n: usize, seed: u64) {
        let a = random_lower_fp::<P>(m, seed);
        let b = random_fp::<P>(m, n, seed.wrapping_add(13));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_lower(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_lower m={} n={}", m, n);
    }

    fn check_trmm_upper_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_upper_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(15));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_upper(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_upper gf2m m={} n={}", m, n);
    }

    fn check_trmm_lower_gf2m(m: usize, n: usize, seed: u64) {
        let a = random_lower_gf2m8(m, seed);
        let b = random_gf2m8(m, n, seed.wrapping_add(17));
        let expected = gemm(&a, &b);
        let mut got = b.clone();
        trmm_lower(a.submat(.., ..), got.submat_mut(.., ..));
        assert_eq!(got, expected, "trmm_lower gf2m m={} n={}", m, n);
    }

    #[test]
    fn test_trmm_upper_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trmm_upper_fp::<7>(m, n, 0x7000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trmm_lower_small_fp7() {
        for &m in &[1usize, 3, 5, 7, 16, 32, 33, 64] {
            for &n in &[1usize, 3, 5] {
                check_trmm_lower_fp::<7>(m, n, 0x8000 + (m as u64) * 13 + n as u64);
            }
        }
    }

    #[test]
    fn test_trmm_upper_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trmm_upper_fp::<MERSENNE_31>(m, 4, 0x9000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_lower_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trmm_lower_fp::<MERSENNE_31>(m, 4, 0xA000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_upper_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trmm_upper_gf2m(m, 4, 0xB000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_lower_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trmm_lower_gf2m(m, 4, 0xC000 + m as u64);
        }
    }

    #[test]
    fn test_trmm_empty() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let mut b = FieldMatrix::<Fp<7>>::zeros(0, 5);
        trmm_upper(a.submat(.., ..), b.submat_mut(.., ..));
        trmm_lower(a.submat(.., ..), b.submat_mut(.., ..));
    }

    fn check_trtri_upper_fp<const P: u64>(m: usize, seed: u64) {
        let a = random_upper_fp::<P>(m, seed);
        let mut a_inv = a.clone();
        trtri_upper(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let id = FieldMatrix::<Fp<P>>::identity(m);
        assert_eq!(prod, id, "trtri_upper A·A⁻¹ = I, m={}", m);
    }

    fn check_trtri_lower_fp<const P: u64>(m: usize, seed: u64) {
        let a = random_lower_fp::<P>(m, seed);
        let mut a_inv = a.clone();
        trtri_lower(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let id = FieldMatrix::<Fp<P>>::identity(m);
        assert_eq!(prod, id, "trtri_lower A·A⁻¹ = I, m={}", m);
    }

    fn check_trtri_upper_gf2m(m: usize, seed: u64) {
        let a = random_upper_gf2m8(m, seed);
        let mut a_inv = a.clone();
        trtri_upper(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let mut id = FieldMatrix::<TriGf2m8>::zeros(m, m);
        for i in 0..m {
            id.set(i, i, TriGf2m8::new([1]));
        }
        assert_eq!(prod, id, "trtri_upper A·A⁻¹ = I gf2m, m={}", m);
    }

    fn check_trtri_lower_gf2m(m: usize, seed: u64) {
        let a = random_lower_gf2m8(m, seed);
        let mut a_inv = a.clone();
        trtri_lower(a_inv.submat_mut(.., ..));
        let prod = gemm(&a, &a_inv);
        let mut id = FieldMatrix::<TriGf2m8>::zeros(m, m);
        for i in 0..m {
            id.set(i, i, TriGf2m8::new([1]));
        }
        assert_eq!(prod, id, "trtri_lower A·A⁻¹ = I gf2m, m={}", m);
    }

    #[test]
    fn test_trtri_upper_small_fp7() {
        for &m in &[1usize, 2, 3, 5, 7, 16, 32, 33, 64] {
            check_trtri_upper_fp::<7>(m, 0xD000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_lower_small_fp7() {
        for &m in &[1usize, 2, 3, 5, 7, 16, 32, 33, 64] {
            check_trtri_lower_fp::<7>(m, 0xE000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_upper_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trtri_upper_fp::<MERSENNE_31>(m, 0xF000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_lower_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trtri_lower_fp::<MERSENNE_31>(m, 0x10000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_upper_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trtri_upper_gf2m(m, 0x11000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_lower_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trtri_lower_gf2m(m, 0x12000 + m as u64);
        }
    }

    #[test]
    fn test_trtri_empty() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(0, 0);
        trtri_upper(a.submat_mut(.., ..));
        trtri_lower(a.submat_mut(.., ..));
    }

    #[test]
    #[should_panic(expected = "trtri_upper: zero pivot")]
    fn test_trtri_upper_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        a.set(0, 0, Fp::<7>::new(1));
        a.set(0, 1, Fp::<7>::new(2));
        // a[1, 1] = 0 — singular.
        trtri_upper(a.submat_mut(.., ..));
    }

    #[test]
    #[should_panic(expected = "trtri_lower: zero pivot")]
    fn test_trtri_lower_singular_panics() {
        let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
        // a[0, 0] = 0 — singular.
        a.set(1, 0, Fp::<7>::new(2));
        a.set(1, 1, Fp::<7>::new(3));
        trtri_lower(a.submat_mut(.., ..));
    }

    fn check_trtrm_fp<const P: u64>(m: usize, seed: u64) {
        let mut l = random_fp::<P>(m, m, seed);
        for r in 0..m {
            for c in r..m {
                l.set(r, c, Fp::<P>::new(0));
            }
        }
        let mut u = random_fp::<P>(m, m, seed.wrapping_add(7));
        for r in 0..m {
            for c in 0..r {
                u.set(r, c, Fp::<P>::new(0));
            }
        }
        let l_dense = {
            let mut tmp = l.clone();
            for d in 0..m {
                tmp.set(d, d, Fp::<P>::new(1));
            }
            tmp
        };
        let expected = gemm(&u, &l_dense);
        let mut got = l.clone();
        trtrm(got.submat_mut(.., ..), u.submat(.., ..));
        assert_eq!(got, expected, "trtrm m={}", m);
    }

    fn check_trtrm_gf2m(m: usize, seed: u64) {
        let mut l = random_gf2m8(m, m, seed);
        for r in 0..m {
            for c in r..m {
                l.set(r, c, TriGf2m8::new([0]));
            }
        }
        let mut u = random_gf2m8(m, m, seed.wrapping_add(7));
        for r in 0..m {
            for c in 0..r {
                u.set(r, c, TriGf2m8::new([0]));
            }
        }
        let l_dense = {
            let mut tmp = l.clone();
            for d in 0..m {
                tmp.set(d, d, TriGf2m8::new([1]));
            }
            tmp
        };
        let expected = gemm(&u, &l_dense);
        let mut got = l.clone();
        trtrm(got.submat_mut(.., ..), u.submat(.., ..));
        assert_eq!(got, expected, "trtrm gf2m m={}", m);
    }

    #[test]
    fn test_trtrm_small_fp7() {
        for &m in &[1usize, 2, 3, 5, 7, 16, 32, 33, 64] {
            check_trtrm_fp::<7>(m, 0x13000 + m as u64);
        }
    }

    #[test]
    fn test_trtrm_mersenne_31() {
        for &m in &[16usize, 32, 33, 65] {
            check_trtrm_fp::<MERSENNE_31>(m, 0x14000 + m as u64);
        }
    }

    #[test]
    fn test_trtrm_gf2m8() {
        for &m in &[1usize, 3, 16, 33, 65] {
            check_trtrm_gf2m(m, 0x15000 + m as u64);
        }
    }

    #[test]
    fn test_trtrm_empty() {
        let mut l = FieldMatrix::<Fp<7>>::zeros(0, 0);
        let u = FieldMatrix::<Fp<7>>::zeros(0, 0);
        trtrm(l.submat_mut(.., ..), u.submat(.., ..));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        #[test]
        fn prop_trsm_upper_fp7(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_upper_fp::<7>(m, seed);
            let b = random_fp::<7>(m, n, seed.wrapping_add(0x71));
            let mut x = b.clone();
            trsm_upper(a.submat(.., ..), x.submat_mut(.., ..));
            let recon = gemm(&a, &x);
            prop_assert_eq!(recon, b);
        }

        #[test]
        fn prop_trsm_lower_fp7(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_lower_fp::<7>(m, seed);
            let b = random_fp::<7>(m, n, seed.wrapping_add(0x72));
            let mut x = b.clone();
            trsm_lower(a.submat(.., ..), x.submat_mut(.., ..));
            let recon = gemm(&a, &x);
            prop_assert_eq!(recon, b);
        }

        #[test]
        fn prop_trmm_upper_fp31(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_upper_fp::<MERSENNE_31>(m, seed);
            let b = random_fp::<MERSENNE_31>(m, n, seed.wrapping_add(0x73));
            let expected = gemm(&a, &b);
            let mut got = b.clone();
            trmm_upper(a.submat(.., ..), got.submat_mut(.., ..));
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_trmm_lower_fp31(m in 1usize..40, n in 1usize..6, seed in 0u64..256) {
            let a = random_lower_fp::<MERSENNE_31>(m, seed);
            let b = random_fp::<MERSENNE_31>(m, n, seed.wrapping_add(0x74));
            let expected = gemm(&a, &b);
            let mut got = b.clone();
            trmm_lower(a.submat(.., ..), got.submat_mut(.., ..));
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_trtri_upper_fp7(m in 1usize..40, seed in 0u64..256) {
            let a = random_upper_fp::<7>(m, seed);
            let mut a_inv = a.clone();
            trtri_upper(a_inv.submat_mut(.., ..));
            let prod = gemm(&a, &a_inv);
            let id = FieldMatrix::<Fp<7>>::identity(m);
            prop_assert_eq!(prod, id);
        }

        #[test]
        fn prop_trtri_lower_fp7(m in 1usize..40, seed in 0u64..256) {
            let a = random_lower_fp::<7>(m, seed);
            let mut a_inv = a.clone();
            trtri_lower(a_inv.submat_mut(.., ..));
            let prod = gemm(&a, &a_inv);
            let id = FieldMatrix::<Fp<7>>::identity(m);
            prop_assert_eq!(prod, id);
        }

        #[test]
        fn prop_trtrm_fp7(m in 1usize..40, seed in 0u64..256) {
            let mut l = random_fp::<7>(m, m, seed);
            for r in 0..m {
                for c in r..m {
                    l.set(r, c, Fp::<7>::new(0));
                }
            }
            let mut u = random_fp::<7>(m, m, seed.wrapping_add(0x75));
            for r in 0..m {
                for c in 0..r {
                    u.set(r, c, Fp::<7>::new(0));
                }
            }
            let l_dense = {
                let mut tmp = l.clone();
                for d in 0..m {
                    tmp.set(d, d, Fp::<7>::new(1));
                }
                tmp
            };
            let expected = gemm(&u, &l_dense);
            let mut got = l.clone();
            trtrm(got.submat_mut(.., ..), u.submat(.., ..));
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn prop_trtrm_gf2m8(m in 1usize..30, seed in 0u64..128) {
            let mut l = random_gf2m8(m, m, seed);
            for r in 0..m {
                for c in r..m {
                    l.set(r, c, TriGf2m8::new([0]));
                }
            }
            let mut u = random_gf2m8(m, m, seed.wrapping_add(0x76));
            for r in 0..m {
                for c in 0..r {
                    u.set(r, c, TriGf2m8::new([0]));
                }
            }
            let l_dense = {
                let mut tmp = l.clone();
                for d in 0..m {
                    tmp.set(d, d, TriGf2m8::new([1]));
                }
                tmp
            };
            let expected = gemm(&u, &l_dense);
            let mut got = l.clone();
            trtrm(got.submat_mut(.., ..), u.submat(.., ..));
            prop_assert_eq!(got, expected);
        }
    }

    const TRSM_BOUNDARY_LENS: &[usize] = &[1, 15, 16, 17, 63, 64, 65];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(8))]

        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp7(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<7>(m, mseed);
                    let b = random_fp::<7>(m, n_rhs, mseed.wrapping_add(0xB1));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<7> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp7(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<7>(m, mseed);
                    let b = random_fp::<7>(m, n_rhs, mseed.wrapping_add(0xB2));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<7> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp31(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<31>(m, mseed);
                    let b = random_fp::<31>(m, n_rhs, mseed.wrapping_add(0xB3));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<31> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp31(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<31>(m, mseed);
                    let b = random_fp::<31>(m, n_rhs, mseed.wrapping_add(0xB4));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<31> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp127(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<127>(m, mseed);
                    let b = random_fp::<127>(m, n_rhs, mseed.wrapping_add(0xB5));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<127> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp127(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<127>(m, mseed);
                    let b = random_fp::<127>(m, n_rhs, mseed.wrapping_add(0xB6));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<127> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp241(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<241>(m, mseed);
                    let b = random_fp::<241>(m, n_rhs, mseed.wrapping_add(0xB7));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<241> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp241(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<241>(m, mseed);
                    let b = random_fp::<241>(m, n_rhs, mseed.wrapping_add(0xB8));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<241> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp251(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<251>(m, mseed);
                    let b = random_fp::<251>(m, n_rhs, mseed.wrapping_add(0xB9));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<251> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp251(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<251>(m, mseed);
                    let b = random_fp::<251>(m, n_rhs, mseed.wrapping_add(0xBA));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<251> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_upper_boundary_sweep_fp65521(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_upper_fp::<65521>(m, mseed);
                    let b = random_fp::<65521>(m, n_rhs, mseed.wrapping_add(0xBB));
                    let mut x_scalar = b.clone();
                    trsm_upper(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_upper_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "upper Fp<65521> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }

        #[test]
        fn prop_blocked_trsm_lower_boundary_sweep_fp65521(seed in 0u64..1_000_000) {
            for &m in TRSM_BOUNDARY_LENS {
                for &n_rhs in TRSM_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n_rhs as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_lower_fp::<65521>(m, mseed);
                    let b = random_fp::<65521>(m, n_rhs, mseed.wrapping_add(0xBC));
                    let mut x_scalar = b.clone();
                    trsm_lower(a.submat(.., ..), x_scalar.submat_mut(.., ..));
                    let mut x_blocked = b.clone();
                    trsm_lower_blocked(a.submat(.., ..), x_blocked.submat_mut(.., ..), TRSM_BLOCKED_PANEL_SIZE);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "lower Fp<65521> mismatch m={} n_rhs={}", m, n_rhs);
                }
            }
        }
    }

    #[test]
    fn test_view_helpers_roundtrip() {
        let m = FieldMatrix::<Fp<7>>::identity(3);
        let v = m.submat(.., ..);
        let owned = v.to_owned();
        assert_eq!(owned, m);
    }

    // Counts hold with no tuning profile installed
    // (`triangular.base_case_max_dim()` = 8). Each `gemm_axpy_into_view` or
    // `gemm_into_view` call adds 2 for its transposed operand;
    // `gemm_axpy_into_view_diag` adds 0.
    use crate::field::matrix::{fieldmatrix_new_count, reset_fieldmatrix_new_count};
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_trsm_zero_allocation() {
        let m = 65;
        let n = 6;
        // 8 internal-recursion gemm calls × 2 alloc-counter bumps per gemm
        // (the `MatView::transpose` path) at threshold = 8.
        const EXPECTED: u64 = 16;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA0FC);
        let b_upper = random_fp::<MERSENNE_31>(m, n, 0xA0FD);
        let mut x_upper = b_upper.clone();
        reset_fieldmatrix_new_count();
        trsm_upper(a_upper.submat(.., ..), x_upper.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trsm_upper at m={} expected {} FieldMatrix-class allocs (8 gemm calls × 2 \
             bumps each: to_owned + transpose in `MatView::transpose`); got {}",
            m, EXPECTED, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA0FE);
        let b_lower = random_fp::<MERSENNE_31>(m, n, 0xA0FF);
        let mut x_lower = b_lower.clone();
        reset_fieldmatrix_new_count();
        trsm_lower(a_lower.submat(.., ..), x_lower.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trsm_lower at m={} expected {} FieldMatrix-class allocs; got {}",
            m, EXPECTED, allocs
        );
    }

    #[test]
    #[serial]
    fn test_trmm_zero_allocation() {
        let m = 65;
        let n = 6;
        const EXPECTED: u64 = 16;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA1FC);
        let b = random_fp::<MERSENNE_31>(m, n, 0xA1FD);
        let mut got_upper = b.clone();
        reset_fieldmatrix_new_count();
        trmm_upper(a_upper.submat(.., ..), got_upper.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trmm_upper at m={} expected {} FieldMatrix-class allocs (8 gemm calls × 2 \
             bumps each: to_owned + transpose in `MatView::transpose`); got {}",
            m, EXPECTED, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA1FE);
        let mut got_lower = b.clone();
        reset_fieldmatrix_new_count();
        trmm_lower(a_lower.submat(.., ..), got_lower.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trmm_lower at m={} expected {} FieldMatrix-class allocs; got {}",
            m, EXPECTED, allocs
        );
    }

    #[test]
    #[serial]
    fn test_trtri_allocation_budget() {
        let m = 64;
        // 8 leaf inv buffers + 7 chain levels × (1 chain scratch + 2 gemms × 2
        // transpose bumps).
        const EXPECTED: u64 = 43;
        let a_upper = random_upper_fp::<MERSENNE_31>(m, 0xA2FC);
        let mut a_inv = a_upper.clone();
        reset_fieldmatrix_new_count();
        trtri_upper(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trtri_upper at m={} expected {} FieldMatrix-class allocs \
             (8 leaf inv + 7 chain levels × (1 scratch + 2 gemms × 2 transpose bumps)); got {}",
            m, EXPECTED, allocs
        );

        let a_lower = random_lower_fp::<MERSENNE_31>(m, 0xA2FD);
        let mut a_inv = a_lower.clone();
        reset_fieldmatrix_new_count();
        trtri_lower(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trtri_lower at m={} expected {} FieldMatrix-class allocs; got {}",
            m, EXPECTED, allocs
        );
    }

    #[test]
    #[serial]
    fn test_trtri_at_threshold_one_allocation() {
        let m = TRI_BASE_MAX_DIM_DEFAULT;
        let a = random_upper_fp::<MERSENNE_31>(m, 0xA3FC);
        let mut a_inv = a.clone();
        reset_fieldmatrix_new_count();
        trtri_upper(a_inv.submat_mut(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, 1,
            "trtri_upper at base-case size (m={}) expected 1 FieldMatrix::new \
             (the inv buffer; column-by-column writes scalars in place); got {}",
            m, allocs
        );
    }

    #[test]
    #[serial]
    fn test_trtrm_allocation_budget() {
        let m = 64;
        // 7 non-leaf levels × 5 allocs/level (the `U12 · L21` fold, the `A21`
        // scratch and its `gemm_into_view` transpose).
        const EXPECTED: u64 = 35;
        let mut l = random_fp::<MERSENNE_31>(m, m, 0xA4FC);
        for r in 0..m {
            for c in r..m {
                l.set(r, c, Fp::<MERSENNE_31>::new(0));
            }
        }
        let mut u = random_fp::<MERSENNE_31>(m, m, 0xA4FD);
        for r in 0..m {
            for c in 0..r {
                u.set(r, c, Fp::<MERSENNE_31>::new(0));
            }
        }
        reset_fieldmatrix_new_count();
        trtrm(l.submat_mut(.., ..), u.submat(.., ..));
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED,
            "trtrm at m={} expected {} FieldMatrix-class allocs (7 non-leaf levels × \
             5 allocs/level: step 2b gemm_axpy + step 3 chain scratch + step 3 \
             gemm_into_view's MatView::transpose); got {}",
            m, EXPECTED, allocs
        );
    }

    #[test]
    fn test_recursive_split_just_above_threshold() {
        let m = TRI_BASE_MAX_DIM_DEFAULT + 1;
        check_trsm_upper_fp::<MERSENNE_31>(m, 4, 0xABCD);
        check_trsm_lower_fp::<MERSENNE_31>(m, 4, 0xABCE);
        check_trmm_upper_fp::<MERSENNE_31>(m, 4, 0xABCF);
        check_trmm_lower_fp::<MERSENNE_31>(m, 4, 0xABD0);
        check_trtri_upper_fp::<MERSENNE_31>(m, 0xABD1);
        check_trtri_lower_fp::<MERSENNE_31>(m, 0xABD2);
        check_trtrm_fp::<MERSENNE_31>(m, 0xABD3);
    }
}
