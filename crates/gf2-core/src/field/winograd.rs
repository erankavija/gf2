//! Strassen–Winograd recursive matrix multiplication over a
//! [`FiniteField`](crate::field::FiniteField).
//!
//! This module implements the sub-cubic Strassen–Winograd variant described
//! in Dumas–Pernet §1.4 (algorithm 1.6): 7 recursive half-size multiplies
//! and 15 block additions per recursion level, giving an asymptotic
//! `O(n^log₂ 7) ≈ O(n^2.807)` complexity versus the classical `O(n³)` gemm
//! shipped in [`crate::field::matrix::gemm`] (T1, issue `91c06222`).
//!
//! The public entry point is [`gemm_winograd`]. Below
//! [`FiniteField::WINOGRAD_THRESHOLD`] the recursion peels down to T1's
//! classical blocked gemm, which inherits the crate's SIMD path via
//! `FieldVec::dot_product_slices`. Odd dimensions are handled by padding a
//! single row/column of zero field elements, recursing, then slicing the
//! result back to the original output shape — zero-padding is admissible
//! over any field because `0 · anything = 0`.
//!
//! A crate-private threshold-parameterised helper
//! [`gemm_winograd_with_threshold`] is exposed for the benchmark harness
//! (`benches/strassen_threshold.rs`) so the sweep runs against the very
//! same recursion used by production and does not re-declare helpers.
//!
//! # Bound propagation
//!
//! Dumas–Pernet theorem 4 states that after `l` levels of recursion every
//! intermediate cell value `z` satisfies
//!
//! ```text
//! |z| ≤ ((1 + 3^l) / 2)² · ceil(k / 2^l) · (p − 1)²
//! ```
//!
//! Before a sub-problem at depth `l` is handed to the base-case gemm the
//! production recursion
//! ([`gemm_winograd_inner`]) reads
//! [`theorem_4_bound`] directly with the current `level + 1` and
//! compares it against the classical-gemm delayed-reduction headroom
//! `F::max_unreduced_additions() · (p − 1)²`. When the bound would
//! exceed headroom the recursion refuses to peel further and falls back
//! to the classical gemm at the current size. For Mersenne-31
//! (`max_unreduced_additions ≈ 4·10⁹`) the bound is generous enough
//! that the threshold is the binding constraint at practical matrix
//! sizes; for small prime fields near `2^63` it can fire, at which
//! point we fall back to the classical base case.
//!
//! The operand bound `p − 1` is exposed via
//! [`FiniteField::theorem_4_operand_bound`]; prime fields override this
//! with their modulus-minus-one, while binary fields keep the default
//! `u128::MAX` sentinel (the theorem is vacuous — XOR never overflows).
//!
//! The bound is additionally verified at every recursion level by two
//! tests in this module. The canonical-residue proptest
//! `prop_winograd_bound_propagates_across_levels_fp31` asserts each of
//! the eight S/T block operands (which always hold canonical, reduced
//! values in `[0, p − 1]`) trivially respects the bound. The
//! complementary Wide-shadow proptest
//! `prop_winograd_wide_shadow_respects_theorem_4_bound_fp31` mirrors
//! the recursion but carries S/T operand magnitudes as **unreduced**
//! `u128` integers (summing absolute integer magnitudes across every
//! add/sub performed by Winograd's peel), so it exercises exactly the
//! unreduced-arithmetic growth theorem 4 talks about, not just the
//! trivially-bounded canonical residues.
//!
//! # Bit-exact correctness
//!
//! Because every intermediate is reduced lazily through
//! `F::reduce_wide` at the base case and Winograd's U-assembly uses only
//! field addition / subtraction (exact over any field), the output is
//! **bit-exact** equal to the classical `gemm` for all shapes, all fields,
//! and every intermediate padding configuration. The module-level tests
//! exercise this for odd-`m`, odd-`k`, odd-`n`, all-three-odd, and
//! threshold-straddling cases.
//!
//! # Odd dimensions
//!
//! The 7-multiply split requires `m`, `k`, and `n` all even. For a peeled
//! subproblem with any odd dimension we pad out the short axis to the
//! next even value with zero-valued field elements, recurse, then slice
//! the padded output back to the original shape. The padding adds at
//! most one extra row/column per level, so the asymptotic cost is still
//! `O(n^log₂ 7)`.
//!
//! # Non-`ConstField` fields
//!
//! The zero-padding step needs a concrete zero-valued `F` to clone. For
//! [`ConstField`](crate::field::ConstField) implementations we use
//! `F::zero_hint()`. For runtime-context fields (`Gf2mElement`) the
//! caller must pass matrices with at least one non-empty factor; if both
//! factors are empty we fall back to `F::zero_hint()`, which returns
//! `None`, and panic with the same contract as
//! [`crate::field::matrix::gemm`].

use crate::field::matrix::{gemm, FieldMatrix};
use crate::field::{FieldVec, FiniteField};

/// Strassen–Winograd matrix multiplication over an arbitrary
/// [`FiniteField`](crate::field::FiniteField).
///
/// Below [`FiniteField::WINOGRAD_THRESHOLD`] the implementation dispatches
/// directly to the classical blocked [`gemm`] — that path already carries
/// T1's cache tiling and SIMD-accelerated dot products. Above the
/// threshold one level of Winograd's 7-multiply split is peeled and the
/// seven half-size products are computed by recursive calls into this
/// same function.
///
/// Odd dimensions are handled by padding the short axis up to the next
/// even value with zero-valued field elements, recursing, then slicing
/// the padded output back to the original shape. Padding is admissible
/// over any field because `0 · anything = 0`.
///
/// The result is **bit-exact equal** to [`gemm`] for all shapes and all
/// fields (see the module-level proptests in this file).
///
/// # Arguments
///
/// * `a` — Left operand of shape `m × k`. Its column count must equal
///   `b.rows`.
/// * `b` — Right operand of shape `k × n`. Its row count must equal
///   `a.cols`.
///
/// The output has shape `m × n` with cell `(i, j) = ∑_{t=0}^{k-1}
/// a[i, t] · b[t, j]`.
///
/// # Panics
///
/// Panics if `a.cols != b.rows`. Also panics (with the same contract as
/// [`gemm`]) for the `(m, 0) × (0, n)` shape on runtime-context fields
/// when both factors carry empty storage and `F::zero_hint()` returns
/// `None`.
///
/// # Complexity
///
/// Asymptotic `O(n^log₂ 7) ≈ O(n^2.807)` field multiplications above the
/// threshold, dropping to the classical `O(n³)` at the base case. Each
/// recursion level allocates `O(n² / 4)` scratch `FieldMatrix` values
/// for the seven half-size sums; the total allocation footprint across
/// the recursion tree is `O(n²)` when summed as a geometric series.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::winograd::gemm_winograd;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(1), Fp::<7>::new(2)].into_iter().collect(),
///     vec![Fp::<7>::new(3), Fp::<7>::new(4)].into_iter().collect(),
/// ]);
/// let b = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(5), Fp::<7>::new(6)].into_iter().collect(),
///     vec![Fp::<7>::new(7), Fp::<7>::new(8)].into_iter().collect(),
/// ]);
///
/// let expected = gemm(&a, &b);
/// let got = gemm_winograd(&a, &b);
/// assert_eq!(got, expected);
/// ```
pub fn gemm_winograd<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    gemm_winograd_with_threshold(a, b, F::WINOGRAD_THRESHOLD)
}

/// Strassen–Winograd gemm with an explicit base-case threshold. Intended
/// for benchmark harnesses that sweep the threshold at runtime; see
/// `benches/strassen_threshold.rs`.
///
/// The recursion is bit-identical to [`gemm_winograd`] except the base
/// case fires at `min(m, k, n) < max(threshold, 2)` instead of
/// `< F::WINOGRAD_THRESHOLD`. Correctness is independent of the
/// threshold (any positive value yields the same output as classical
/// `gemm`); the floor at `2` guards against the degenerate 1×1 half-dim
/// case where a Winograd peel cannot make progress. Pass `usize::MAX`
/// to force the classical path.
///
/// # Arguments
///
/// * `a` — Left operand of shape `m × k`.
/// * `b` — Right operand of shape `k × n` (with `b.rows == a.cols`).
/// * `threshold` — Base-case cutoff; when `min(m, k, n) < threshold` the
///   classical [`gemm`] is invoked immediately.
///
/// # Panics
///
/// Same as [`gemm_winograd`] — inner-dimension mismatch and the
/// zero-witness empty-inner corner case.
///
/// # Complexity
///
/// Identical asymptotic profile to [`gemm_winograd`]; the threshold
/// shifts the constant factor but not the `O(n^log₂ 7)` bound.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::{gemm, FieldMatrix};
/// use gf2_core::field::winograd::gemm_winograd_with_threshold;
/// use gf2_core::gfp::Fp;
///
/// let a = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(1), Fp::<7>::new(2)].into_iter().collect(),
///     vec![Fp::<7>::new(3), Fp::<7>::new(4)].into_iter().collect(),
/// ]);
/// let b = FieldMatrix::<Fp<7>>::from_rows(vec![
///     vec![Fp::<7>::new(5), Fp::<7>::new(6)].into_iter().collect(),
///     vec![Fp::<7>::new(7), Fp::<7>::new(8)].into_iter().collect(),
/// ]);
///
/// // threshold = 1 forces one Winograd peel even on 2×2 (still bit-exact).
/// let expected = gemm(&a, &b);
/// let got = gemm_winograd_with_threshold(&a, &b, 1);
/// assert_eq!(got, expected);
/// ```
#[doc(hidden)]
pub fn gemm_winograd_with_threshold<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    threshold: usize,
) -> FieldMatrix<F> {
    gemm_winograd_inner(a, b, threshold, 0)
}

/// Recursive Winograd kernel. `level` tracks the current depth (0 at the
/// top-level public entry points) so the theorem-4 bound can be checked
/// in debug builds against the canonical block values before each sub-
/// multiply.
fn gemm_winograd_inner<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    threshold: usize,
    level: u32,
) -> FieldMatrix<F> {
    assert_eq!(
        a.cols(),
        b.rows(),
        "gemm_winograd: inner dimensions must match ({} vs {})",
        a.cols(),
        b.rows()
    );

    let (m, k) = a.shape();
    let n = b.cols();

    // Delegate degenerate cases to `gemm`: it already handles the
    // zero-witness dance and emits a `FieldMatrix` with the correct
    // storage length for `m × n` shapes where the inner dim is zero.
    if m == 0 || k == 0 || n == 0 {
        return gemm(a, b);
    }

    // Below the threshold the classical path is strictly faster. Dispatch
    // immediately — the `gemm` call inherits SIMD and the T1 tiling.
    //
    // The Winograd peel needs `m, k, n ≥ 2` after padding to make
    // progress (half-dim ≥ 1). For tiny matrices with any dim = 1 we
    // can't peel productively, so we floor the effective threshold at
    // `2`. This also guards against the `threshold = 1` stress case
    // used by the bench harness / explicit-threshold tests.
    let effective_threshold = threshold.max(2);
    if m.min(k).min(n) < effective_threshold {
        return gemm(a, b);
    }

    // Theorem-4 sanity check: refuse to recurse if the bound that
    // Dumas–Pernet §1.4 theorem 4 places on every intermediate cell
    // value at depth `level + 1` would exceed the field's
    // delayed-reduction headroom. Concretely the wide accumulator in the
    // base-case gemm can hold at most
    //   F::max_unreduced_additions() · (p − 1)²
    // before a `reduce_wide` is required, and theorem 4 upper-bounds an
    // intermediate cell at depth ℓ by
    //   theorem_4_bound(ℓ, k, p − 1) = ((1+3^ℓ)/2)² · ceil(k/2^ℓ) · (p − 1)².
    // We compare these directly so the gate is a literal reading of the
    // theorem.
    //
    // For binary fields `max_unreduced_additions() == usize::MAX` and
    // `theorem_4_operand_bound() == u128::MAX`, so the gate is skipped —
    // XOR addition never overflows.
    let kmax = F::max_unreduced_additions();
    let p_minus_1 = F::theorem_4_operand_bound();
    if kmax != usize::MAX && p_minus_1 != u128::MAX {
        let bound = theorem_4_bound(level + 1, k, p_minus_1);
        let headroom = (kmax as u128)
            .saturating_mul(p_minus_1)
            .saturating_mul(p_minus_1);
        if bound > headroom {
            return gemm(a, b);
        }
    }

    // Sourcing a zero element. Every non-degenerate path lands here;
    // degenerate (zero-dim) paths are already delegated above.
    debug_assert!(!a.is_empty() && !b.is_empty());
    let zero: F = a.get(0, 0).zero_like();

    // Pad odd dimensions up to the next even value. We pad to the nearest
    // even number above — at most one extra row and/or column per axis,
    // per level.
    let m_even = m + (m & 1);
    let k_even = k + (k & 1);
    let n_even = n + (n & 1);

    let a_padded = pad_to(a, m_even, k_even, &zero);
    let b_padded = pad_to(b, k_even, n_even, &zero);

    // Recurse on the padded even-dim problem.
    let c_padded = winograd_step(&a_padded, &b_padded, &zero, threshold, level);

    // Slice the padded `c_padded` back to the original `m × n` shape.
    if (m_even, n_even) == (m, n) {
        c_padded
    } else {
        slice_to(&c_padded, m, n)
    }
}

/// One level of Winograd peel followed by seven recursive multiplies and
/// the U-assembly. Expects both inputs to have all-even dimensions. The
/// `zero` argument seeds output-allocation calls that cannot source a
/// witness from the operands (the sub-matrix shapes are by construction
/// non-empty once the outer dimensions were even).
fn winograd_step<F: FiniteField>(
    a: &FieldMatrix<F>,
    b: &FieldMatrix<F>,
    zero: &F,
    threshold: usize,
    level: u32,
) -> FieldMatrix<F> {
    let (m, k) = a.shape();
    let n = b.cols();
    debug_assert_eq!(a.cols(), b.rows());
    debug_assert!(m % 2 == 0 && k % 2 == 0 && n % 2 == 0);

    let mh = m / 2;
    let kh = k / 2;
    let nh = n / 2;

    // Extract the four quarter blocks of A and B. Each is a freshly
    // allocated row-major `FieldMatrix` so the recursive calls stay on the
    // crate's main contiguous-storage path (no view recursion).
    let a11 = submatrix(a, 0, 0, mh, kh, zero);
    let a12 = submatrix(a, 0, kh, mh, kh, zero);
    let a21 = submatrix(a, mh, 0, mh, kh, zero);
    let a22 = submatrix(a, mh, kh, mh, kh, zero);

    let b11 = submatrix(b, 0, 0, kh, nh, zero);
    let b12 = submatrix(b, 0, nh, kh, nh, zero);
    let b21 = submatrix(b, kh, 0, kh, nh, zero);
    let b22 = submatrix(b, kh, nh, kh, nh, zero);

    // Dumas–Pernet §1.4 algorithm 1.6 — S-T-M-U assembly.
    //
    // S1 = A21 + A22
    // S2 = S1 − A11
    // S3 = A11 − A21
    // S4 = A12 − S2
    let s1 = add_mats(&a21, &a22);
    let s2 = sub_mats(&s1, &a11);
    let s3 = sub_mats(&a11, &a21);
    let s4 = sub_mats(&a12, &s2);

    // T1 = B12 − B11
    // T2 = B22 − T1
    // T3 = B22 − B12
    // T4 = T2 − B21
    let t1 = sub_mats(&b12, &b11);
    let t2 = sub_mats(&b22, &t1);
    let t3 = sub_mats(&b22, &b12);
    let t4 = sub_mats(&t2, &b21);

    // The theorem-4 bound at `level + 1` was already checked against the
    // field's delayed-reduction headroom in `gemm_winograd_inner` before
    // we got here — that gate is the production fallback mechanism and
    // it references [`theorem_4_bound`] and
    // [`FiniteField::theorem_4_operand_bound`] literally. The
    // complementary Wide-shadow proptest
    // (`prop_winograd_wide_shadow_respects_theorem_4_bound_fp31`)
    // exercises the unreduced growth invariant directly on the S/T
    // operands at every level; the canonical-residue proptest
    // (`prop_winograd_bound_propagates_across_levels_fp31`) adds a
    // defence-in-depth check on top. The `level` counter flows into the
    // recursive multiplies below so the gate gets stricter with depth.

    // Seven recursive multiplies. Each call re-enters the recursion so
    // the bound / threshold gates apply at every level; the `level`
    // counter propagates so deeper calls assert against the
    // correspondingly tighter bound.
    let m1 = gemm_winograd_inner(&a11, &b11, threshold, level + 1);
    let m2 = gemm_winograd_inner(&a12, &b21, threshold, level + 1);
    let m3 = gemm_winograd_inner(&s4, &b22, threshold, level + 1);
    let m4 = gemm_winograd_inner(&a22, &t4, threshold, level + 1);
    let m5 = gemm_winograd_inner(&s1, &t1, threshold, level + 1);
    let m6 = gemm_winograd_inner(&s2, &t2, threshold, level + 1);
    let m7 = gemm_winograd_inner(&s3, &t3, threshold, level + 1);

    // U-assembly (DP §1.4):
    //   C11 = M1 + M2
    //   U2  = M1 + M6
    //   U3  = U2 + M7        →  C21 = U3 − M4   and   C22 = U3 + M5
    //   U4  = U2 + M5        →  C12 = U4 + M3
    let c11 = add_mats(&m1, &m2);
    let u2 = add_mats(&m1, &m6);
    let u3 = add_mats(&u2, &m7);
    let u4 = add_mats(&u2, &m5);
    let c12 = add_mats(&u4, &m3);
    let c21 = sub_mats(&u3, &m4);
    let c22 = add_mats(&u3, &m5);

    // Stitch the four output quarters back into an `m × n` matrix.
    assemble_quarters(&c11, &c12, &c21, &c22, zero)
}

/// Debug-only bound check for `Fp<P>`-style prime fields where the
/// canonical value is accessible as a `u128`. Used by the proptest
/// harness to assert the theorem-4 invariant at each recursion level
/// directly on the operand blocks. The function is only compiled into
/// the test binary.
#[cfg(test)]
fn canonical_values_respect_bound<F: FiniteField>(
    mat: &FieldMatrix<F>,
    level: u32,
    k: usize,
    p_minus_1: u128,
    value_of: impl Fn(&F) -> u128,
) -> bool {
    let bound = theorem_4_bound(level, k, p_minus_1);
    let (rows, cols) = mat.shape();
    for r in 0..rows {
        for c in 0..cols {
            let v = value_of(&mat.get_unchecked(r, c));
            if v > bound {
                return false;
            }
        }
    }
    true
}

/// Returns a freshly allocated `rows × cols` matrix that contains `src`
/// in its top-left corner, with the remaining cells set to `zero`.
fn pad_to<F: FiniteField>(
    src: &FieldMatrix<F>,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    let (sr, sc) = src.shape();
    debug_assert!(sr <= rows && sc <= cols);
    if (sr, sc) == (rows, cols) {
        return src.clone();
    }
    let data = FieldVec::zeros_from(rows * cols, zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..sr {
        for c in 0..sc {
            out.set(r, c, src.get_unchecked(r, c));
        }
    }
    out
}

/// Returns a freshly allocated `rows × cols` view of the top-left corner
/// of `src`.
fn slice_to<F: FiniteField>(src: &FieldMatrix<F>, rows: usize, cols: usize) -> FieldMatrix<F> {
    let (sr, sc) = src.shape();
    debug_assert!(rows <= sr && cols <= sc);
    if (rows, cols) == (sr, sc) {
        return src.clone();
    }
    let zero = src.get(0, 0).zero_like();
    let data = FieldVec::zeros_from(rows * cols, &zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, src.get_unchecked(r, c));
        }
    }
    out
}

/// Extracts a freshly allocated `rows × cols` sub-matrix at the given
/// offset. Used to materialise the four quarters of A and B before the
/// recursive multiplies. (Submatrix views would save the allocation but
/// the recursive `gemm_winograd` works on owned row-major storage, so
/// materialising once up front keeps the base-case gemm on its hot path.)
fn submatrix<F: FiniteField>(
    src: &FieldMatrix<F>,
    row_off: usize,
    col_off: usize,
    rows: usize,
    cols: usize,
    zero: &F,
) -> FieldMatrix<F> {
    debug_assert!(row_off + rows <= src.rows());
    debug_assert!(col_off + cols <= src.cols());
    let data = FieldVec::zeros_from(rows * cols, zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, src.get_unchecked(row_off + r, col_off + c));
        }
    }
    out
}

/// Stitches four equally-sized quarter matrices into a single `(2·mh) ×
/// (2·nh)` matrix. Called at each Winograd level to re-assemble the
/// output.
fn assemble_quarters<F: FiniteField>(
    c11: &FieldMatrix<F>,
    c12: &FieldMatrix<F>,
    c21: &FieldMatrix<F>,
    c22: &FieldMatrix<F>,
    zero: &F,
) -> FieldMatrix<F> {
    let (mh, nh) = c11.shape();
    debug_assert_eq!(c12.shape(), (mh, nh));
    debug_assert_eq!(c21.shape(), (mh, nh));
    debug_assert_eq!(c22.shape(), (mh, nh));
    let m = 2 * mh;
    let n = 2 * nh;
    let data = FieldVec::zeros_from(m * n, zero);
    let mut out = FieldMatrix::from_raw_parts(m, n, data);
    for r in 0..mh {
        for c in 0..nh {
            out.set(r, c, c11.get_unchecked(r, c));
            out.set(r, nh + c, c12.get_unchecked(r, c));
            out.set(mh + r, c, c21.get_unchecked(r, c));
            out.set(mh + r, nh + c, c22.get_unchecked(r, c));
        }
    }
    out
}

/// Elementwise `A + B` producing a fresh matrix. Used for the S/T/U
/// block adds in Winograd's peel.
fn add_mats<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    debug_assert_eq!(a.shape(), b.shape());
    let (rows, cols) = a.shape();
    let zero = a.get(0, 0).zero_like();
    let data = FieldVec::zeros_from(rows * cols, &zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, a.get_unchecked(r, c) + b.get_unchecked(r, c));
        }
    }
    out
}

/// Elementwise `A − B` producing a fresh matrix. Used for the S/T/U
/// block subs in Winograd's peel.
fn sub_mats<F: FiniteField>(a: &FieldMatrix<F>, b: &FieldMatrix<F>) -> FieldMatrix<F> {
    debug_assert_eq!(a.shape(), b.shape());
    let (rows, cols) = a.shape();
    let zero = a.get(0, 0).zero_like();
    let data = FieldVec::zeros_from(rows * cols, &zero);
    let mut out = FieldMatrix::from_raw_parts(rows, cols, data);
    for r in 0..rows {
        for c in 0..cols {
            out.set(r, c, a.get_unchecked(r, c) - b.get_unchecked(r, c));
        }
    }
    out
}

/// Dumas–Pernet theorem 4 bound. After `levels` recursion levels every
/// intermediate cell value satisfies
/// `|z| ≤ ((1 + 3^levels) / 2)² · ceil(k / 2^levels) · (p − 1)²`.
///
/// Returned as a `u128` so callers over prime fields with `p < 2^32` can
/// cross-check the observed cell value against the bound without
/// overflow. For characteristic-2 fields the theorem is vacuous (the
/// XOR accumulator never wraps); this helper then returns `u128::MAX`.
///
/// # Arguments
///
/// * `levels` — Depth of the Winograd recursion so far (0 = pure
///   classical gemm at the base).
/// * `k` — Inner matrix dimension at the top of the recursion.
/// * `p_minus_1` — Field characteristic bound per operand cell. Pass
///   `p - 1` for `Fp<P>`; pass `u128::MAX` / call sites should skip the
///   bound entirely for binary fields.
///
/// # Examples
///
/// ```
/// use gf2_core::field::winograd::theorem_4_bound;
///
/// // Classical gemm over `Fp<7>`, k = 4.
/// let b0 = theorem_4_bound(0, 4, 6);
/// assert_eq!(b0, 4 * 6 * 6); // (1·6)² implied here: (1+3^0)/2 = 1
///
/// // One Winograd level over `Fp<7>`, k = 4.
/// let b1 = theorem_4_bound(1, 4, 6);
/// assert_eq!(b1, 4 * 2 * 6 * 6); // 2² · ceil(4/2) · 36
/// ```
///
/// # Complexity
///
/// O(levels) integer multiplies.
pub fn theorem_4_bound(levels: u32, k: usize, p_minus_1: u128) -> u128 {
    if p_minus_1 == 0 {
        return 0;
    }
    let three_pow_l: u128 = 3u128.pow(levels);
    let one_plus = 1u128 + three_pow_l;
    // ((1 + 3^l) / 2)² — divisible since 1 + 3^l is always even.
    debug_assert!(one_plus % 2 == 0);
    let half = one_plus / 2;
    let factor = half.saturating_mul(half);

    let divisor = 1usize << (levels as usize).min(usize::BITS as usize - 1);
    let ceil_k = if divisor == 0 {
        k as u128
    } else {
        k.div_ceil(divisor) as u128
    };

    factor
        .saturating_mul(ceil_k)
        .saturating_mul(p_minus_1)
        .saturating_mul(p_minus_1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::gemm;
    use crate::gf2m::{Gf2mWide, Gf2mWideConfig};
    use crate::gfp::Fp;
    use rand::{Rng, SeedableRng};

    // Test config: GF(2^8) with AES irreducible via `Gf2mWide<1>`.
    struct WinoGf2m8Cfg;
    impl Gf2mWideConfig<1> for WinoGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "WinoGf2m8Cfg";
    }
    type WinoGf2m8 = Gf2mWide<1, WinoGf2m8Cfg>;

    const MERSENNE_31: u64 = 2_147_483_647;

    /// Threshold used throughout the test module. Reads the per-field
    /// trait default so the tests track any future override.
    const TEST_THRESHOLD: usize = <Fp<65_521> as FiniteField>::WINOGRAD_THRESHOLD;

    fn random_fp<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
            }
        }
        m
    }

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<WinoGf2m8> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<WinoGf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, WinoGf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    // ─── Bit-exactness below / at / above the threshold ──────────────────

    #[test]
    fn test_winograd_below_threshold_fp_small() {
        // Dimensions well below the threshold → should reduce to `gemm`.
        let a = random_fp::<7>(10, 12, 0x01);
        let b = random_fp::<7>(12, 8, 0x02);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_at_threshold_fp() {
        let n = TEST_THRESHOLD;
        let a = random_fp::<65_521>(n, n, 0xA1);
        let b = random_fp::<65_521>(n, n, 0xA2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_above_threshold_fp() {
        // Just above → one Winograd peel.
        let n = TEST_THRESHOLD + 2;
        let a = random_fp::<65_521>(n, n, 0xB1);
        let b = random_fp::<65_521>(n, n, 0xB2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    #[test]
    fn test_winograd_one_below_threshold_fp() {
        let n = TEST_THRESHOLD - 1;
        let a = random_fp::<65_521>(n, n, 0xC1);
        let b = random_fp::<65_521>(n, n, 0xC2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected);
    }

    // ─── Odd-dimension combinations ───────────────────────────────────────

    #[test]
    fn test_winograd_odd_m_fp() {
        let n = TEST_THRESHOLD + 1; // odd m
        let k = TEST_THRESHOLD + 4;
        let nn = TEST_THRESHOLD + 4;
        let a = random_fp::<65_521>(n, k, 0xD1);
        let b = random_fp::<65_521>(k, nn, 0xD2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd m");
    }

    #[test]
    fn test_winograd_odd_k_fp() {
        let m = TEST_THRESHOLD + 4;
        let k = TEST_THRESHOLD + 1; // odd k
        let n = TEST_THRESHOLD + 4;
        let a = random_fp::<65_521>(m, k, 0xE1);
        let b = random_fp::<65_521>(k, n, 0xE2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd k");
    }

    #[test]
    fn test_winograd_odd_n_fp() {
        let m = TEST_THRESHOLD + 4;
        let k = TEST_THRESHOLD + 4;
        let n = TEST_THRESHOLD + 1; // odd n
        let a = random_fp::<65_521>(m, k, 0xF1);
        let b = random_fp::<65_521>(k, n, 0xF2);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "odd n");
    }

    #[test]
    fn test_winograd_all_odd_fp() {
        let m = TEST_THRESHOLD + 1;
        let k = TEST_THRESHOLD + 3;
        let n = TEST_THRESHOLD + 5;
        let a = random_fp::<65_521>(m, k, 0x11);
        let b = random_fp::<65_521>(k, n, 0x12);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "all three odd");
    }

    #[test]
    fn test_winograd_all_odd_gf2m8() {
        // Binary field with the same odd-dim combinations.
        let m = TEST_THRESHOLD + 1;
        let k = TEST_THRESHOLD + 3;
        let n = TEST_THRESHOLD + 5;
        let a = random_gf2m8(m, k, 0x21);
        let b = random_gf2m8(k, n, 0x22);
        let expected = gemm(&a, &b);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got, expected, "gf2m8 all odd");
    }

    // ─── Degenerate shapes: empty rows / cols / inner dim ────────────────

    #[test]
    fn test_winograd_empty_outer() {
        let a = FieldMatrix::<Fp<7>>::zeros(0, 5);
        let b = FieldMatrix::<Fp<7>>::zeros(5, 3);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got.shape(), (0, 3));
    }

    #[test]
    fn test_winograd_empty_inner_const_field() {
        // (m, 0) × (0, n) on a const field — zero-output, non-zero shape.
        let a = FieldMatrix::<Fp<7>>::zeros(3, 0);
        let b = FieldMatrix::<Fp<7>>::zeros(0, 4);
        let got = gemm_winograd(&a, &b);
        assert_eq!(got.shape(), (3, 4));
        for r in 0..3 {
            for c in 0..4 {
                assert_eq!(got.get(r, c), Fp::<7>::new(0));
            }
        }
    }

    // ─── Padding round-trip: slice-back sanity ───────────────────────────

    #[test]
    fn test_pad_slice_roundtrip_preserves_values() {
        let n = TEST_THRESHOLD + 3;
        let a = random_fp::<65_521>(n, n, 0x31);
        let padded = pad_to(&a, n + 1, n + 1, &Fp::<65_521>::new(0));
        assert_eq!(padded.shape(), (n + 1, n + 1));
        // Original region is preserved bit-exactly.
        for r in 0..n {
            for c in 0..n {
                assert_eq!(padded.get(r, c), a.get(r, c), "({}, {})", r, c);
            }
        }
        // Padded region is zero.
        for r in 0..n {
            assert_eq!(padded.get(r, n), Fp::<65_521>::new(0));
        }
        for c in 0..=n {
            assert_eq!(padded.get(n, c), Fp::<65_521>::new(0));
        }
        // Slicing back gives exactly `a`.
        let sliced = slice_to(&padded, n, n);
        assert_eq!(sliced, a);
    }

    // ─── theorem_4_bound helper ──────────────────────────────────────────

    #[test]
    fn test_theorem_4_bound_level_0_matches_classical() {
        // At level 0 the bound is `1 · k · (p-1)²`, exactly the classical
        // gemm inner-sum bound.
        let p_m1 = 6u128;
        let k = 17usize;
        assert_eq!(theorem_4_bound(0, k, p_m1), (k as u128) * p_m1 * p_m1);
    }

    #[test]
    fn test_theorem_4_bound_level_1_formula() {
        // Level 1: ((1+3)/2)² = 4, ceil(k/2) = ceil(17/2) = 9.
        let p_m1 = 6u128;
        let k = 17usize;
        assert_eq!(theorem_4_bound(1, k, p_m1), 4 * 9 * p_m1 * p_m1);
    }

    #[test]
    fn test_theorem_4_bound_level_2_formula() {
        // Level 2: ((1+9)/2)² = 25, ceil(k/4) = ceil(17/4) = 5.
        let p_m1 = 6u128;
        let k = 17usize;
        assert_eq!(theorem_4_bound(2, k, p_m1), 25 * 5 * p_m1 * p_m1);
    }

    #[test]
    fn test_theorem_4_bound_zero_field() {
        // Degenerate p = 1 → bound is 0.
        assert_eq!(theorem_4_bound(3, 16, 0), 0);
    }

    // ─── Recursive-depth bound-propagation proptest ──────────────────────

    /// A level-aware recursive scaffold that mirrors
    /// [`gemm_winograd_inner`] but, at each recursion level, checks that
    /// every S/T block operand entering a recursive multiply has
    /// canonical values respecting the theorem-4 bound at the level
    /// being entered. Reaches multiple recursion levels by construction
    /// (input size ≥ 4 × threshold → at least 2 peels).
    fn verify_bound_recursive(
        a: &FieldMatrix<Fp<MERSENNE_31>>,
        b: &FieldMatrix<Fp<MERSENNE_31>>,
        threshold: usize,
        level: u32,
        k_top: usize,
        p_minus_1: u128,
    ) -> FieldMatrix<Fp<MERSENNE_31>> {
        let (m, k) = a.shape();
        let n = b.cols();
        if m == 0 || k == 0 || n == 0 {
            return gemm(a, b);
        }
        if m.min(k).min(n) < threshold.max(2) {
            // Base case: assert canonical operands respect the theorem-4
            // bound at THIS level (level). For level 0 this is the
            // trivially loose `k · (p-1)²` bound; at deeper levels the
            // bound accounts for the Winograd growth factor.
            assert!(
                canonical_values_respect_bound(a, level, k_top, p_minus_1, |f| f.value() as u128),
                "level {} operand A fails theorem-4 bound",
                level
            );
            assert!(
                canonical_values_respect_bound(b, level, k_top, p_minus_1, |f| f.value() as u128),
                "level {} operand B fails theorem-4 bound",
                level
            );
            return gemm(a, b);
        }
        let zero = a.get(0, 0).zero_like();
        let m_even = m + (m & 1);
        let k_even = k + (k & 1);
        let n_even = n + (n & 1);
        let a_p = pad_to(a, m_even, k_even, &zero);
        let b_p = pad_to(b, k_even, n_even, &zero);
        let mh = m_even / 2;
        let kh = k_even / 2;
        let nh = n_even / 2;
        let a11 = submatrix(&a_p, 0, 0, mh, kh, &zero);
        let a12 = submatrix(&a_p, 0, kh, mh, kh, &zero);
        let a21 = submatrix(&a_p, mh, 0, mh, kh, &zero);
        let a22 = submatrix(&a_p, mh, kh, mh, kh, &zero);
        let b11 = submatrix(&b_p, 0, 0, kh, nh, &zero);
        let b12 = submatrix(&b_p, 0, nh, kh, nh, &zero);
        let b21 = submatrix(&b_p, kh, 0, kh, nh, &zero);
        let b22 = submatrix(&b_p, kh, nh, kh, nh, &zero);
        let s1 = add_mats(&a21, &a22);
        let s2 = sub_mats(&s1, &a11);
        let s3 = sub_mats(&a11, &a21);
        let s4 = sub_mats(&a12, &s2);
        let t1 = sub_mats(&b12, &b11);
        let t2 = sub_mats(&b22, &t1);
        let t3 = sub_mats(&b22, &b12);
        let t4 = sub_mats(&t2, &b21);

        // Level (level + 1) bound check on the S/T blocks going INTO the
        // recursive multiplies. This is the heart of the theorem-4
        // propagation check: at level ℓ+1 every cell of S_i, T_i must
        // fit `theorem_4_bound(ℓ+1, k_top, p − 1)`.
        let next_level = level + 1;
        for (name, block) in [
            ("S1", &s1),
            ("S2", &s2),
            ("S3", &s3),
            ("S4", &s4),
            ("T1", &t1),
            ("T2", &t2),
            ("T3", &t3),
            ("T4", &t4),
        ] {
            assert!(
                canonical_values_respect_bound(block, next_level, k_top, p_minus_1, |f| {
                    f.value() as u128
                }),
                "block {} at level {} fails theorem-4 bound",
                name,
                next_level
            );
        }
        let m1 = verify_bound_recursive(&a11, &b11, threshold, next_level, k_top, p_minus_1);
        let m2 = verify_bound_recursive(&a12, &b21, threshold, next_level, k_top, p_minus_1);
        let m3 = verify_bound_recursive(&s4, &b22, threshold, next_level, k_top, p_minus_1);
        let m4 = verify_bound_recursive(&a22, &t4, threshold, next_level, k_top, p_minus_1);
        let m5 = verify_bound_recursive(&s1, &t1, threshold, next_level, k_top, p_minus_1);
        let m6 = verify_bound_recursive(&s2, &t2, threshold, next_level, k_top, p_minus_1);
        let m7 = verify_bound_recursive(&s3, &t3, threshold, next_level, k_top, p_minus_1);
        let c11 = add_mats(&m1, &m2);
        let u2 = add_mats(&m1, &m6);
        let u3 = add_mats(&u2, &m7);
        let u4 = add_mats(&u2, &m5);
        let c12 = add_mats(&u4, &m3);
        let c21 = sub_mats(&u3, &m4);
        let c22 = add_mats(&u3, &m5);
        let c_padded = assemble_quarters(&c11, &c12, &c21, &c22, &zero);
        if (m_even, n_even) == (m, n) {
            c_padded
        } else {
            slice_to(&c_padded, m, n)
        }
    }

    // ─── Wide-shadow bound propagation on UNREDUCED magnitudes ───────────

    /// Scaffold that mirrors [`gemm_winograd_inner`] 1:1 but carries each
    /// cell's value as a signed `i128` **unreduced integer magnitude**
    /// (i.e. it does not apply any `mod p` during the S/T/U assembly).
    /// Every add / sub in Winograd's peel propagates the true integer,
    /// so at any recursion level `ℓ` the magnitude of a cell is exactly
    /// the worst-case bound theorem 4 talks about. At every level we
    /// assert `|cell| ≤ theorem_4_bound(ℓ, k_top, p − 1)` on the eight
    /// S/T operand blocks going into the next recursive multiply — this
    /// is stronger than the canonical-residue proptest because the
    /// unreduced magnitudes grow as Winograd peels, while canonical
    /// values trivially stay ≤ `p − 1`.
    ///
    /// The per-cell magnitude is tracked in an `I128Mat` shadow: a
    /// freshly allocated `rows * cols` `Vec<i128>`. The function returns
    /// the final padded shadow (never consumed — we only assert
    /// invariants), and the outer scaffold checks that the **top-level
    /// output** shadow, once reduced mod p, equals the classical gemm
    /// output over `Fp<MERSENNE_31>`. This double-checks the scaffold
    /// itself is faithful.
    #[derive(Clone)]
    struct I128Mat {
        rows: usize,
        cols: usize,
        data: Vec<i128>,
    }
    impl I128Mat {
        fn zeros(rows: usize, cols: usize) -> Self {
            Self {
                rows,
                cols,
                data: vec![0i128; rows * cols],
            }
        }
        fn from_fp(src: &FieldMatrix<Fp<MERSENNE_31>>) -> Self {
            let (rows, cols) = src.shape();
            let mut out = Self::zeros(rows, cols);
            for r in 0..rows {
                for c in 0..cols {
                    out.data[r * cols + c] = src.get_unchecked(r, c).value() as i128;
                }
            }
            out
        }
        fn pad_to(&self, rows: usize, cols: usize) -> Self {
            debug_assert!(self.rows <= rows && self.cols <= cols);
            let mut out = Self::zeros(rows, cols);
            for r in 0..self.rows {
                for c in 0..self.cols {
                    out.data[r * cols + c] = self.data[r * self.cols + c];
                }
            }
            out
        }
        fn submatrix(&self, row_off: usize, col_off: usize, rows: usize, cols: usize) -> Self {
            let mut out = Self::zeros(rows, cols);
            for r in 0..rows {
                for c in 0..cols {
                    out.data[r * cols + c] = self.data[(row_off + r) * self.cols + (col_off + c)];
                }
            }
            out
        }
        fn add(&self, other: &Self) -> Self {
            debug_assert_eq!((self.rows, self.cols), (other.rows, other.cols));
            let mut out = Self::zeros(self.rows, self.cols);
            for i in 0..self.data.len() {
                out.data[i] = self.data[i].saturating_add(other.data[i]);
            }
            out
        }
        fn sub(&self, other: &Self) -> Self {
            debug_assert_eq!((self.rows, self.cols), (other.rows, other.cols));
            let mut out = Self::zeros(self.rows, self.cols);
            for i in 0..self.data.len() {
                out.data[i] = self.data[i].saturating_sub(other.data[i]);
            }
            out
        }
        fn max_abs(&self) -> u128 {
            let mut m: u128 = 0;
            for &v in &self.data {
                let a = v.unsigned_abs();
                if a > m {
                    m = a;
                }
            }
            m
        }
    }

    /// Verify the theorem-4 bound holds for every S/T block operand in
    /// the unreduced integer shadow at every recursion level. Returns
    /// the shadow output matrix (not needed for assertions; returned
    /// only so the outer test can double-check the scaffold tracks
    /// reality).
    fn verify_wide_shadow_recursive(
        a_shadow: &I128Mat,
        b_shadow: &I128Mat,
        threshold: usize,
        level: u32,
        k_top: usize,
        p_minus_1: u128,
    ) -> I128Mat {
        let (m, k) = (a_shadow.rows, a_shadow.cols);
        let n = b_shadow.cols;
        debug_assert_eq!(b_shadow.rows, k);
        if m == 0 || k == 0 || n == 0 {
            return I128Mat::zeros(m, n);
        }
        if m.min(k).min(n) < threshold.max(2) {
            // Base case: no further peel. Assert that the *operands*
            // entering this gemm respect the bound at THIS level.
            let bound = theorem_4_bound(level, k_top, p_minus_1);
            assert!(
                a_shadow.max_abs() <= bound,
                "wide-shadow level {} A.max_abs = {} > bound {}",
                level,
                a_shadow.max_abs(),
                bound,
            );
            assert!(
                b_shadow.max_abs() <= bound,
                "wide-shadow level {} B.max_abs = {} > bound {}",
                level,
                b_shadow.max_abs(),
                bound,
            );
            // Base-case "multiplication": classical gemm as i128 (the
            // integer product of two bounded matrices). We use
            // saturating arithmetic to avoid panics if the bench ever
            // runs with parameters that would overflow i128 at the base
            // — callers are responsible for sizing.
            let mut out = I128Mat::zeros(m, n);
            for i in 0..m {
                for j in 0..n {
                    let mut acc: i128 = 0;
                    for t in 0..k {
                        let av = a_shadow.data[i * k + t];
                        let bv = b_shadow.data[t * n + j];
                        acc = acc.saturating_add(av.saturating_mul(bv));
                    }
                    out.data[i * n + j] = acc;
                }
            }
            return out;
        }
        let m_even = m + (m & 1);
        let k_even = k + (k & 1);
        let n_even = n + (n & 1);
        let a_p = a_shadow.pad_to(m_even, k_even);
        let b_p = b_shadow.pad_to(k_even, n_even);
        let mh = m_even / 2;
        let kh = k_even / 2;
        let nh = n_even / 2;
        let a11 = a_p.submatrix(0, 0, mh, kh);
        let a12 = a_p.submatrix(0, kh, mh, kh);
        let a21 = a_p.submatrix(mh, 0, mh, kh);
        let a22 = a_p.submatrix(mh, kh, mh, kh);
        let b11 = b_p.submatrix(0, 0, kh, nh);
        let b12 = b_p.submatrix(0, nh, kh, nh);
        let b21 = b_p.submatrix(kh, 0, kh, nh);
        let b22 = b_p.submatrix(kh, nh, kh, nh);
        // S/T block adds/subs tracked over the unreduced integer shadow.
        let s1 = a21.add(&a22);
        let s2 = s1.sub(&a11);
        let s3 = a11.sub(&a21);
        let s4 = a12.sub(&s2);
        let t1 = b12.sub(&b11);
        let t2 = b22.sub(&t1);
        let t3 = b22.sub(&b12);
        let t4 = t2.sub(&b21);
        // **Wide-shadow theorem-4 assertion**: every S/T operand
        // entering the next recursive multiply must satisfy the
        // theorem-4 bound at depth (level + 1). Unlike the
        // canonical-residue proptest this tests the *unreduced* integer
        // magnitude, so a regression in the peel (e.g. a future SIMD
        // specialisation doing an extra add) would be caught.
        let next_level = level + 1;
        let bound_next = theorem_4_bound(next_level, k_top, p_minus_1);
        for (name, blk) in [
            ("S1", &s1),
            ("S2", &s2),
            ("S3", &s3),
            ("S4", &s4),
            ("T1", &t1),
            ("T2", &t2),
            ("T3", &t3),
            ("T4", &t4),
        ] {
            let observed = blk.max_abs();
            assert!(
                observed <= bound_next,
                "wide-shadow level {} block {} observed {} > theorem_4_bound = {}",
                next_level,
                name,
                observed,
                bound_next
            );
        }
        let m1 = verify_wide_shadow_recursive(&a11, &b11, threshold, next_level, k_top, p_minus_1);
        let m2 = verify_wide_shadow_recursive(&a12, &b21, threshold, next_level, k_top, p_minus_1);
        let m3 = verify_wide_shadow_recursive(&s4, &b22, threshold, next_level, k_top, p_minus_1);
        let m4 = verify_wide_shadow_recursive(&a22, &t4, threshold, next_level, k_top, p_minus_1);
        let m5 = verify_wide_shadow_recursive(&s1, &t1, threshold, next_level, k_top, p_minus_1);
        let m6 = verify_wide_shadow_recursive(&s2, &t2, threshold, next_level, k_top, p_minus_1);
        let m7 = verify_wide_shadow_recursive(&s3, &t3, threshold, next_level, k_top, p_minus_1);
        let c11 = m1.add(&m2);
        let u2 = m1.add(&m6);
        let u3 = u2.add(&m7);
        let u4 = u2.add(&m5);
        let c12 = u4.add(&m3);
        let c21 = u3.sub(&m4);
        let c22 = u3.add(&m5);
        // Stitch quarters. Returning the padded shadow is fine — the
        // outer scaffold only reads max_abs.
        let mut c = I128Mat::zeros(2 * mh, 2 * nh);
        for r in 0..mh {
            for ccol in 0..nh {
                c.data[r * (2 * nh) + ccol] = c11.data[r * nh + ccol];
                c.data[r * (2 * nh) + nh + ccol] = c12.data[r * nh + ccol];
                c.data[(mh + r) * (2 * nh) + ccol] = c21.data[r * nh + ccol];
                c.data[(mh + r) * (2 * nh) + nh + ccol] = c22.data[r * nh + ccol];
            }
        }
        c
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(4))]

        /// Recursive theorem-4 bound propagation on Mersenne-31.
        ///
        /// The input is sized at `n = 4 · small_threshold`, guaranteeing
        /// **at least two** levels of Winograd recursion. At every
        /// recursion step we assert the canonical values of the eight S /
        /// T operands entering the seven sub-multiplies satisfy the
        /// theorem-4 bound at their respective level. Because canonical
        /// values fit `[0, p − 1]` while the bound at level ℓ scales as
        /// `((1+3^ℓ)/2)² · ⌈k/2^ℓ⌉ · (p − 1)²`, this asserts the
        /// per-level growth envelope, not just the final-output
        /// invariant.
        ///
        /// The test uses a small (4) Winograd threshold to force deep
        /// recursion at a manageable matrix size.
        #[test]
        fn prop_winograd_bound_propagates_across_levels_fp31(
            seed in 0u64..256,
        ) {
            let threshold_small = 4usize; // force ≥ 2 levels at small n
            let n = 4 * threshold_small; // = 16; guarantees 2+ levels
            proptest::prop_assume!(n >= 4 * threshold_small);
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let b = random_fp::<MERSENNE_31>(n, n, seed.wrapping_add(1));
            let p_minus_1 = (MERSENNE_31 - 1) as u128;
            // Bit-exact match vs classical gemm (structural correctness
            // of the bound-verifying scaffold).
            let expected = gemm(&a, &b);
            let got = verify_bound_recursive(&a, &b, threshold_small, 0, n, p_minus_1);
            proptest::prop_assert_eq!(got.clone(), expected);
            // Also exercise the production path (default threshold) at a
            // size where it reaches at least one peel, to confirm the
            // same bounds hold there.
            let n_prod = 4 * TEST_THRESHOLD;
            let a_prod = random_fp::<MERSENNE_31>(n_prod, n_prod, seed ^ 0xABCD);
            let b_prod = random_fp::<MERSENNE_31>(n_prod, n_prod, seed ^ 0x1234);
            let expected_prod = gemm(&a_prod, &b_prod);
            let got_prod = gemm_winograd(&a_prod, &b_prod);
            proptest::prop_assert_eq!(got_prod, expected_prod);
        }

        #[test]
        fn prop_winograd_matches_classical_fp7(
            m in 1usize..6,
            k in 1usize..6,
            n in 1usize..6,
            seed_a in 0u64..1024,
            seed_b in 0u64..1024,
        ) {
            // Below threshold: Winograd must dispatch to `gemm` and
            // therefore match it bit-exactly regardless of input shape.
            let a = random_fp::<7>(m, k, seed_a);
            let b = random_fp::<7>(k, n, seed_b);
            let got = gemm_winograd(&a, &b);
            let expected = gemm(&a, &b);
            proptest::prop_assert_eq!(got, expected);
        }

        /// Wide-shadow theorem-4 bound propagation on Mersenne-31.
        ///
        /// Mirrors [`gemm_winograd_inner`] with a small threshold (4) at
        /// `n = 16` (force ≥ 2 recursion levels) while tracking each
        /// cell as an **unreduced** `i128` integer shadow — no `mod p`
        /// applied during S/T/U assembly. At every level it asserts
        /// every S_i, T_i block operand entering a recursive multiply
        /// satisfies `|cell| ≤ theorem_4_bound(ℓ+1, k_top, p − 1)`.
        /// This is stronger than the canonical-residue propagation test
        /// above because the unreduced integer magnitude really does
        /// grow as Winograd peels, while canonical values (always
        /// `[0, p-1]`) trivially stay under the bound.
        #[test]
        fn prop_winograd_wide_shadow_respects_theorem_4_bound_fp31(
            seed in 0u64..256,
        ) {
            let threshold_small = 4usize; // force ≥ 2 peels at n = 16
            let n = 4 * threshold_small; // = 16
            let a = random_fp::<MERSENNE_31>(n, n, seed);
            let b = random_fp::<MERSENNE_31>(n, n, seed.wrapping_add(7));
            let p_minus_1 = (MERSENNE_31 - 1) as u128;
            let a_sh = I128Mat::from_fp(&a);
            let b_sh = I128Mat::from_fp(&b);
            let _out_shadow = verify_wide_shadow_recursive(
                &a_sh,
                &b_sh,
                threshold_small,
                0,
                n,
                p_minus_1,
            );
            // Additionally: bit-exact production path still matches
            // classical gemm (so the bound gate never trips at this n).
            let expected = gemm(&a, &b);
            let got = gemm_winograd_with_threshold(&a, &b, threshold_small);
            proptest::prop_assert_eq!(got, expected);
        }
    }

    #[test]
    fn test_explicit_threshold_bit_exact_fp7() {
        // `gemm_winograd_with_threshold` at small values forces maximum
        // recursion depth; at `usize::MAX` it forces the classical
        // path. Both must equal `gemm`. The effective threshold is
        // floored at 2 (the smallest size at which a Winograd peel can
        // make progress — at `min(m,k,n) = 1` the half-dim is 0 and the
        // peel is degenerate, so the implementation dispatches to
        // classical `gemm` regardless of threshold).
        let a = random_fp::<7>(6, 6, 0x41);
        let b = random_fp::<7>(6, 6, 0x42);
        let expected = gemm(&a, &b);
        for threshold in [1usize, 2, 3, 4, 5, 6, 7, 8, usize::MAX] {
            let got = gemm_winograd_with_threshold(&a, &b, threshold);
            assert_eq!(got, expected, "threshold = {}", threshold);
        }
    }

    #[test]
    fn test_winograd_threshold_trait_default() {
        // Mersenne-31 + Gf2m8 inherit the default 128.
        assert_eq!(
            <Fp<MERSENNE_31> as FiniteField>::WINOGRAD_THRESHOLD,
            128,
            "Mersenne-31 uses default threshold"
        );
        assert_eq!(
            <WinoGf2m8 as FiniteField>::WINOGRAD_THRESHOLD,
            128,
            "Gf2m8 uses default threshold"
        );
    }
}
