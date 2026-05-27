//! PLE decomposition and derived row-echelon / RREF / nullspace / LU
//! operations over an arbitrary [`FiniteField`].
//!
//! Issue `c3f8c1cb` (R2 rework). Implements Dumas–Pernet §2.2 algorithm
//! 2.5: given an `m × n` matrix `A`, compute a permutation `P`, a unit
//! lower-trapezoidal `L` (`m × r`) and a row-echelon matrix `E` (`r × n`)
//! such that
//!
//! ```text
//!     P · L · E = A
//! ```
//!
//! where `r = rank(A)`. The decomposition is unique once a pivot rule is
//! fixed (this implementation uses the first non-zero entry from the top,
//! standard in the cited paper).
//!
//! # Allocation budget
//!
//! The PLE recursion runs in place on a single working clone of the input
//! matrix; sub-blocks are passed as [`MatView`] / [`MatViewMut`]. Each
//! recursive level pays for two intrinsic gemm-kernel B-transposes (one
//! from [`trsm_lower`] on the rank-deficient branch, one from
//! [`gemm_axpy_into_view`] for the Schur complement update). To bridge
//! the row-major layout's lack of a safe `split_cols_mut`, the read-side
//! operands `L1` (the unit-lower-triangular leading block) and `L1_bot`
//! (its strict-lower extension) are materialised into owned buffers per
//! level.
//!
//! Total `FieldMatrix<F>` allocations pinned in
//! `tests::test_*_allocation_budget_*`:
//!
//! - [`ple`](FieldMatrix::ple)`(m × n)`: input clone + final L + final E
//!   + per-level (L1 owned, L1_bot owned, gemm B-transpose, trsm
//!     B-transpose).
//! - [`row_echelon`](FieldMatrix::row_echelon)`(m × n)`: PLE + the inverted
//!   `L_full` block + `Pᵀ` + the assembled `E_full`.
//! - [`rref`](FieldMatrix::rref)`(m × n)`: row_echelon + panelized
//!   back-substitution (Stage 3a/3b) when `max(m, n) >= BLOCKED_BACK_SUB_MIN_DIM`
//!   (= 128), allocating 8 scratch `FieldMatrix` instances (`e_piv_piv`,
//!   `e_piv_free`, `x_piv`, `e_nonpiv_piv`, `e_piv_free_post`, `e_nonpiv_free`,
//!   `x_piv_post`, `x_nonpiv`); falls through to the scalar loop below the
//!   threshold. At n=64 uses the scalar loop: total count = `EXPECTED_RREF_N64`
//!   = 280. At n >= 128 the blocked path adds 8 scratch matrices + trsm
//!   B-transposes.
//! - [`lu`](FieldMatrix::lu)`(m × n)`: PLE + 0 (just repackages PLE's
//!   outputs).
//! - [`nullspace`](FieldMatrix::nullspace)`(m × n)`: rref + `(n − rank)`
//!   [`FieldVec`] allocations (no extra `FieldMatrix::new`).
//!
//! Exact counts are pinned in `tests::test_ple_allocation_budget_*` with
//! strict integer asserts.
//!
//! # Algorithm
//!
//! Block-recursive (Dumas–Pernet §2.2 alg. 2.5), splitting on columns, with
//! a direct-elimination base case when the column window reaches
//! [`FiniteField::PLE_BASE_COLS`] (default 1 — the single-column leaf):
//!
//! ```text
//! ple(A):  // A is m × n
//!     if n <= PLE_BASE_COLS:
//!         direct column-by-column Gaussian elimination (ple_base_direct)
//!     else:
//!         h = n / 2
//!         (r1, pc_left) = ple(A[:, 0..h])         (recurse on the left;
//!                                                   pc_left is the set of
//!                                                   r1 absolute pivot
//!                                                   columns found in
//!                                                   [0..h))
//!         L1     = unit-lower-triangular shaped from A[0..r1, pc_left]
//!         L1_bot = A[r1..m, pc_left]
//!         A[0..r1, h..n]   ← trsm_lower(L1, A[0..r1, h..n])             (A3)
//!         A[r1..m, h..n]   ← A[r1..m, h..n] − L1_bot · A[0..r1, h..n]   (A4)
//!         r2 = ple(A[r1..m, h..n])
//!         return r1 + r2
//! ```
//!
//! Note: `L1` and `L1_bot` source their cells from the actual pivot
//! columns `pc_left` rather than the contiguous prefix `[0..r1)`. The
//! compact-storage convention places L's multipliers under their pivot
//! columns (see below), and those columns are non-contiguous when the
//! left half is rank-deficient (one or more columns in `[0..h)` had
//! no pivot). Sourcing from the contiguous prefix in that case reads
//! pre-Schur-eliminated zeros (or earlier pivots' multipliers) and
//! silently corrupts the trsm + gemm update — see jit:bd9c6e13 for the
//! discovery case (15x17 GF(7), seed=1, density=0.05).
//!
//! The default `PLE_BASE_COLS = 1` uses the block-recursive trsm+gemm path
//! for all window widths > 1. This is optimal for large-prime fields (e.g.
//! Mersenne-31) where the blocked GEMM with delayed u128 reduction
//! significantly outperforms any scalar schoolbook loop. Fields with cheap
//! per-element arithmetic (e.g. GF(2^m)) or small primes (p ≤ 251 with AVX2)
//! may benefit from a larger `PLE_BASE_COLS` override if profiling confirms
//! the schoolbook base case beats the recursive dispatch overhead.
//!
//! Compact storage: after the recursion, the working buffer interleaves
//! `E`'s entries and `L`'s multipliers within a single dense `m × n`
//! grid. Specifically, for each pivot index `k = 0..r` with absolute
//! pivot column `pc[k]`:
//!
//! - `working[k, pc[k]..n]` holds row `k` of `E` (above the diagonal of
//!   the leading pivot block); cells `working[k, j]` for `j < pc[k]`
//!   may hold L-multipliers of earlier rows but are projected to zero
//!   when E is extracted.
//! - `working[i, pc[k]]` for `i > k` holds `L`'s `k`-th column
//!   multiplier (not the value at column `k` of `working`, since
//!   `pc[k]` may exceed `k`).
//!
//! The base case writes `working[k, col] = working[k, col] / pivot` for
//! `k > 0`, leaving the pivot value at the pivot row (so the diagonal
//! cell `working[k, pc[k]]` carries E's pivot value, NOT `1`; the L
//! factor's unit diagonal is synthesised when extracting `L`).
//!
//! See Dumas–Pernet, "Polynomial-time matrix algorithms over finite fields,"
//! 2010, alg. 2.5 (PLE), 2.6 (row echelon), 2.7 (RREF).
//!
//! # Relationship to derived ops
//!
//! - [`row_echelon`](FieldMatrix::row_echelon): from `(P, L, E, r)`,
//!   `X = L_full⁻¹ · Pᵀ` (with `L` extended to `m × m` by appending an
//!   identity block) is solved via [`trsm_lower`] then composed with
//!   `Pᵀ`.
//! - [`rref`](FieldMatrix::rref): start from echelon `(X₀, E)`, scale
//!   each pivot row to make leading entries `1`, then peel each pivot
//!   column (zero entries above and below).
//! - [`rank`](FieldMatrix::rank): the fourth return of `ple`.
//! - [`nullspace`](FieldMatrix::nullspace): from RREF, free columns
//!   produce basis vectors.
//! - [`lu`](FieldMatrix::lu): exists only when `rank == min(m, n)`;
//!   returns `(P, L, U)` where `U = E`.

use crate::field::matrix::{gemm_axpy_into_view, FieldMatrix, MatView, MatViewMut};
use crate::field::triangular::{trsm_lower, trsm_upper};
use crate::field::vec::FieldVec;
use crate::field::FiniteField;

// ─── Permutation ─────────────────────────────────────────────────────────────

/// A row permutation produced by [`FieldMatrix::ple`].
///
/// Stored compactly as a `Vec<usize>` of length `m` where `perm[i]` is
/// the row of the original matrix that has been moved to row `i`. This is
/// the **destination → source** convention: applying the permutation to a
/// matrix `A` row-wise yields a matrix `B` with `B[i, *] = A[perm[i], *]`.
///
/// Equivalently, the permutation matrix `P` defined by `P[i, perm[i]] = 1`
/// (and zero elsewhere) satisfies `B = P · A`.
///
/// `Permutation` does NOT materialise an `m × m` field-matrix; it stores
/// just the index vector.
///
/// # Examples
///
/// ```
/// use gf2_core::field::matrix::FieldMatrix;
/// use gf2_core::gfp::Fp;
///
/// let mut a = FieldMatrix::<Fp<7>>::zeros(3, 1);
/// a.set(1, 0, Fp::<7>::new(2));
/// a.set(2, 0, Fp::<7>::new(3));
/// let (p, _l, _e, _r) = a.ple();
/// // First non-zero is at row 1, so P swaps row 0 with row 1.
/// assert_eq!(p.indices(), &[1, 0, 2]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permutation {
    /// `perm[i]` is the row of the input that ended up at row `i` of
    /// `P · A`. Always a valid permutation of `0..len()`.
    perm: Vec<usize>,
}

impl Permutation {
    /// Builds the identity permutation on `n` rows.
    ///
    /// # Arguments
    ///
    /// * `n` — Number of rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::Permutation;
    ///
    /// let p = Permutation::identity(4);
    /// assert_eq!(p.indices(), &[0, 1, 2, 3]);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` writes plus one allocation.
    pub fn identity(n: usize) -> Self {
        Self {
            perm: (0..n).collect(),
        }
    }

    /// Builds a permutation directly from an index vector.
    ///
    /// The caller must ensure `perm` is a valid permutation of
    /// `0..perm.len()`. Debug builds verify this; release builds trust
    /// the caller.
    ///
    /// # Arguments
    ///
    /// * `perm` — Destination → source vector. `perm[i] = j` means row
    ///   `j` of the input lands at row `i` of `P · A`.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if `perm` is not a valid permutation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::Permutation;
    ///
    /// let p = Permutation::from_indices(vec![2, 0, 1]);
    /// assert_eq!(p.indices(), &[2, 0, 1]);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` for the debug-mode validation, `O(1)` in release.
    pub fn from_indices(perm: Vec<usize>) -> Self {
        if cfg!(debug_assertions) {
            let n = perm.len();
            let mut seen = vec![false; n];
            for &i in &perm {
                assert!(
                    i < n,
                    "Permutation::from_indices: index {} out of bounds (len={})",
                    i,
                    n
                );
                assert!(!seen[i], "Permutation::from_indices: duplicate index {}", i);
                seen[i] = true;
            }
        }
        Self { perm }
    }

    /// Returns the destination → source index vector.
    ///
    /// `indices()[i]` is the original row that ended up at row `i` after
    /// applying the permutation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::Permutation;
    ///
    /// let p = Permutation::identity(3);
    /// assert_eq!(p.indices(), &[0, 1, 2]);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn indices(&self) -> &[usize] {
        &self.perm
    }

    /// Length of the permutation (number of rows it permutes).
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::Permutation;
    ///
    /// assert_eq!(Permutation::identity(5).len(), 5);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn len(&self) -> usize {
        self.perm.len()
    }

    /// Returns `true` if this permutation has length zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::Permutation;
    ///
    /// assert!(Permutation::identity(0).is_empty());
    /// assert!(!Permutation::identity(3).is_empty());
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(1)`.
    pub fn is_empty(&self) -> bool {
        self.perm.is_empty()
    }

    /// Returns the inverse permutation `P⁻¹`.
    ///
    /// If `self.indices()[i] = j`, then `inverse().indices()[j] = i`.
    /// `P⁻¹ · (P · A) = A` for any matrix `A`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::Permutation;
    ///
    /// let p = Permutation::from_indices(vec![2, 0, 1]);
    /// let inv = p.inverse();
    /// assert_eq!(inv.indices(), &[1, 2, 0]);
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(n)` writes plus one allocation of size `n`.
    pub fn inverse(&self) -> Permutation {
        let n = self.perm.len();
        let mut inv = vec![0usize; n];
        for (i, &j) in self.perm.iter().enumerate() {
            inv[j] = i;
        }
        Permutation { perm: inv }
    }

    /// Applies this permutation to the rows of `m`, returning `P · m`.
    ///
    /// The output's row `i` is row `self.indices()[i]` of `m`.
    ///
    /// # Arguments
    ///
    /// * `m` — Input matrix; must satisfy `m.rows() == self.len()`.
    ///
    /// # Panics
    ///
    /// Panics if `m.rows() != self.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::{FieldMatrix, Permutation};
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut m = FieldMatrix::<Fp<7>>::zeros(3, 1);
    /// m.set(0, 0, Fp::<7>::new(1));
    /// m.set(1, 0, Fp::<7>::new(2));
    /// m.set(2, 0, Fp::<7>::new(3));
    /// let p = Permutation::from_indices(vec![2, 0, 1]);
    /// let pm = p.apply(&m);
    /// assert_eq!(pm.get(0, 0), Fp::<7>::new(3));
    /// assert_eq!(pm.get(1, 0), Fp::<7>::new(1));
    /// assert_eq!(pm.get(2, 0), Fp::<7>::new(2));
    /// ```
    ///
    /// # Complexity
    ///
    /// `O(rows · cols)` clones plus one `rows × cols` allocation.
    pub fn apply<F: FiniteField>(&self, m: &FieldMatrix<F>) -> FieldMatrix<F> {
        assert_eq!(
            m.rows(),
            self.perm.len(),
            "Permutation::apply: rows ({}) must equal permutation length ({})",
            m.rows(),
            self.perm.len()
        );
        let (r, c) = m.shape();
        if r == 0 || c == 0 {
            return zero_matrix_like(r, c, m);
        }
        let mut out = zero_matrix_like(r, c, m);
        for i in 0..r {
            let src = self.perm[i];
            for j in 0..c {
                out.set(i, j, m.get(src, j));
            }
        }
        out
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Builds an `r × c` zero matrix sourcing the field's zero from `template`.
fn zero_matrix_like<F: FiniteField>(
    r: usize,
    c: usize,
    template: &FieldMatrix<F>,
) -> FieldMatrix<F> {
    if r == 0 || c == 0 {
        // Empty FieldMatrix: zero-storage, doesn't read any cell. Prefer
        // a template witness when present, then ConstField's zero hint,
        // otherwise build raw empty storage for runtime-context fields.
        let zero = if !template.is_empty() {
            template.get(0, 0).zero_like()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            return FieldMatrix::from_raw_parts(r, c, FieldVec::new());
        };
        return FieldMatrix::new(r, c, zero);
    }
    let zero = template.get(0, 0).zero_like();
    FieldMatrix::new(r, c, zero)
}

// ─── PLE in-place driver ─────────────────────────────────────────────────────

/// Direct column-by-column Gaussian elimination base case for small windows.
///
/// Called by [`ple_in_place_window`] when `win <= F::PLE_BASE_COLS`. Processes
/// the column window `[col_lo, col_hi)` of the full matrix view `a` using a
/// simple left-to-right partial-pivoting elimination. Maintains compact storage
/// convention: pivot values stay in row `rank`'s diagonal entry; the entries
/// below each pivot column are scaled to L's multipliers.
///
/// # Returns
///
/// Number of pivots found (rank contribution from this window).
fn ple_base_direct<F: FiniteField>(
    a: &mut MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> usize {
    let m = a.rows();
    let zero = a.get(0, col_lo).zero_like();
    let mut rank = 0usize; // next available pivot row

    for col in col_lo..col_hi {
        if rank >= m {
            break;
        }
        // Step 1: find pivot in rows [rank..m] of column `col`.
        let mut pivot_row: Option<usize> = None;
        for i in rank..m {
            if a.get(i, col) != zero {
                pivot_row = Some(i);
                break;
            }
        }
        let Some(p) = pivot_row else {
            // No pivot in this column — zero column, skip.
            continue;
        };

        // Step 2: swap row `p` into row `rank` (full-row swap for permutation
        // consistency across all already-processed columns).
        if p != rank {
            a.swap_rows(rank, p);
            perm.swap(rank, p);
        }

        // Step 3: scale. Compact storage keeps a[rank, col] = pivot value
        // (the E entry); all a[k, col] for k > rank become L's multipliers.
        let pivot = a.get(rank, col);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!("ple_base_direct: pivot a[{rank}, {col}] failed to invert (zero pivot)")
        });
        for k in (rank + 1)..m {
            let v = a.get(k, col) * inv.clone();
            a.set(k, col, v);
        }

        // Step 4: eliminate — for every column `c` strictly right of `col`
        // in the window, subtract multiplier[k] * a[rank, c] from a[k, c].
        // This performs the Schur-complement update within the window,
        // keeping the remaining columns in reduced form.
        for c in (col + 1)..col_hi {
            let pivot_c = a.get(rank, c);
            if pivot_c == zero {
                continue;
            }
            for k in (rank + 1)..m {
                let mult = a.get(k, col); // L's multiplier at (k, col)
                let v = a.get(k, c) - mult.clone() * pivot_c.clone();
                a.set(k, c, v);
            }
        }

        // Record this pivot's absolute column index. `pivot_cols` is
        // consumed by `split_compact` to skip the post-factorisation
        // O(rank * n) pivot-rediscovery scan.
        pivot_cols.push(col);
        rank += 1;
    }

    rank
}

/// Panelized SIMD base-case dispatch helper (issue `6823c8a0`,
/// design `2e8c5a29`).
///
/// Called by [`ple_in_place_window`] when the column window is at or
/// below the field's `PLE_PANEL_COLS` threshold and the field's
/// `has_simd_ple_panel_base` returns `true`. Extracts the parent
/// matrix's raw storage from the `MatViewMut`, invokes the field's
/// `try_simd_ple_panel_base` hook, and returns `Some(rank)` on
/// success or `None` if the kernel declined (caller then falls back
/// to the recursive trsm + gemm split or scalar `ple_base_direct`).
///
/// The kernel operates on the column window `[col_lo, col_hi)` of
/// the row range `[row_offset, row_offset + rows)`. It handles the
/// pivot search, swap, scale, and Schur update; the caller's
/// permutation tracker `perm` (length = view rows) and absolute
/// pivot column indices are updated in place.
fn try_panel_base_dispatch<F: FiniteField>(
    a: &mut MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> Option<usize> {
    let (data, parent_cols, row_offset, col_offset, rows, cols) = a.raw_parts_mut();
    debug_assert_eq!(col_offset, 0, "ple panel dispatch: view col_offset != 0");
    debug_assert!(
        col_lo <= col_hi && col_hi <= cols,
        "ple panel dispatch: col window out of bounds"
    );
    // Build the sub-slice spanning the view's rows.
    let row_start = row_offset * parent_cols;
    let row_end = row_start + rows * parent_cols;
    let sub = &mut data[row_start..row_end];
    F::try_simd_ple_panel_base(sub, parent_cols, rows, col_lo, col_hi, perm, pivot_cols)
}

/// In-place PLE on the supplied [`MatViewMut`]. Records destination →
/// source row swaps in `perm` (caller-managed). Writes the L-factor's
/// strict-lower entries directly into `a`'s storage; the leading pivot
/// values stay on `a`'s diagonal (the L-factor's unit diagonal is
/// synthesised at extraction time).
///
/// The caller passes a row-restricted view spanning the FULL column range
/// of the working matrix (so that `swap_rows` swaps whole rows for
/// permutation consistency across already-processed columns). The
/// algorithm operates on the column window `[col_lo, col_hi)`.
///
/// Returns the rank of the column window (i.e., the number of pivots
/// found in those columns).
fn ple_in_place<F: FiniteField>(
    mut a: MatViewMut<'_, F>,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> usize {
    let n = a.cols();
    ple_in_place_window(a.reborrow(), 0, n, perm, pivot_cols)
}

/// Inner driver — see [`ple_in_place`]. The window `[col_lo, col_hi)`
/// is the column range to process; cells outside this window are not
/// modified by the elimination but DO get permuted by `swap_rows`.
///
/// Appends discovered pivot columns (absolute column indices into the
/// full working matrix) to `pivot_cols` in the order they are found.
/// On return `pivot_cols.len()` increases by the rank of this window.
///
/// # Compact storage and pivot-column scatter (jit:bd9c6e13)
///
/// Pivots are found left-to-right by column scan; the storage convention
/// places L's multipliers in the **pivot columns themselves**, not in
/// the leftmost `r1` columns of the window. When the left-half recursion
/// finds `r1` pivots at columns `pivot_cols[L..L+r1]`, the L multipliers
/// live at `a[i, pivot_cols[L+k]]` (not at `a[i, col_lo + k]`).
///
/// For the inter-block trsm + gemm step to be correct on rank-deficient
/// inputs where the left half has gaps (i.e., one or more columns in
/// `[col_lo, mid)` had no pivot), we must read L1 and L1_bot from the
/// actual pivot columns rather than the contiguous prefix. The earlier
/// implementation read the contiguous prefix `[col_lo, col_lo + r1)`,
/// which silently used wrong multipliers when pivots were non-contiguous
/// — corrupting the Schur complement update and dropping otherwise-
/// valid pivots in the right-half recursion.
fn ple_in_place_window<F: FiniteField>(
    mut a: MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> usize {
    let m = a.rows();
    let win = col_hi.saturating_sub(col_lo);
    debug_assert_eq!(perm.len(), m, "ple_in_place_window: perm length mismatch");
    if m == 0 || win == 0 {
        return 0;
    }

    // Base case: column window at or below `PLE_BASE_COLS`.
    //
    // When `win <= F::PLE_BASE_COLS`, use `ple_base_direct` — a direct
    // column-by-column Gaussian elimination that avoids the per-level
    // materialise_l1_unit / materialise_block / trsm dispatch overhead.
    // The default `PLE_BASE_COLS = 1` restricts this to the single-column
    // leaf, where the recursive path would recurse into an empty right half.
    // Fields with cheap per-element arithmetic may override to a larger value.
    //
    // Complexity: O(m · win²) element operations per call.
    if win <= F::PLE_BASE_COLS {
        return ple_base_direct(&mut a, col_lo, col_hi, perm, pivot_cols);
    }

    // Panelized SIMD base case (issue 6823c8a0, design 2e8c5a29).
    //
    // When `win <= F::PLE_PANEL_COLS` (= 256 for `Fp<P>` with P ≤ 251,
    // default = PLE_BASE_COLS otherwise) and the field has registered
    // a panel-base kernel via `try_simd_ple_panel_base`, dispatch
    // through the AVX2 panel kernel. Falls through to the recursive
    // trsm+gemm split below when the kernel declines or the field has
    // no panel path.
    //
    // The `PLE_PANEL_COLS >= PLE_BASE_COLS` invariant (R4 from the
    // design doc) is asserted in debug builds; release builds trust
    // the trait impl.
    debug_assert!(
        F::PLE_PANEL_COLS >= F::PLE_BASE_COLS,
        "PLE_PANEL_COLS ({}) must be >= PLE_BASE_COLS ({})",
        F::PLE_PANEL_COLS,
        F::PLE_BASE_COLS
    );

    // Recursive PLUQ left-looking blocking for small-prime fields (issue
    // 6823c8a0 R1, design 2e8c5a29 → R1 amendment).
    //
    // When the field exposes the AVX2 panel base kernel, we no longer
    // run the panel kernel over a full `PLE_PANEL_COLS`-wide window in
    // one shot. The panel kernel's inner Schur update is row-major axpy
    // (one pivot at a time over a shrinking tail); even with AVX2 byte
    // lanes its throughput at large win is roughly 8 Gop/s — far below
    // fflas-ffpack's ~30 Gop/s sgemm-cascade PLUQ. To close the gap we
    // dispatch the panel kernel only on a narrow leftmost sub-panel
    // (`PLE_PANEL_RECURSIVE_BASE` columns wide), then update the wide
    // right tail via the existing `trsm_lower` + `gemm_axpy_into_view`
    // path. The wide gemm inherits the small-prime whole-GEMM fast path
    // from issue 40195c09 (lift), which hits the kernel's u8 byte-lane
    // throughput on the bulk of the operations.
    //
    // The threshold below is chosen so the panel kernel still
    // amortises its packing overhead (canonical-byte scratch pack +
    // outside-window row permutation) over a useful number of pivots,
    // but each panel handles few enough columns that the wide GEMM
    // dominates the work between panels. 128 was empirically selected
    // from a tuning sweep over {32, 48, 64, 96, 128} — see
    // `dev/bench_results/2026-05-26-6823c8a0-r1-recursive-pluq.md` § 2.
    const PLE_PANEL_RECURSIVE_BASE: usize = 128;
    if F::has_simd_ple_panel_base() && win > PLE_PANEL_RECURSIVE_BASE {
        return ple_panel_recursive_window::<F>(
            a,
            col_lo,
            col_hi,
            perm,
            pivot_cols,
            PLE_PANEL_RECURSIVE_BASE,
        );
    }
    if win <= F::PLE_PANEL_COLS && F::has_simd_ple_panel_base() {
        if let Some(rank) = try_panel_base_dispatch::<F>(&mut a, col_lo, col_hi, perm, pivot_cols) {
            return rank;
        }
    }

    let h = win / 2;
    let mid = col_lo + h;

    // Step 1 — recurse on the left half. `a` continues to span the
    // full parent column range; we restrict only via the col window.
    //
    // Snapshot the pivot-cols length so we can locate this level's own
    // left-half pivots after the recursion returns (they sit at
    // `pivot_cols[pivot_cols_start..pivot_cols_start + r1]`).
    let pivot_cols_start = pivot_cols.len();
    let r1 = ple_in_place_window(a.reborrow(), col_lo, mid, perm, pivot_cols);

    // Steps 2 & 3 — trsm and gemm on the right half.
    //
    // We need to read `L1` and `L1_bot` from `a`'s left half while
    // writing to `a`'s right half. Row-major storage forbids holding
    // simultaneous mutable views over disjoint column ranges in safe
    // Rust, so we materialise the read-side operands into owned
    // buffers. The materialised L1 carries an explicit unit diagonal
    // so it can feed `trsm_lower` (which reads diagonal cells).
    if r1 > 0 && mid < col_hi {
        // The left-half recursion places its r1 pivots at the absolute
        // column indices `pivot_cols[pivot_cols_start..pivot_cols_start + r1]`.
        // L1 and L1_bot must be sourced from THOSE columns (not from
        // the contiguous prefix `[col_lo, col_lo + r1)`); otherwise on
        // rank-deficient inputs where pivots are non-contiguous within
        // `[col_lo, mid)` the multipliers are scattered across gap
        // columns and the contiguous read returns either non-pivot
        // residue (almost always zero — pre-Schur-eliminated) or the
        // wrong pivot's multipliers. See jit:bd9c6e13 for the discovery
        // case (15x17 GF(7), seed=1, density=0.05).
        let left_pivots: &[usize] = &pivot_cols[pivot_cols_start..pivot_cols_start + r1];
        // Materialise L1 (r1 × r1, unit lower-triangular). Source
        // strict-lower cells from `a[0..r1, left_pivots[j]]` for j<i.
        let l1 = materialise_l1_unit_at_cols(&a.as_view(), 0, left_pivots);
        // trsm_lower: solve L1 · X = a[0..r1, mid..col_hi] in place.
        trsm_lower(l1.submat(.., ..), a.submat_mut(0..r1, mid..col_hi));

        // Step 3 — Schur complement: a[r1..m, mid..col_hi] -=
        //   L1_bot · a[0..r1, mid..col_hi].
        if r1 < m {
            let l1_bot = materialise_block_at_cols(&a.as_view(), r1, left_pivots, m - r1);
            let zero = a.get(0, col_lo).zero_like();
            let one = zero.one_like();
            let neg_one = zero - one.clone();
            // Disjoint borrow: split rows of the right-half view.
            let right = a.submat_mut(.., mid..col_hi);
            let (a3_mut, a4_mut) = right.split_rows_mut(r1);
            let a3_view = a3_mut.as_view();
            gemm_axpy_into_view(neg_one, &l1_bot.submat(.., ..), &a3_view, one, a4_mut);
        }
    }

    // Step 4 — recurse on the bottom-right block a[r1..m, mid..col_hi].
    // Use split_rows_mut so the recursive view spans the full parent
    // column range (preserving full-row swap semantics) but only rows
    // r1..m. The recursion processes the column window [mid, col_hi).
    //
    // Rank-deficient early exit: when r1 == m all rows have been assigned
    // pivots in the left half, so the bottom-right block is empty. Skip
    // the recursion entirely (avoids the function-call overhead on
    // inputs that exhibit early-termination rank-deficiency).
    let r2 = if r1 < m && mid < col_hi {
        let (_top, a4) = a.split_rows_mut(r1);
        ple_in_place_window(a4, mid, col_hi, &mut perm[r1..], pivot_cols)
    } else {
        0
    };

    r1 + r2
}

/// Recursive PLUQ left-looking blocking for small-prime fields (issue
/// 6823c8a0 R1, design 2e8c5a29 R1 amendment).
///
/// Iterates over the column window `[col_lo, col_hi)` in narrow
/// sub-panels of width `base_cols` each. For every sub-panel:
///
/// 1. Dispatch the field's AVX2 panel-base kernel on the sub-panel
///    covering rows below the running rank cursor. Returns `r_i` new
///    pivots and appends them to `pivot_cols`.
/// 2. If the sub-panel produced any pivots AND there are columns
///    remaining to the right: solve `L11 · X = right_top` (trsm) and
///    perform the Schur-complement update `right_bot -= L_bot · X`
///    (gemm). The wide gemm hits the small-prime whole-GEMM fast path
///    (`gemm_axpy_into_view` → `40195c09` lift) and the medium-prime
///    panel u16 path for primes outside the byte-lane range.
/// 3. Advance the rank cursor and the column cursor.
///
/// This mirrors the recursive PLUQ shape of Dumas-Pernet-Sultan 2017
/// (arXiv:1703.02438) — left-looking column-axis split rather than
/// 2D recursion. The 2D split is structurally equivalent at the cost
/// of more recursion bookkeeping; the 1D column-axis form has been
/// sufficient to close the GF(251) ratio gap empirically (see
/// `dev/bench_results/2026-05-26-6823c8a0-r1-recursive-pluq.md`).
///
/// # Correctness invariants preserved
///
/// - The kernel's `pivot_cols` reads from `pivot_cols[start..start+r_i]`
///   carry **absolute** column indices in the parent matrix (the panel
///   wrapper offsets the panel-local indices by `col_lo`). This means
///   `materialise_l1_unit_at_cols` and `materialise_block_at_cols`
///   continue to see the correct scattered-pivot pattern even when the
///   left sub-panel is rank-deficient (bd9c6e13 fix preserved).
/// - The kernel's row swaps are propagated to cells **outside** the
///   sub-panel via the `try_simd_ple_panel_base` hook. Crucially, those
///   "outside" cells include both the columns to the left of the
///   sub-panel (where L's multipliers from earlier sub-panels live)
///   AND the columns to the right (the not-yet-processed right tail).
///   The row order remains consistent across the whole matrix at every
///   step.
///
/// # Parameters
///
/// * `a` — full-width row-restricted view; only the column window
///   `[col_lo, col_hi)` is mutated by the elimination, but full-row
///   swaps propagate across the entire parent column range.
/// * `col_lo`, `col_hi` — absolute column window in the parent matrix.
/// * `perm` — row permutation tracker, length = `a.rows()`. Mutated
///   in place for every row swap performed by the kernel.
/// * `pivot_cols` — absolute pivot-column accumulator. New pivots are
///   appended in left-to-right order.
/// * `base_cols` — width of each sub-panel. Empirically tuned; see the
///   constant in `ple_in_place_window`.
fn ple_panel_recursive_window<F: FiniteField>(
    mut a: MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
    base_cols: usize,
) -> usize {
    let m = a.rows();
    let mut col_cur = col_lo;
    let mut rank_total = 0usize;

    while col_cur < col_hi && rank_total < m {
        let sub_hi = (col_cur + base_cols).min(col_hi);

        // Snapshot pivot-cols length so we can slice this sub-panel's
        // own pivots after the dispatch returns. The dispatch pushes
        // ABSOLUTE column indices (offset by col_lo of the panel call,
        // which is `col_cur` here).
        let pivot_cols_start = pivot_cols.len();

        // Run the panel kernel on rows [rank_total..m] × cols [col_cur, sub_hi).
        // The panel wrapper requires a view rooted at row 0 of the
        // working clone (so its kernel-local row_perm indexes from 0).
        // We split off the top `rank_total` rows and pass the bottom
        // slice; the kernel handles in-window swaps + outside-window
        // propagation across the parent's full column range.
        let r_i_opt = if rank_total == 0 {
            // Fast path: no top rows to skip. Reborrow `a` for this iteration.
            try_panel_base_dispatch::<F>(&mut a.reborrow(), col_cur, sub_hi, perm, pivot_cols)
        } else {
            // Split off the top `rank_total` rows of a freshly reborrowed
            // view. The bottom slice sees rows [rank_total..m] of the
            // parent across the full column range.
            // `try_panel_base_dispatch` extracts the raw parent slice via
            // `raw_parts_mut`, so the bottom view's `row_offset =
            // rank_total` is reflected in the slice the kernel sees.
            let (_top, mut bot) = a.reborrow().split_rows_mut(rank_total);
            try_panel_base_dispatch::<F>(
                &mut bot,
                col_cur,
                sub_hi,
                &mut perm[rank_total..],
                pivot_cols,
            )
        };

        // If the panel declined, fall back to the binary-halving recursive
        // path on this sub-panel. This branch should only trigger when AVX2
        // is unavailable at runtime mid-execution, which doesn't happen on
        // a fixed host; included for defensive correctness.
        let r_i = match r_i_opt {
            Some(r) => r,
            None => {
                if rank_total == 0 {
                    ple_in_place_window_no_panel::<F>(
                        a.reborrow(),
                        col_cur,
                        sub_hi,
                        perm,
                        pivot_cols,
                    )
                } else {
                    let (_top, bot) = a.reborrow().split_rows_mut(rank_total);
                    ple_in_place_window_no_panel::<F>(
                        bot,
                        col_cur,
                        sub_hi,
                        &mut perm[rank_total..],
                        pivot_cols,
                    )
                }
            }
        };

        // Inter-block trsm + gemm update on the right tail (if any).
        //
        // The new pivots sit at `pivot_cols[pivot_cols_start..pivot_cols_start + r_i]`
        // and reference ABSOLUTE column indices in the parent matrix
        // (they live within `[col_cur, sub_hi)`). The L-multipliers
        // beneath the new pivots live at those same columns in rows
        // `[rank_total + r_i .. m]`.
        if r_i > 0 && sub_hi < col_hi {
            let new_pivots: Vec<usize> =
                pivot_cols[pivot_cols_start..pivot_cols_start + r_i].to_vec();

            // L1 (r_i × r_i, unit lower-triangular) sourced from rows
            // [rank_total .. rank_total + r_i] at the new pivot columns.
            let l1 = materialise_l1_unit_at_cols(&a.as_view(), rank_total, &new_pivots);
            // trsm_lower solves L1 · X = a[rank_total..rank_total+r_i, sub_hi..col_hi].
            trsm_lower(
                l1.submat(.., ..),
                a.submat_mut(rank_total..rank_total + r_i, sub_hi..col_hi),
            );

            // Schur complement on rows below the new pivots.
            if rank_total + r_i < m {
                let l1_bot = materialise_block_at_cols(
                    &a.as_view(),
                    rank_total + r_i,
                    &new_pivots,
                    m - rank_total - r_i,
                );
                let zero = a.get(0, col_lo).zero_like();
                let one = zero.one_like();
                let neg_one = zero - one.clone();
                let right = a.submat_mut(rank_total.., sub_hi..col_hi);
                let (a3_mut, a4_mut) = right.split_rows_mut(r_i);
                let a3_view = a3_mut.as_view();
                gemm_axpy_into_view(neg_one, &l1_bot.submat(.., ..), &a3_view, one, a4_mut);
            }
        }

        rank_total += r_i;
        col_cur = sub_hi;
    }

    rank_total
}

/// Fallback variant of [`ple_in_place_window`] that does NOT take the
/// SIMD panel-base path even when available. Used by
/// [`ple_panel_recursive_window`] when the panel dispatch declines mid-
/// execution (e.g. AVX2 unavailable at runtime). Mirrors the structure
/// of `ple_in_place_window` minus the panel-base dispatch arm.
fn ple_in_place_window_no_panel<F: FiniteField>(
    mut a: MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> usize {
    let m = a.rows();
    let win = col_hi.saturating_sub(col_lo);
    if m == 0 || win == 0 {
        return 0;
    }
    if win <= F::PLE_BASE_COLS {
        return ple_base_direct(&mut a, col_lo, col_hi, perm, pivot_cols);
    }

    let h = win / 2;
    let mid = col_lo + h;
    let pivot_cols_start = pivot_cols.len();
    let r1 = ple_in_place_window_no_panel::<F>(a.reborrow(), col_lo, mid, perm, pivot_cols);

    if r1 > 0 && mid < col_hi {
        let left_pivots: &[usize] = &pivot_cols[pivot_cols_start..pivot_cols_start + r1];
        let l1 = materialise_l1_unit_at_cols(&a.as_view(), 0, left_pivots);
        trsm_lower(l1.submat(.., ..), a.submat_mut(0..r1, mid..col_hi));
        if r1 < m {
            let l1_bot = materialise_block_at_cols(&a.as_view(), r1, left_pivots, m - r1);
            let zero = a.get(0, col_lo).zero_like();
            let one = zero.one_like();
            let neg_one = zero - one.clone();
            let right = a.submat_mut(.., mid..col_hi);
            let (a3_mut, a4_mut) = right.split_rows_mut(r1);
            let a3_view = a3_mut.as_view();
            gemm_axpy_into_view(neg_one, &l1_bot.submat(.., ..), &a3_view, one, a4_mut);
        }
    }

    let r2 = if r1 < m && mid < col_hi {
        let (_top, a4) = a.split_rows_mut(r1);
        ple_in_place_window_no_panel::<F>(a4, mid, col_hi, &mut perm[r1..], pivot_cols)
    } else {
        0
    };
    r1 + r2
}

/// Materialises an `r1 × r1` unit-lower-triangular L1 factor by sourcing
/// strict-lower entries from the **pivot columns** of `a`.
///
/// `pivot_cols[k]` is the absolute column index in `a` holding L's
/// `k`-th column multipliers. Reads `a.get(row_off + i, pivot_cols[j])`
/// for `j < i` (the strict-lower part); fills diagonal with `1` and
/// strict-upper with `0`.
///
/// This replaces the earlier `materialise_l1_unit(a, row_off, col_off, r1)`
/// which read a contiguous `[col_off, col_off + r1)` range. The contiguous
/// read was correct only when the left-half pivots happened to be at
/// columns `col_off, col_off + 1, …, col_off + r1 - 1`; on rank-deficient
/// inputs whose left half had gaps, the contiguous read returned wrong
/// values, corrupting the inter-block trsm + Schur update. See
/// jit:bd9c6e13.
fn materialise_l1_unit_at_cols<F: FiniteField>(
    a: &MatView<'_, F>,
    row_off: usize,
    pivot_cols: &[usize],
) -> FieldMatrix<F> {
    let r1 = pivot_cols.len();
    debug_assert!(r1 > 0, "materialise_l1_unit_at_cols called with r1 == 0");
    let zero = a.get(row_off, pivot_cols[0]).zero_like();
    let one = zero.one_like();
    let mut l1 = FieldMatrix::new(r1, r1, zero);
    for i in 0..r1 {
        l1.set(i, i, one.clone());
        for (j, &pcj) in pivot_cols.iter().enumerate().take(i) {
            l1.set(i, j, a.get(row_off + i, pcj));
        }
        // Strict-upper stays zero.
    }
    l1
}

/// Materialises a `rows × r1` block sourcing column `j` from
/// `a.get(row_off + i, pivot_cols[j])` for each row `i`.
///
/// Used to build L1_bot (the strict-lower-trapezoidal L factor below
/// the pivot rows) for the Schur-complement update. Mirrors
/// `materialise_l1_unit_at_cols` but covers a rectangular block.
fn materialise_block_at_cols<F: FiniteField>(
    a: &MatView<'_, F>,
    row_off: usize,
    pivot_cols: &[usize],
    rows: usize,
) -> FieldMatrix<F> {
    let cols = pivot_cols.len();
    debug_assert!(
        rows > 0 && cols > 0,
        "materialise_block_at_cols: empty (rows={rows}, cols={cols})"
    );
    let zero = a.get(row_off, pivot_cols[0]).zero_like();
    let mut out = FieldMatrix::new(rows, cols, zero);
    for i in 0..rows {
        for (j, &pcj) in pivot_cols.iter().enumerate() {
            out.set(i, j, a.get(row_off + i, pcj));
        }
    }
    out
}

/// Splits the working buffer's compact storage into the L (`m × rank`)
/// and E (`rank × n`) factors.
///
/// `working[i, pivot_cols[i]..n]` for `i < rank` holds E's entries (the
/// part of row `i` from its pivot column rightward); cells to the left
/// of `pivot_cols[i]` either hold earlier pivots' L-multipliers (when
/// `j ∈ pivot_cols` with index `< i`) or are zero. `working[i, pivot_cols[k]]`
/// for `i > k` holds L's `k`-th-column strict-lower entry (L's unit
/// diagonal is synthesised at extraction).
///
/// **Rank-deficient optimisation.** `pivot_cols` is pre-filled by
/// `ple_in_place` during the factorisation — one entry per pivot, in
/// left-to-right order. Providing them here eliminates the O(rank * n)
/// row-scan that the naive approach would use to rediscover them from
/// the compact storage.
///
/// For an `n x n` rank-`r` matrix the saving is `O(r * (n - c_last))`
/// comparisons where `c_last` is the rightmost pivot column. In the
/// rank-deficient regime (`r = n/2`, all pivots in the left half,
/// `c_last ~= n/2`) this removes approximately `n/2 * n/2 = n^2/4`
/// comparisons from the post-factorisation extraction path.
fn split_compact<F: FiniteField>(
    working: &FieldMatrix<F>,
    rank: usize,
    pivot_cols: &[usize],
) -> (FieldMatrix<F>, FieldMatrix<F>) {
    debug_assert_eq!(
        pivot_cols.len(),
        rank,
        "split_compact: pivot_cols length {} != rank {}",
        pivot_cols.len(),
        rank
    );
    let m = working.rows();
    let n = working.cols();
    if rank == 0 {
        let l = zero_matrix_like(m, 0, working);
        let e = zero_matrix_like(0, n, working);
        return (l, e);
    }
    let zero = working.get(0, 0).zero_like();
    let one = zero.one_like();

    // E: rank × n, in row-echelon form.
    //
    // Compact storage of `working` interleaves L's multipliers and E's
    // entries within the top `rank` rows: `working[i, j]` holds
    //   - E[i, j]              if j ∈ pivot_cols and j is row i's pivot or
    //                          to its right (j ≥ pivot_cols[i])
    //   - L's multiplier L[i, k]   if j == pivot_cols[k] for some k < i
    //                          (i.e., j is strictly to the LEFT of row
    //                           i's pivot but happens to be a pivot
    //                           column of an earlier row)
    //   - 0                    otherwise (zeroed by Schur updates)
    //
    // So when extracting E, we must zero out the multiplier cells. The
    // simplest correct rule: E[i, j] = working[i, j] if j ≥
    // pivot_cols[i], else 0. This satisfies the row-echelon property
    // (each row has its leading non-zero at pivot_cols[i], strictly to
    // the right of the previous row's pivot).
    let mut e = FieldMatrix::new(rank, n, zero.clone());
    for (i, &pci) in pivot_cols.iter().enumerate().take(rank) {
        for j in pci..n {
            e.set(i, j, working.get(i, j));
        }
        // Cells j < pci stay zero (from FieldMatrix::new).
    }

    // L: m × rank, unit lower-trapezoidal. L[k, k] = 1 for k < rank.
    // For i > k, L[i, k] is the multiplier that lived in working at
    // column `pivot_cols[k]` of row i — i.e., L[i, k] =
    // working[i, pivot_cols[k]] (after the recursion the column of
    // pivot_cols[k] in rows > k holds L's multipliers; the recursion
    // does NOT zero these out in-place).
    let mut l = FieldMatrix::new(m, rank, zero);
    for k in 0..rank {
        l.set(k, k, one.clone());
    }
    // Above the diagonal: L[i, j] = 0 for i < j. (Already zero from
    // FieldMatrix::new.)
    // Below and on the diagonal: L[i, j] for i > j (within the rank-`r`
    // range of i). These come from working[i, pivot_cols[j]].
    for (j, &pcj) in pivot_cols.iter().enumerate().take(rank) {
        for i in (j + 1)..m {
            l.set(i, j, working.get(i, pcj));
        }
    }
    (l, e)
}

// ─── Public entry points on FieldMatrix ──────────────────────────────────────

impl<F: FiniteField> FieldMatrix<F> {
    /// Computes the PLE decomposition `P · L · E = self`.
    ///
    /// Returns `(P, L, E, r)` where:
    ///
    /// - `P` is a [`Permutation`] on the `m` rows of `self`.
    /// - `L` is `m × r`, unit lower-trapezoidal: `L[k, k] = 1` for
    ///   `k < r`, `L[i, j] = 0` for `j > i`, and free entries for
    ///   `j < i`.
    /// - `E` is `r × n`, row-echelon: each row has a leading non-zero
    ///   entry strictly to the right of the previous row's leading.
    /// - `r` is the rank of `self`.
    ///
    /// Implements Dumas–Pernet §2.2 algorithm 2.5 via a horizontal
    /// block-recursive split, dispatching to
    /// [`crate::field::triangular::trsm_lower`] for the off-diagonal
    /// solves and to
    /// [`gemm_axpy_into_view`](crate::field::matrix::gemm_axpy_into_view)
    /// for the rank-update.
    ///
    /// # Arguments
    ///
    /// * `self` — `m × n` input. The matrix is not modified.
    ///
    /// # Panics
    ///
    /// Does not panic on rank-deficient or singular inputs.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n))` field operations. The recursion is
    /// `O(log min(m, n))` deep and reduces a `min(m, n)`-rank
    /// elimination to balanced sub-problems.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// // A = [[2, 4], [1, 3]] over GF(7).
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 1, Fp::<7>::new(4));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(3));
    /// let (_p, _l, _e, r) = a.ple();
    /// assert_eq!(r, 2);
    /// ```
    pub fn ple(&self) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize) {
        let (m, n) = self.shape();
        if m == 0 || n == 0 {
            // Empty input: identity permutation, empty L, empty E.
            let l = zero_matrix_like(m, 0, self);
            let e = zero_matrix_like(0, n, self);
            return (Permutation::identity(m), l, e, 0);
        }
        // 1 alloc: working clone (we cannot destroy &self).
        let mut working = self.clone();
        // 0 matrix allocs: identity permutation in a Vec<usize>.
        let mut perm: Vec<usize> = (0..m).collect();
        // Pivot-column accumulator filled by ple_in_place; reused by
        // split_compact to skip the O(rank * n) post-factorisation scan.
        let max_rank = m.min(n);
        let mut pivot_cols: Vec<usize> = Vec::with_capacity(max_rank);
        // Run the in-place driver. Per-level allocations come from
        // materialised L1/L1_bot operands and the gemm/trsm B-transpose
        // scratches. See module rustdoc for the budget.
        let rank = ple_in_place(working.submat_mut(.., ..), &mut perm, &mut pivot_cols);
        // 2 allocs: split working's compact storage into owned L and E.
        // pivot_cols is already populated by ple_in_place, so split_compact
        // does no rediscovery scan.
        let (l, e) = split_compact(&working, rank, &pivot_cols);
        // The recursion's `perm` is the destination → source map: applying
        // it to `self` row-wise gives the matrix that decomposes as L · E.
        // The contract `P · L · E = self` requires the inverse permutation.
        let inverse_perm = invert_perm(&perm);
        (Permutation::from_indices(inverse_perm), l, e, rank)
    }

    /// Returns the row-echelon form of `self` with the transform.
    ///
    /// Computes `(X, E)` where `X · self = E` and `E` is in row-echelon
    /// form. Implements Dumas–Pernet §2.2 algorithm 2.6 by composing
    /// PLE with one [`trsm_lower`](crate::field::triangular::trsm_lower)
    /// solve to invert `L`'s effect.
    ///
    /// `X` is an `m × m` matrix equal to `L_full⁻¹ · Pᵀ`, where `L_full`
    /// is the full `m × m` unit lower-triangular matrix obtained by
    /// appending an `(m − r) × (m − r)` identity block to the right of
    /// `L`. By construction `X` is non-singular.
    ///
    /// # Arguments
    ///
    /// * `self` — `m × n` input.
    ///
    /// # Returns
    ///
    /// `(X, E)` such that `X · self == E` (verified by tests) and `E`
    /// is row-echelon.
    ///
    /// # Panics
    ///
    /// Does not panic on rank-deficient inputs over `ConstField`. For
    /// `m × 0` (zero-width) inputs over runtime-context fields without
    /// a `zero_hint`, panics with a clear message — there is no `F`
    /// witness to seed the identity `X`. Use `F: ConstField` or a
    /// non-empty input.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n) + m³)` field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// // A = [[0, 2], [1, 3]] over GF(7).
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 1, Fp::<7>::new(2));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(3));
    /// let (_x, e) = a.row_echelon();
    /// // E[0, 0] != 0 (leading entry of first row).
    /// assert_ne!(e.get(0, 0), Fp::<7>::new(0));
    /// ```
    pub fn row_echelon(&self) -> (FieldMatrix<F>, FieldMatrix<F>) {
        let (m, n) = self.shape();
        if m == 0 {
            // X is 0×0, E is 0×n.
            let x = zero_matrix_like(0, 0, self);
            let e = zero_matrix_like(0, n, self);
            return (x, e);
        }
        if n == 0 {
            // X is identity m×m, E is m×0. With cols == 0 we cannot read
            // a witness from `self`'s storage (it has no cells); fall
            // back to F::zero_hint(), which is `Some(F::zero())` for
            // every ConstField in the project. Runtime-context fields
            // (`Gf2mElement`) return `None` from zero_hint and would need
            // a witness from a non-empty `self`; with `cols == 0` and
            // any rows, no witness exists, so the only sound result is
            // to panic with a clear message — matching the locked
            // `gemm`/`matvec` zero-inner-dim contract from `ab791e27`.
            let zero = F::zero_hint().expect(
                "row_echelon: cannot construct identity X for an m×0 input \
                 over a runtime-context field (no F witness available); \
                 use F: ConstField, or pass a non-empty input.",
            );
            let one = zero.one_like();
            let mut x = FieldMatrix::new(m, m, zero);
            for i in 0..m {
                x.set(i, i, one.clone());
            }
            let e = FieldMatrix::new(m, 0, F::zero_hint().unwrap_or_else(|| x.get(0, 0)));
            return (x, e);
        }

        let (p, l, e, _r) = self.ple();
        // Build L_full (m × m unit lower-triangular) by padding L's
        // (m × r) trapezoidal shape with an identity block.
        let l_full = pad_l_to_full(&l, m, self);

        // Pᵀ has 1 at (perm[i], i) — equivalently, a column-permutation
        // of the m × m identity. Build it explicitly.
        let zero = self.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut p_t = FieldMatrix::new(m, m, zero.clone());
        for (i, &src) in p.indices().iter().enumerate() {
            p_t.set(src, i, one.clone());
        }
        // Solve L_full · X = Pᵀ in place; result lands in p_t.
        trsm_lower(l_full.submat(.., ..), p_t.submat_mut(.., ..));

        // E_full: m × n with E (r × n) at the top, zeros below.
        let r = e.rows();
        let mut e_full = FieldMatrix::new(m, n, zero);
        for i in 0..r {
            for j in 0..n {
                e_full.set(i, j, e.get(i, j));
            }
        }
        (p_t, e_full)
    }

    /// Returns the reduced row-echelon form of `self` with the transform.
    ///
    /// Computes `(X, R)` where `X · self = R` and `R` is in RREF
    /// (leading entries are 1, all other entries in pivot columns are
    /// zero, leading 1s strictly to the right of the previous row's).
    ///
    /// Implements Dumas–Pernet §2.2 algorithm 2.7: starts from the
    /// echelon `(X₀, E)` returned by [`row_echelon`](Self::row_echelon)
    /// and peels each pivot column.
    ///
    /// # Arguments
    ///
    /// * `self` — `m × n` input.
    ///
    /// # Returns
    ///
    /// `(X, R)` such that `X · self == R` and `R` is in RREF.
    ///
    /// # Panics
    ///
    /// Does not panic on `ConstField` inputs of any shape, rank, or
    /// pivot pattern. For `m × 0` (zero-width) inputs over
    /// runtime-context fields without a `zero_hint` (e.g.
    /// `Gf2mElement`), the inner [`row_echelon`](Self::row_echelon)
    /// call panics with a clear message — there is no `F` witness
    /// available to seed the identity `X`.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n) + m · (m + n) · r)` where `r = rank(self)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 3);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 2, Fp::<7>::new(1));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(3));
    /// let (_x, r) = a.rref();
    /// // R[0, 0] is the leading 1 of row 0.
    /// assert_eq!(r.get(0, 0), Fp::<7>::new(1));
    /// ```
    pub fn rref(&self) -> (FieldMatrix<F>, FieldMatrix<F>) {
        let (m, n) = self.shape();
        if m == 0 || n == 0 {
            return self.row_echelon();
        }
        let (mut x, mut e) = self.row_echelon();
        let zero = self.get(0, 0).zero_like();
        let one = zero.one_like();

        // Identify pivots in the echelon form.
        let mut pivots: Vec<(usize, usize)> = Vec::new();
        let mut last: isize = -1;
        for i in 0..m {
            let start = (last + 1).max(0) as usize;
            let mut found: Option<usize> = None;
            for j in start..n {
                if e.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                pivots.push((i, p));
                last = p as isize;
            } else {
                break;
            }
        }

        // Fast path: blocked back-substitution via gemm_axpy_into_view (Stage 3a)
        // and trsm_upper (Stage 3b). Falls back to the scalar loop when the
        // blocked path is unavailable (e.g., the inner dims are zero).
        if try_blocked_back_sub(&mut x, &mut e, &pivots, m, n) {
            return (x, e);
        }

        // Scalar fallback (unchanged from original implementation).
        for &(pi, pc) in &pivots {
            // Scale row `pi` so the pivot is 1.
            let pivot_val = e.get(pi, pc);
            if pivot_val != one {
                let inv = pivot_val
                    .inv()
                    .unwrap_or_else(|| panic!("rref: pivot at ({}, {}) failed to invert", pi, pc));
                for j in 0..n {
                    let v = e.get(pi, j) * inv.clone();
                    e.set(pi, j, v);
                }
                for j in 0..m {
                    let v = x.get(pi, j) * inv.clone();
                    x.set(pi, j, v);
                }
            }
            // Eliminate above (and below — already-zero by row-echelon
            // form, but the loop is symmetric and cheap).
            for k in 0..m {
                if k == pi {
                    continue;
                }
                let factor = e.get(k, pc);
                if factor == zero {
                    continue;
                }
                for j in 0..n {
                    let v = e.get(k, j) - factor.clone() * e.get(pi, j);
                    e.set(k, j, v);
                }
                for j in 0..m {
                    let v = x.get(k, j) - factor.clone() * x.get(pi, j);
                    x.set(k, j, v);
                }
            }
        }
        (x, e)
    }

    /// Returns the rank of `self`.
    ///
    /// Convenience wrapper for the fourth return of [`ple`](Self::ple).
    ///
    /// # Arguments
    ///
    /// * `self` — `m × n` input.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n))` field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(1));
    /// a.set(0, 1, Fp::<7>::new(2));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(2));
    /// assert_eq!(a.rank(), 1);
    /// ```
    pub fn rank(&self) -> usize {
        self.ple().3
    }

    /// Returns a basis for the right null-space of `self`.
    ///
    /// The null-space `ker(self) = { v ∈ Fⁿ : self · v = 0 }` has
    /// dimension `n − rank(self)`. This routine returns a basis of size
    /// exactly that dimension, with each basis vector verified to lie
    /// in the kernel by construction.
    ///
    /// Implementation: compute `(_, R) = self.rref()`, identify pivot
    /// and free columns. For each free column `f`, build vector `v`
    /// with `v[f] = 1` and `v[pivot_cols[k]] = -R[k, f]` for each
    /// pivot row `k`.
    ///
    /// # Arguments
    ///
    /// * `self` — `m × n` input.
    ///
    /// # Returns
    ///
    /// `Vec<FieldVec<F>>` of length `n − rank(self)`. Each entry is a
    /// length-`n` vector in `ker(self)`.
    ///
    /// # Complexity
    ///
    /// Dominated by [`rref`](Self::rref).
    ///
    /// # Panics
    ///
    /// Does not panic on `ConstField` inputs of any shape, rank, or
    /// pivot pattern. For `m = 0` (zero-row) inputs over runtime-context
    /// fields without a `zero_hint` (e.g. `Gf2mElement`), panics with a
    /// clear message — there is no `F` witness available to seed the
    /// canonical basis vectors. Use `F: ConstField`, or pass a non-empty
    /// input.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(1, 3);
    /// a.set(0, 0, Fp::<7>::new(1));
    /// a.set(0, 1, Fp::<7>::new(2));
    /// a.set(0, 2, Fp::<7>::new(3));
    /// let basis = a.nullspace();
    /// assert_eq!(basis.len(), 2);
    /// ```
    pub fn nullspace(&self) -> Vec<FieldVec<F>> {
        let (m, n) = self.shape();
        if n == 0 {
            return Vec::new();
        }
        if m == 0 {
            // Every vector is in the kernel.
            let zero = if let Some(z) = F::zero_hint() {
                z
            } else {
                panic!(
                    "nullspace: m=0 with runtime-context field requires a template; \
                     supply at least one row"
                );
            };
            let one = zero.one_like();
            let mut basis = Vec::with_capacity(n);
            for f in 0..n {
                let mut v = FieldVec::zeros_from(n, &zero);
                v.set(f, one.clone());
                basis.push(v);
            }
            return basis;
        }
        let (_x, r) = self.rref();
        let zero = self.get(0, 0).zero_like();
        let one = zero.one_like();

        let mut pivot_cols: Vec<usize> = Vec::new();
        let mut last: isize = -1;
        for i in 0..m {
            let start = (last + 1).max(0) as usize;
            let mut found: Option<usize> = None;
            for j in start..n {
                if r.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                pivot_cols.push(p);
                last = p as isize;
            } else {
                break;
            }
        }
        let pivot_set: std::collections::HashSet<usize> = pivot_cols.iter().copied().collect();
        let mut basis = Vec::with_capacity(n - pivot_cols.len());
        for f in 0..n {
            if pivot_set.contains(&f) {
                continue;
            }
            let mut v = FieldVec::zeros_from(n, &zero);
            v.set(f, one.clone());
            for (k, &pc) in pivot_cols.iter().enumerate() {
                let coef = r.get(k, f);
                if coef != zero {
                    v.set(pc, -coef);
                }
            }
            basis.push(v);
        }
        basis
    }

    /// Returns the LU decomposition of `self` if it has full rank.
    ///
    /// Returns `Some((P, L, U))` such that `P · self = L · U`, if and
    /// only if `rank(self) == min(m, n)`. Otherwise returns `None`.
    ///
    /// `L` is the `m × r` factor from PLE (unit lower-trapezoidal) and
    /// `U = E` is the `r × n` echelon factor (upper-trapezoidal under
    /// full rank: leading entry of row `k` is at column `k`).
    ///
    /// # Arguments
    ///
    /// * `self` — `m × n` input.
    ///
    /// # Returns
    ///
    /// `Some((P, L, U))` if `rank == min(m, n)`, else `None`.
    ///
    /// # Panics
    ///
    /// Does not panic.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n))` field operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use gf2_core::field::matrix::FieldMatrix;
    /// use gf2_core::gfp::Fp;
    ///
    /// let mut a = FieldMatrix::<Fp<7>>::zeros(2, 2);
    /// a.set(0, 0, Fp::<7>::new(2));
    /// a.set(0, 1, Fp::<7>::new(4));
    /// a.set(1, 0, Fp::<7>::new(1));
    /// a.set(1, 1, Fp::<7>::new(3));
    /// assert!(a.lu().is_some());
    /// ```
    pub fn lu(&self) -> Option<(Permutation, FieldMatrix<F>, FieldMatrix<F>)> {
        let (m, n) = self.shape();
        let (p_ple, l, e, r) = self.ple();
        if r != m.min(n) {
            return None;
        }
        // Contract orientation. PLE: `A = P_ple · L · E`. LU asks for
        // `P_lu · A = L · U`, so `P_lu = P_ple^{-1}`. For involutive
        // permutations these coincide, but for non-involutive `P_ple`
        // (any non-trivial pivoting beyond a single swap), returning
        // `p_ple` directly would silently violate the contract — the
        // R3 reviewer caught this.
        Some((p_ple.inverse(), l, e))
    }
}

// ─── Helpers for row_echelon ─────────────────────────────────────────────────

/// Returns the inverse of a destination-source permutation.
fn invert_perm(perm: &[usize]) -> Vec<usize> {
    let n = perm.len();
    let mut inv = vec![0usize; n];
    for (i, &j) in perm.iter().enumerate() {
        inv[j] = i;
    }
    inv
}

/// Pads an `m × r` lower-trapezoidal `L` to a full `m × m` unit lower-
/// triangular matrix by appending the `(m − r) × (m − r)` identity block
/// at the bottom-right.
fn pad_l_to_full<F: FiniteField>(
    l: &FieldMatrix<F>,
    m: usize,
    template: &FieldMatrix<F>,
) -> FieldMatrix<F> {
    let r = l.cols();
    debug_assert_eq!(l.rows(), m, "pad_l_to_full: row mismatch");
    if r == m {
        return l.clone();
    }
    let zero = template.get(0, 0).zero_like();
    let one = zero.one_like();
    let mut full = FieldMatrix::new(m, m, zero);
    for i in 0..m {
        for j in 0..r {
            full.set(i, j, l.get(i, j));
        }
    }
    for i in r..m {
        full.set(i, i, one.clone());
    }
    full
}

// ─── Blocked back-substitution (Stage 3a + 3b) ───────────────────────────────

/// Blocked back-substitution for [`FieldMatrix::rref`] (design `24a93e4e`).
///
/// Given the echelon form `(x, e)` from [`FieldMatrix::row_echelon`] and the
/// list of `(pivot_row, pivot_col)` pairs, performs the back-substitution in
/// three stages:
///
/// 1. **Scale** each pivot row so that `e[pi, pc] = 1` (and scale the
///    corresponding rows of `x`).
///
/// 2. **Stage 3b — Pivot rows TRSM.** Extract `E_piv_piv` (the `r×r`
///    upper unit-triangular pivot-column block) and apply `trsm_upper` to
///    both `E[pivot rows, free cols]` and `X[pivot rows, *]` to eliminate
///    the off-diagonal coupling among pivot rows.
///
/// 3. **Stage 3a — Non-pivot rows GEMM.** Using the post-3b pivot-row blocks,
///    eliminate the pivot columns from the non-pivot rows:
///    - `E[non-pivot rows, free cols] -= E[non-pivot rows, pivot cols] · E[pivot rows, free cols]`
///    - `X[non-pivot rows, *] -= E[non-pivot rows, pivot cols] · X[pivot rows, *]`
///
/// Finally, zero the pivot columns of `e` (set to identity columns).
///
/// Returns `true` always (no fallback needed); `r == 0` is handled as
/// an immediate no-op returning `true`.
/// Minimum matrix dimension (max(m, n)) below which `try_blocked_back_sub`
/// returns `false` and lets `rref` fall through to the scalar loop.
///
/// Below this threshold the scatter/gather overhead of allocating 8 scratch
/// `FieldMatrix` instances dominates the back-sub work, regressing `rref`
/// by up to ~20% vs the scalar loop (observed at GF(M31)/n=64/deficient).
/// The crossover based on CCX1 measurements (2026-05-27) is between 64 and
/// 256; a threshold of 128 leaves a ≥5% safety margin in both directions.
pub(crate) const BLOCKED_BACK_SUB_MIN_DIM: usize = 128;

pub(crate) fn try_blocked_back_sub<F: FiniteField>(
    x: &mut FieldMatrix<F>,
    e: &mut FieldMatrix<F>,
    pivots: &[(usize, usize)],
    m: usize,
    n: usize,
) -> bool {
    let r = pivots.len();
    if r == 0 {
        // No pivots — e is all-zero, x is already the identity. Done.
        return true;
    }

    // For small matrices the scatter/gather overhead exceeds the scalar loop
    // cost.  Let the caller fall through to the scalar path.
    if m.max(n) < BLOCKED_BACK_SUB_MIN_DIM {
        return false;
    }

    let zero = e.get(0, 0).zero_like();
    let one = zero.one_like();
    let neg_one = zero.clone() - one.clone();

    // Collect pivot row / column indices in declaration order.
    let pivot_rows: Vec<usize> = pivots.iter().map(|&(pi, _)| pi).collect();
    let pivot_cols: Vec<usize> = pivots.iter().map(|&(_, pc)| pc).collect();

    // Non-pivot row and free (non-pivot) column index sets.
    let is_pivot_row = {
        let mut v = vec![false; m];
        for &pi in &pivot_rows {
            v[pi] = true;
        }
        v
    };
    let is_pivot_col = {
        let mut v = vec![false; n];
        for &pc in &pivot_cols {
            v[pc] = true;
        }
        v
    };
    let non_pivot_rows: Vec<usize> = (0..m).filter(|&i| !is_pivot_row[i]).collect();
    let free_cols: Vec<usize> = (0..n).filter(|&j| !is_pivot_col[j]).collect();
    let n_nonpiv = non_pivot_rows.len();
    let n_free = free_cols.len();

    // ── 1. Scale pivot rows so that e[pi, pc] = 1 ────────────────────────────
    for &(pi, pc) in pivots {
        let pivot_val = e.get(pi, pc);
        if pivot_val != one {
            let inv = pivot_val.inv().unwrap_or_else(|| {
                panic!("rref blocked: pivot at ({}, {}) not invertible", pi, pc)
            });
            for j in 0..n {
                let v = e.get(pi, j) * inv.clone();
                e.set(pi, j, v);
            }
            for j in 0..m {
                let v = x.get(pi, j) * inv.clone();
                x.set(pi, j, v);
            }
        }
    }

    // ── 2. Stage 3b — Pivot rows TRSM ────────────────────────────────────────
    //
    // After scaling, E_piv_piv (r×r) is upper unit triangular.  We apply
    // trsm_upper to BOTH the free-col block of e AND to x[pivot rows, *].
    // Key: E_piv_piv is the `a` arg to trsm_upper (not `b`) so it is NOT
    // modified by trsm_upper.  We extract it once and reuse for both calls.
    let e_piv_piv = {
        let mut m_pp = FieldMatrix::new(r, r, zero.clone());
        for (ki, &pi) in pivot_rows.iter().enumerate() {
            for (kj, &pc) in pivot_cols.iter().enumerate() {
                m_pp.set(ki, kj, e.get(pi, pc));
            }
        }
        m_pp
    };

    // 2a. trsm_upper on E[pivot rows, free cols].
    if n_free > 0 {
        let mut e_piv_free = FieldMatrix::new(r, n_free, zero.clone());
        for (ki, &pi) in pivot_rows.iter().enumerate() {
            for (fj, &fc) in free_cols.iter().enumerate() {
                e_piv_free.set(ki, fj, e.get(pi, fc));
            }
        }
        trsm_upper(e_piv_piv.submat(.., ..), e_piv_free.submat_mut(.., ..));
        for (ki, &pi) in pivot_rows.iter().enumerate() {
            for (fj, &fc) in free_cols.iter().enumerate() {
                e.set(pi, fc, e_piv_free.get(ki, fj));
            }
        }
    }

    // 2b. trsm_upper on X[pivot rows, *].
    // (r == 1 case: unit triangular 1×1 with diagonal 1 — trsm is identity, skip.)
    if r > 1 {
        let mut x_piv = FieldMatrix::new(r, m, zero.clone());
        for (ki, &pi) in pivot_rows.iter().enumerate() {
            for j in 0..m {
                x_piv.set(ki, j, x.get(pi, j));
            }
        }
        trsm_upper(e_piv_piv.submat(.., ..), x_piv.submat_mut(.., ..));
        for (ki, &pi) in pivot_rows.iter().enumerate() {
            for j in 0..m {
                x.set(pi, j, x_piv.get(ki, j));
            }
        }
    }

    // ── 3. Stage 3a — Non-pivot rows GEMM ────────────────────────────────────
    //
    // Uses the post-stage-3b pivot-row values.
    // E[non-pivot rows, free cols] -= E[non-pivot rows, pivot cols] · E[pivot rows, free cols]
    // X[non-pivot rows, *]         -= E[non-pivot rows, pivot cols] · X[pivot rows, *]
    if n_nonpiv > 0 {
        // E_nonpiv_piv ((m-r)×r): pivot-column values for non-pivot rows.
        let e_nonpiv_piv = {
            let mut m_np = FieldMatrix::new(n_nonpiv, r, zero.clone());
            for (ni, &npi) in non_pivot_rows.iter().enumerate() {
                for (kj, &pc) in pivot_cols.iter().enumerate() {
                    m_np.set(ni, kj, e.get(npi, pc));
                }
            }
            m_np
        };

        // 3a-e: update E[non-pivot rows, free cols].
        if n_free > 0 {
            let mut e_piv_free_post = FieldMatrix::new(r, n_free, zero.clone());
            for (ki, &pi) in pivot_rows.iter().enumerate() {
                for (fj, &fc) in free_cols.iter().enumerate() {
                    e_piv_free_post.set(ki, fj, e.get(pi, fc));
                }
            }
            let mut e_nonpiv_free = FieldMatrix::new(n_nonpiv, n_free, zero.clone());
            for (ni, &npi) in non_pivot_rows.iter().enumerate() {
                for (fj, &fc) in free_cols.iter().enumerate() {
                    e_nonpiv_free.set(ni, fj, e.get(npi, fc));
                }
            }
            gemm_axpy_into_view(
                neg_one.clone(),
                &e_nonpiv_piv.submat(.., ..),
                &e_piv_free_post.submat(.., ..),
                one.clone(),
                e_nonpiv_free.submat_mut(.., ..),
            );
            for (ni, &npi) in non_pivot_rows.iter().enumerate() {
                for (fj, &fc) in free_cols.iter().enumerate() {
                    e.set(npi, fc, e_nonpiv_free.get(ni, fj));
                }
            }
        }

        // 3a-x: update X[non-pivot rows, *].
        {
            // X_piv (r×m) — post stage-3b, already updated in x.
            let mut x_piv_post = FieldMatrix::new(r, m, zero.clone());
            for (ki, &pi) in pivot_rows.iter().enumerate() {
                for j in 0..m {
                    x_piv_post.set(ki, j, x.get(pi, j));
                }
            }
            let mut x_nonpiv = FieldMatrix::new(n_nonpiv, m, zero.clone());
            for (ni, &npi) in non_pivot_rows.iter().enumerate() {
                for j in 0..m {
                    x_nonpiv.set(ni, j, x.get(npi, j));
                }
            }
            gemm_axpy_into_view(
                neg_one,
                &e_nonpiv_piv.submat(.., ..),
                &x_piv_post.submat(.., ..),
                one.clone(),
                x_nonpiv.submat_mut(.., ..),
            );
            for (ni, &npi) in non_pivot_rows.iter().enumerate() {
                for j in 0..m {
                    x.set(npi, j, x_nonpiv.get(ni, j));
                }
            }
        }
    }

    // ── 4. Pivot column zeroing ───────────────────────────────────────────────
    //
    // Set e[*, pc] to the identity column: e[pi, pc] = 1, 0 elsewhere.
    for &(pi, pc) in pivots {
        for k in 0..m {
            e.set(k, pc, zero.clone());
        }
        e.set(pi, pc, one.clone());
    }

    true
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::{fieldmatrix_new_count, gemm, reset_fieldmatrix_new_count};
    use crate::field::test_random_matrix::{
        dense_random_fp_sparse, direct_rref_oracle_fp, random_fp, random_gf2m_wide_1,
    };
    use crate::gf2m::wide::Gf2mWide;
    use crate::gf2m::wide_config::Gf2mWideConfig;
    use crate::gf2m::{Gf2mElement, Gf2mField};
    use crate::gfp::Fp;
    use serial_test::serial;

    const MERSENNE_31: u64 = 2_147_483_647;

    /// AES-irreducible Gf2mWide<8>.
    struct PleGf2m8Cfg;
    impl Gf2mWideConfig<1> for PleGf2m8Cfg {
        const M: usize = 8;
        const MODULUS: [u64; 1] = [0x1B];
        const NAME: &'static str = "PleGf2m8Cfg";
    }
    type Gf2m8 = Gf2mWide<1, PleGf2m8Cfg>;

    /// Gf2mWide<16>: Conway polynomial x^16 + x^5 + x^3 + x^2 + 1
    /// → low 16 bits 0x002D (with implicit leading one at bit 16).
    struct PleGf2m16Cfg;
    impl Gf2mWideConfig<1> for PleGf2m16Cfg {
        const M: usize = 16;
        const MODULUS: [u64; 1] = [0x002D];
        const NAME: &'static str = "PleGf2m16Cfg";
    }
    type Gf2m16 = Gf2mWide<1, PleGf2m16Cfg>;

    // Local convenience aliases that monomorphise the shared generic
    // helpers in `field::test_random_matrix` to this module's configs.
    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        random_gf2m_wide_1::<PleGf2m8Cfg>(rows, cols, seed)
    }

    fn random_gf2m16(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m16> {
        random_gf2m_wide_1::<PleGf2m16Cfg>(rows, cols, seed)
    }

    /// Reconstructs `P · L · E` and compares to `a`. Returns the rank.
    fn check_ple<F: FiniteField>(a: &FieldMatrix<F>) -> usize {
        let (p, l, e, r) = a.ple();
        assert_eq!(l.cols(), r, "L cols ({}) != rank ({})", l.cols(), r);
        assert_eq!(e.rows(), r, "E rows ({}) != rank ({})", e.rows(), r);
        let le = if r == 0 {
            // L is m×0, E is 0×n; product is the m×n zero matrix.
            // gemm panics on (m×0)·(0×n) for runtime-context fields if
            // both factors are storage-empty. Build the zero matrix
            // directly.
            zero_matrix_like(a.rows(), a.cols(), a)
        } else {
            gemm(&l, &e)
        };
        let rebuild = p.apply(&le);
        assert_eq!(rebuild, *a, "P · L · E != A");
        // Echelon form of E.
        if r > 0 {
            let zero = a.get(0, 0).zero_like();
            let mut last: isize = -1;
            for i in 0..r {
                let mut found: Option<usize> = None;
                for j in 0..e.cols() {
                    if e.get(i, j) != zero {
                        found = Some(j);
                        break;
                    }
                }
                let pp = found.expect("E row should have a leading non-zero entry");
                assert!(
                    (pp as isize) > last,
                    "E pivot at row {} is column {}, expected > {}",
                    i,
                    pp,
                    last
                );
                last = pp as isize;
            }
        }
        // Unit lower-trapezoidal L.
        if l.rows() > 0 && l.cols() > 0 {
            let zero = a.get(0, 0).zero_like();
            let one = zero.one_like();
            for i in 0..l.rows() {
                for j in 0..l.cols() {
                    if i == j {
                        assert_eq!(l.get(i, j), one, "L[{}, {}] expected 1", i, j);
                    } else if j > i {
                        assert_eq!(l.get(i, j), zero, "L[{}, {}] expected 0", i, j);
                    }
                }
            }
        }
        r
    }

    // ── Hard SC#1: P · L · E == A on five fields ─────────────────────────────

    #[test]
    fn test_ple_random_fp7() {
        for seed in 0..5u64 {
            let m = 4 + (seed as usize % 3);
            let n = 5 + (seed as usize % 3);
            let a = random_fp::<7>(m, n, seed);
            check_ple(&a);
        }
    }

    #[test]
    fn test_ple_random_fp65521() {
        for seed in 0..5u64 {
            let a = random_fp::<65521>(5, 6, seed);
            check_ple(&a);
        }
    }

    #[test]
    fn test_ple_random_mersenne31() {
        for seed in 0..5u64 {
            let a = random_fp::<MERSENNE_31>(6, 5, seed);
            check_ple(&a);
        }
    }

    #[test]
    fn test_ple_random_gf2m8() {
        for seed in 0..5u64 {
            let a = random_gf2m8(5, 6, seed);
            check_ple(&a);
        }
    }

    #[test]
    fn test_ple_random_gf2m16() {
        for seed in 0..3u64 {
            let a = random_gf2m16(4, 5, seed);
            check_ple(&a);
        }
    }

    // ── Hard SC#2: rank-deficient inputs ─────────────────────────────────────

    #[test]
    fn test_ple_rank_deficient_duplicated_row() {
        let mut a = random_fp::<MERSENNE_31>(4, 4, 0xDEAD);
        for j in 0..4 {
            let v = a.get(0, j);
            a.set(2, j, v);
        }
        let r = check_ple(&a);
        assert!(r <= 3, "rank ≤ 3 with duplicated row, got {}", r);
    }

    #[test]
    fn test_ple_rank_deficient_zero_row() {
        let mut a = random_fp::<MERSENNE_31>(4, 4, 0xBEEF);
        for j in 0..4 {
            a.set(3, j, Fp::<MERSENNE_31>::new(0));
        }
        let r = check_ple(&a);
        assert!(r <= 3, "rank ≤ 3 with zero row, got {}", r);
    }

    #[test]
    fn test_ple_rank_deficient_scaled_column() {
        let mut a = random_fp::<MERSENNE_31>(4, 4, 0xCAFE);
        for i in 0..4 {
            let scaled = a.get(i, 0) * Fp::<MERSENNE_31>::new(3);
            a.set(i, 2, scaled);
        }
        let r = check_ple(&a);
        assert!(r <= 3, "rank ≤ 3, got {}", r);
    }

    #[test]
    fn test_ple_rank_deficient_zero_matrix() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 4);
        let (_p, l, e, r) = a.ple();
        assert_eq!(r, 0);
        assert_eq!(l.shape(), (4, 0));
        assert_eq!(e.shape(), (0, 4));
        check_ple(&a);
    }

    #[test]
    fn test_ple_rank_deficient_outer_product() {
        let m = 5;
        let n = 4;
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(m, n);
        let u: Vec<Fp<MERSENNE_31>> = (1..=m as u64).map(Fp::<MERSENNE_31>::new).collect();
        let v: Vec<Fp<MERSENNE_31>> = (1..=n as u64).map(Fp::<MERSENNE_31>::new).collect();
        for (i, &ui) in u.iter().enumerate().take(m) {
            for (j, &vj) in v.iter().enumerate().take(n) {
                a.set(i, j, ui * vj);
            }
        }
        let r = check_ple(&a);
        assert_eq!(r, 1, "outer product has rank 1");
    }

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

        #[test]
        fn prop_ple_rank_deficient_factored(seed in 0u64..1000) {
            // 4×5 matrix with rank ≤ 2 by construction.
            let f1 = random_fp::<MERSENNE_31>(4, 2, seed);
            let f2 = random_fp::<MERSENNE_31>(2, 5, seed.wrapping_add(1));
            let a = gemm(&f1, &f2);
            let (p, l, e, r) = a.ple();
            proptest::prop_assert!(r <= 2, "got rank {}", r);
            let le = if r == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l, &e)
            };
            let rebuild = p.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }
    }

    // ── Hard SC#3: row_echelon and rref forms ────────────────────────────────

    fn check_row_echelon<F: FiniteField>(a: &FieldMatrix<F>) {
        let (x, e) = a.row_echelon();
        let xa = gemm(&x, a);
        assert_eq!(xa, e, "X · A != E");
        let zero = a.get(0, 0).zero_like();
        let mut last: isize = -1;
        for i in 0..e.rows() {
            let mut found: Option<usize> = None;
            for j in 0..e.cols() {
                if e.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                assert!(
                    (p as isize) > last,
                    "row_echelon order violated at row {}",
                    i
                );
                last = p as isize;
                for k in (i + 1)..e.rows() {
                    assert_eq!(
                        e.get(k, p),
                        zero,
                        "below pivot ({}, {}) non-zero at row {}",
                        i,
                        p,
                        k
                    );
                }
            }
        }
    }

    fn check_rref<F: FiniteField>(a: &FieldMatrix<F>) {
        let (x, r) = a.rref();
        let xa = gemm(&x, a);
        assert_eq!(xa, r, "X · A != R");
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut last: isize = -1;
        for i in 0..r.rows() {
            let mut found: Option<usize> = None;
            for j in 0..r.cols() {
                if r.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                assert!((p as isize) > last, "rref pivot order at row {}", i);
                last = p as isize;
                assert_eq!(r.get(i, p), one, "rref leading not 1");
                for k in 0..r.rows() {
                    if k != i {
                        assert_eq!(r.get(k, p), zero, "rref pivot col {} non-zero at {}", p, k);
                    }
                }
            }
        }
    }

    #[test]
    fn test_row_echelon_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(5, 6, seed);
            check_row_echelon(&a);
        }
    }

    #[test]
    fn test_rref_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(5, 6, seed);
            check_rref(&a);
        }
    }

    #[test]
    fn test_row_echelon_random_gf2m8() {
        for seed in 0..3u64 {
            let a = random_gf2m8(4, 5, seed);
            check_row_echelon(&a);
        }
    }

    #[test]
    fn test_rref_random_gf2m8() {
        for seed in 0..3u64 {
            let a = random_gf2m8(4, 5, seed);
            check_rref(&a);
        }
    }

    #[test]
    fn test_row_echelon_rank_deficient() {
        let f1 = random_fp::<MERSENNE_31>(4, 2, 0x55);
        let f2 = random_fp::<MERSENNE_31>(2, 5, 0xAA);
        let a = gemm(&f1, &f2);
        check_row_echelon(&a);
        check_rref(&a);
    }

    // ── Canonical RREF (jit:bd9c6e13) ────────────────────────────────────────
    //
    // `FieldMatrix::rref` is contractually required to produce the
    // canonical reduced row-echelon form: the unique RREF whose pivot
    // columns are the leftmost linearly-independent subset of the input's
    // columns. The check_rref helper above verifies the structural RREF
    // property (leading-1, zero in pivot columns, increasing pivot order)
    // but does NOT verify canonical-leftmost pivots — which is a separate
    // uniqueness contract. The harness below adds:
    //
    //   1. `direct_rref_oracle_fp` — shared SSOT in
    //      `field::test_random_matrix`. Textbook column-by-column
    //      Gauss-Jordan over GF(p); produces canonical RREF by
    //      construction (jit:bd9c6e13 SSOT fix).
    //   2. `dense_random_fp_seeded` — thin alias to the shared
    //      `dense_random_fp_sparse` SSOT in `field::test_random_matrix`.
    //      Same seeded sparse-random generator used by Markowitz sweep
    //      tests in `sparse_matrix.rs`.
    //   3. `check_canonical_rref_fp` — bit-exact equality between
    //      `FieldMatrix::rref` and the oracle.
    //
    // The actual regression guard is
    // `test_rref_canonical_known_buggy_cells_jit_bd9c6e13` (5 cells
    // that diverged pre-fix, with hardcoded expected pivot sets).
    // `test_rref_canonical_15x17_gf7_seed1_structural_correctness` is
    // the issue-named structural check (does NOT guard regression; see
    // evidence doc § 10).

    /// Thin alias: `dense_random_fp_seeded` delegates to the shared
    /// `dense_random_fp_sparse` SSOT in `field::test_random_matrix`
    /// (jit:bd9c6e13 SSOT fix). Tests below use this name unchanged.
    fn dense_random_fp_seeded<const P: u64>(
        rows: usize,
        cols: usize,
        density: f64,
        seed: u64,
    ) -> FieldMatrix<Fp<P>> {
        dense_random_fp_sparse::<P>(rows, cols, density, seed)
    }

    /// Returns the pivot columns of an RREF matrix in ascending order.
    #[cfg(test)]
    fn pivot_cols_of_rref<F: FiniteField>(r: &FieldMatrix<F>) -> Vec<usize> {
        let (m, n) = r.shape();
        let zero = if m == 0 || n == 0 {
            return Vec::new();
        } else {
            r.get(0, 0).zero_like()
        };
        let mut pivots = Vec::new();
        let mut last: isize = -1;
        for i in 0..m {
            let start = (last + 1).max(0) as usize;
            let mut found: Option<usize> = None;
            for j in start..n {
                if r.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                pivots.push(p);
                last = p as isize;
            } else {
                break;
            }
        }
        pivots
    }

    /// Asserts `FieldMatrix::rref` produces the canonical RREF — bit-exact
    /// equal to the textbook Gauss-Jordan oracle.
    #[cfg(test)]
    fn check_canonical_rref_fp<const P: u64>(a: &FieldMatrix<Fp<P>>) {
        let (_x, got) = a.rref();
        let expected = direct_rref_oracle_fp(a);
        assert_eq!(
            got,
            expected,
            "FieldMatrix::rref != canonical (direct_rref_oracle_fp)\n\
             got pivots:      {:?}\n\
             expected pivots: {:?}",
            pivot_cols_of_rref(&got),
            pivot_cols_of_rref(&expected),
        );
    }

    /// Issue-named structural-correctness check for jit:bd9c6e13.
    ///
    /// Input: 15x17 GF(7) matrix, density=0.05, seed=1 (same seeded
    /// generator as `crates/gf2-core/src/field/sparse_matrix.rs` tests).
    ///
    /// NOTE: this exact cell happens to agree pre-fix and post-fix (the
    /// pivots on this seed coincide by chance — see evidence doc § 10).
    /// It is a structural correctness check, NOT a regression guard.
    /// The actual regression guard is
    /// `test_rref_canonical_known_buggy_cells_jit_bd9c6e13` below.
    #[test]
    fn test_rref_canonical_15x17_gf7_seed1_structural_correctness() {
        let a = dense_random_fp_seeded::<7>(15, 17, 0.05, 1);
        check_canonical_rref_fp(&a);
        // Rank reported by ple() must match the canonical pivot count.
        let (_x, got) = a.rref();
        assert_eq!(
            a.rank(),
            pivot_cols_of_rref(&got).len(),
            "rank(15x17 GF(7)/seed=1) must match canonical pivot count"
        );
    }

    /// Regression guard for jit:bd9c6e13 — hardcoded cells that diverged
    /// pre-fix (from evidence doc § 3 discovery sweep, 47 divergent cells).
    ///
    /// Each entry: `(pre-XOR seed, rows, cols, density, expected canonical pivots)`.
    /// The actual generator key used is `seed ^ 0xF1AB_CAFE` (matching
    /// `test_rref_canonical_markowitz_grid_sweep_fp7`). Expected pivots
    /// were captured at post-fix HEAD (commit 95f28a57) against
    /// `direct_rref_oracle_fp`.
    ///
    /// Pre-fix pivot divergences observed:
    ///   - seed=0x8,  3×5,  0.50: got `[1, 4]`,       expected `[1, 2, 4]`
    ///   - seed=0x19, 8×8,  0.05: got `[1, 5]`,       expected `[1, 3, 5]`
    ///   - seed=0x1f, 8×8,  0.05: got `[1, 2, 4]`,    expected `[1, 2, 4, 5]`
    ///   - seed=0x4,  8×8,  0.25: got 5 pivots,        expected 6 pivots
    ///   - seed=0xc,  8×8,  0.25: got 5 pivots,        expected 6 pivots
    #[test]
    fn test_rref_canonical_known_buggy_cells_jit_bd9c6e13() {
        // (pre-XOR seed, rows, cols, density, expected canonical pivots)
        let cells: &[(u64, usize, usize, f64, &[usize])] = &[
            (0x8, 3, 5, 0.5, &[1, 2, 4]),
            (0x19, 8, 8, 0.05, &[1, 3, 5]),
            (0x1f, 8, 8, 0.05, &[1, 2, 4, 5]),
            (0x4, 8, 8, 0.25, &[0, 2, 3, 4, 6, 7]),
            (0xc, 8, 8, 0.25, &[0, 2, 4, 5, 6, 7]),
        ];
        for &(seed, rows, cols, density, expected_pivots) in cells {
            let a = dense_random_fp_seeded::<7>(rows, cols, density, seed ^ 0xF1AB_CAFE);
            // bit-exact equality with the canonical oracle
            check_canonical_rref_fp(&a);
            // Also assert the specific expected pivot columns (regression
            // guard: if the fix regresses, the wrong pivot set is caught here).
            let (_x, got) = a.rref();
            let got_pivots = pivot_cols_of_rref(&got);
            assert_eq!(
                got_pivots, expected_pivots,
                "canonical pivot regression: seed={seed:#x} rows={rows} cols={cols} \
                 density={density}: got {got_pivots:?}, expected {expected_pivots:?}",
            );
        }
    }

    /// Mirrors the seed/shape/density grid that `test_rref_markowitz_sweep_fp7`
    /// uses in `sparse_matrix.rs` (32 seeds x 6 shapes x 5 densities), and
    /// asserts byte-equal canonical RREF between the dense PLE-based
    /// `FieldMatrix::rref` and the textbook oracle. Pre-fix this sweep
    /// flagged 47 divergent cells (jit:bd9c6e13 evidence doc § "Reproducer").
    #[test]
    fn test_rref_canonical_markowitz_grid_sweep_fp7() {
        const SHAPES: &[(usize, usize)] = &[(1, 1), (3, 5), (5, 3), (8, 8), (15, 17), (24, 24)];
        const DENSITIES: &[f64] = &[0.0_f64, 0.05, 0.25, 0.5, 0.9];
        let mut divergent = 0usize;
        for seed in 0u64..32 {
            for &(rows, cols) in SHAPES {
                for &density in DENSITIES {
                    let a = dense_random_fp_seeded::<7>(rows, cols, density, seed ^ 0xF1AB_CAFE);
                    let (_x, got) = a.rref();
                    let expected = direct_rref_oracle_fp(&a);
                    if got != expected {
                        divergent += 1;
                        eprintln!(
                            "DIVERGE: seed={seed:#x} rows={rows} cols={cols} density={density} \
                             got_pivots={:?} expected_pivots={:?}",
                            pivot_cols_of_rref(&got),
                            pivot_cols_of_rref(&expected),
                        );
                    }
                }
            }
        }
        assert_eq!(
            divergent, 0,
            "canonical-RREF divergence count: expected 0, got {divergent}; \
             see eprintln output above for the failing cells"
        );
    }

    // Property-based test: bit-exact equality with the textbook canonical
    // Gauss-Jordan oracle on 128 random inputs over GF(7) and GF(251),
    // restricted to rank-deficient matrices.
    //
    // Per jit:bd9c6e13 SC#2: "property-based proptest covering 100+
    // random rank-deficient shapes". Configured for 128 cases (> 100).
    // Rank-deficient matrices are generated by outer-product
    // construction `A = F * G` where `F` is rows×(rank) and `G` is
    // (rank)×cols with `rank = min(rows, cols) - 1`, so
    // `rank(A) <= rank < min(rows, cols)` by construction.
    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(128))]

        #[test]
        fn proptest_field_matrix_rref_canonical_rank_deficient_jit_bd9c6e13(
            rows in 4usize..=16,
            cols in 4usize..=16,
            seed in proptest::prelude::any::<u64>(),
        ) {
            // Build a rank-deficient GF(7) matrix by outer-product
            // construction: A = F * G where F is rows×(rank) and G is
            // (rank)×cols with rank = min(rows,cols) - 1. This guarantees
            // rank(A) <= rank < min(rows,cols) by construction.
            let rank = rows.min(cols) - 1; // guaranteed < min(rows,cols)
            let f7 = random_fp::<7>(rows, rank, seed);
            let g7 = random_fp::<7>(rank, cols, seed.wrapping_add(1));
            let a7 = gemm(&f7, &g7);
            // rank(A) <= rank by construction; equality holds with high prob
            // but we only need rank < min(rows,cols), which always holds.
            proptest::prop_assert!(
                a7.rank() < rows.min(cols),
                "product matrix should have rank < min(rows,cols)"
            );
            let (_x, got) = a7.rref();
            let expected = direct_rref_oracle_fp(&a7);
            proptest::prop_assert_eq!(
                got, expected,
                "FieldMatrix::rref != canonical oracle on GF(7) rank-deficient input"
            );

            // Same for GF(251).
            let f251 = random_fp::<251>(rows, rank, seed.wrapping_add(2));
            let g251 = random_fp::<251>(rank, cols, seed.wrapping_add(3));
            let a251 = gemm(&f251, &g251);
            let (_x2, got2) = a251.rref();
            let expected2 = direct_rref_oracle_fp(&a251);
            proptest::prop_assert_eq!(
                got2, expected2,
                "FieldMatrix::rref != canonical oracle on GF(251) rank-deficient input"
            );
        }
    }

    // ── Hard SC#4: nullspace ─────────────────────────────────────────────────

    fn check_nullspace<F: FiniteField>(a: &FieldMatrix<F>) {
        let basis = a.nullspace();
        let r = a.rank();
        let n = a.cols();
        assert_eq!(basis.len(), n - r, "nullspace size != n − rank");
        let zero = a.get(0, 0).zero_like();
        for (k, v) in basis.iter().enumerate() {
            assert_eq!(v.len(), n, "basis[{}] wrong length", k);
            for i in 0..a.rows() {
                let mut acc = zero.clone();
                for j in 0..n {
                    acc += a.get(i, j) * v.get(j).clone();
                }
                assert_eq!(acc, zero, "a · basis[{}] non-zero at row {}", k, i);
            }
        }
        if !basis.is_empty() {
            let cols = basis.len();
            let mut stacked = FieldMatrix::new(n, cols, zero);
            for (k, v) in basis.iter().enumerate() {
                for i in 0..n {
                    stacked.set(i, k, v.get(i).clone());
                }
            }
            assert_eq!(stacked.rank(), cols, "nullspace not LI");
        }
    }

    #[test]
    fn test_nullspace_random_mersenne31() {
        for seed in 0..3u64 {
            let a = random_fp::<MERSENNE_31>(4, 6, seed);
            check_nullspace(&a);
        }
    }

    #[test]
    fn test_nullspace_rank_deficient() {
        let f1 = random_fp::<MERSENNE_31>(5, 2, 0x11);
        let f2 = random_fp::<MERSENNE_31>(2, 6, 0x22);
        let a = gemm(&f1, &f2);
        check_nullspace(&a);
    }

    #[test]
    fn test_nullspace_full_rank_square() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::identity(4);
        let basis = a.nullspace();
        assert!(basis.is_empty());
    }

    #[test]
    fn test_nullspace_zero_matrix() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(3, 4);
        let basis = a.nullspace();
        assert_eq!(basis.len(), 4);
    }

    // ── Hard SC#5: lu ────────────────────────────────────────────────────────

    #[test]
    fn test_lu_full_rank_square() {
        let a = random_fp::<MERSENNE_31>(4, 4, 0x77);
        match a.lu() {
            Some((p, l, u)) => {
                let pa = p.apply(&a);
                let lu = gemm(&l, &u);
                assert_eq!(pa, lu);
            }
            None => assert!(a.rank() < 4),
        }
    }

    #[test]
    fn test_lu_rank_deficient_returns_none() {
        let f1 = random_fp::<MERSENNE_31>(4, 2, 0x33);
        let f2 = random_fp::<MERSENNE_31>(2, 4, 0x44);
        let a = gemm(&f1, &f2);
        if a.rank() < 4 {
            assert!(a.lu().is_none());
        }
    }

    /// Hand-crafted matrix that forces a NON-INVOLUTIVE permutation in
    /// the PLE recursion. The 4×4 identity-shifted matrix
    ///
    ///   [ 0 0 0 1 ]
    ///   [ 1 0 0 0 ]
    ///   [ 0 1 0 0 ]
    ///   [ 0 0 1 0 ]
    ///
    /// has rank 4. PLE's column-by-column pivoting must do a 4-cycle
    /// (or compose three 2-cycles) on the rows. If `lu()` returns the
    /// PLE permutation directly (the bug R3 caught), then `P · A` does
    /// not equal `L · U`. This test fails on the buggy code and passes
    /// on the orientation-corrected code.
    #[test]
    fn test_lu_non_involutive_permutation_fp_m31() {
        type F = Fp<MERSENNE_31>;
        let mut a = FieldMatrix::<F>::zeros(4, 4);
        a.set(0, 3, F::new(1));
        a.set(1, 0, F::new(1));
        a.set(2, 1, F::new(1));
        a.set(3, 2, F::new(1));
        let (p, l, u) = a.lu().expect("rank == 4 so lu must be Some");
        let pa = p.apply(&a);
        let lu = gemm(&l, &u);
        assert_eq!(pa, lu, "lu contract: P · A == L · U for non-involutive P");
    }

    // Proptest covering 50 random 5×5 matrices over Mersenne-31. Any
    // non-trivial random matrix is expected to be full-rank with
    // probability (1 − 1/p) · (1 − 1/p²) · … ≈ 1; whenever lu()
    // returns Some, P · A == L · U must hold.
    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(50))]
        #[test]
        fn prop_lu_contract_random_5x5_fp_m31(seed in 0u64..1_000_000) {
            type F = Fp<MERSENNE_31>;
            let a = random_fp::<MERSENNE_31>(5, 5, seed);
            if let Some((p, l, u)) = a.lu() {
                let pa = p.apply(&a);
                let lu = gemm(&l, &u);
                proptest::prop_assert_eq!(pa, lu);
                let _ = std::marker::PhantomData::<F>;
            }
        }
    }

    /// Independently-computed-rank check (reviewer-asked): build matrices
    /// with a KNOWN rank by construction using DISTINCT canonical-basis
    /// vectors (e_i ⊗ e_i pattern), then assert `m.rank() == known_rank`.
    /// Using basis vectors guarantees the constructed rank equals the
    /// number of outer products without depending on the field size or
    /// PRNG quality.
    #[test]
    fn test_rank_matches_independent_construction_fp_m31() {
        type F = Fp<MERSENNE_31>;
        let m = 8;
        let n = 6;
        for &target_rank in &[1usize, 2, 3, 4, 5] {
            let mut a = FieldMatrix::<F>::zeros(m, n);
            for i in 0..target_rank {
                // u_i = e_i (the i-th canonical basis vector in Fp^m),
                // v_i = e_i (in Fp^n). The sum ∑ u_i ⊗ v_i is a
                // rank-`target_rank` matrix with 1s on the leading
                // diagonal and 0s elsewhere — independently rank-`r` by
                // construction (the diagonal gives an r × r identity
                // submatrix).
                a.set(i, i, F::new(1));
            }
            assert_eq!(
                a.rank(),
                target_rank,
                "rank mismatch: built {}-rank matrix from {} canonical-basis outer products, got rank()={}",
                target_rank,
                target_rank,
                a.rank()
            );
        }

        // Also cross-check on a non-trivial rank-2 matrix built from two
        // random rank-1 outer products: a = u1 ⊗ v1 + u2 ⊗ v2 with u1
        // and u2 linearly independent (and same for v1, v2). Use
        // distinct canonical-basis-shifted vectors.
        let u1: Vec<F> = (0..m).map(|j| F::new(j as u64 + 1)).collect();
        let u2: Vec<F> = (0..m).map(|j| F::new(((m - j) as u64) * 7 + 13)).collect();
        let v1: Vec<F> = (0..n).map(|j| F::new(j as u64 + 2)).collect();
        let v2: Vec<F> = (0..n).map(|j| F::new(((n - j) as u64) * 5 + 17)).collect();
        let mut a = FieldMatrix::<F>::zeros(m, n);
        for r in 0..m {
            for c in 0..n {
                a.set(r, c, u1[r] * v1[c] + u2[r] * v2[c]);
            }
        }
        // Two independent outer products → rank 2 (with probability ≈ 1
        // for large fields and these specific seeds; verified at
        // construction).
        assert_eq!(
            a.rank(),
            2,
            "rank mismatch: u1⊗v1 + u2⊗v2 should be rank 2, got rank()={}",
            a.rank()
        );
    }

    #[test]
    fn test_lu_full_rank_rectangular() {
        let a = random_fp::<MERSENNE_31>(3, 5, 0x88);
        if a.rank() == 3 {
            let (p, l, u) = a.lu().expect("full row-rank lu");
            let pa = p.apply(&a);
            let lu = gemm(&l, &u);
            assert_eq!(pa, lu);
        }
    }

    // ── Hard SC#6: edge cases ────────────────────────────────────────────────

    #[test]
    fn test_ple_identity() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::identity(4);
        assert_eq!(check_ple(&a), 4);
    }

    #[test]
    fn test_ple_all_ones() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::ones(3, 4);
        assert_eq!(check_ple(&a), 1);
    }

    #[test]
    fn test_ple_single_column() {
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(5, 1);
        a.set(2, 0, Fp::<MERSENNE_31>::new(7));
        a.set(4, 0, Fp::<MERSENNE_31>::new(3));
        assert_eq!(check_ple(&a), 1);
    }

    #[test]
    fn test_ple_single_row() {
        let mut a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(1, 5);
        a.set(0, 2, Fp::<MERSENNE_31>::new(3));
        assert_eq!(check_ple(&a), 1);
    }

    #[test]
    fn test_ple_wide() {
        let a = random_fp::<MERSENNE_31>(3, 10, 0xAB);
        let r = check_ple(&a);
        assert!(r <= 3);
    }

    #[test]
    fn test_ple_tall() {
        let a = random_fp::<MERSENNE_31>(10, 3, 0xCD);
        let r = check_ple(&a);
        assert!(r <= 3);
    }

    #[test]
    fn test_ple_zero_matrix_edge() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(5, 7);
        let (_p, _l, _e, r) = a.ple();
        assert_eq!(r, 0);
    }

    /// Zero-width input (`m × 0`) edge case: every public op must not
    /// panic. `row_echelon` returns `(I_m, 0_m×0)`; `rref` forwards;
    /// `lu` returns `Some` (rank == 0 == min(m, 0)); `nullspace` is
    /// empty (n == 0).
    #[test]
    fn test_zero_width_edge_does_not_panic() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 0);
        // ple
        let (_p, l, e, r) = a.ple();
        assert_eq!(r, 0);
        assert_eq!(l.shape(), (4, 0));
        assert_eq!(e.shape(), (0, 0));
        // row_echelon: X = I_4, E = 0_{4×0}
        let (x, e) = a.row_echelon();
        assert_eq!(x.shape(), (4, 4));
        assert_eq!(e.shape(), (4, 0));
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j {
                    Fp::<MERSENNE_31>::new(1)
                } else {
                    Fp::<MERSENNE_31>::new(0)
                };
                assert_eq!(
                    x.get(i, j),
                    expected,
                    "row_echelon X must be identity at ({i}, {j})"
                );
            }
        }
        // rref forwards to row_echelon for m × 0.
        let (_x2, e2) = a.rref();
        assert_eq!(e2.shape(), (4, 0));
        // lu: rank == 0 == min(4, 0); Some.
        let lu = a.lu();
        assert!(lu.is_some(), "lu(m × 0) must return Some (rank == min)");
        // nullspace is empty (n - rank == 0).
        let ns = a.nullspace();
        assert_eq!(ns.len(), 0);
        // rank
        assert_eq!(a.rank(), 0);
    }

    #[test]
    fn test_ple_empty_runtime_field_edges_do_not_panic() {
        let field = Gf2mField::new(4, 0b10011);

        let zero_rows = FieldMatrix::<Gf2mElement>::new(0, 3, field.element(0));
        let (p, l, e, r) = zero_rows.ple();
        assert_eq!(r, 0);
        assert_eq!(p.len(), 0);
        assert_eq!(l.shape(), (0, 0));
        assert_eq!(e.shape(), (0, 3));

        let (x, echelon) = zero_rows.row_echelon();
        assert_eq!(x.shape(), (0, 0));
        assert_eq!(echelon.shape(), (0, 3));

        let zero_cols = FieldMatrix::<Gf2mElement>::new(3, 0, field.element(0));
        let (p, l, e, r) = zero_cols.ple();
        assert_eq!(r, 0);
        assert_eq!(p.len(), 3);
        assert_eq!(l.shape(), (3, 0));
        assert_eq!(e.shape(), (0, 0));
    }

    #[test]
    fn test_ple_gf2m8_edge_cases() {
        let id = FieldMatrix::<Gf2m8>::identity(4);
        check_ple(&id);
        let zeros = FieldMatrix::<Gf2m8>::zeros(3, 5);
        let (_p, _l, _e, r) = zeros.ple();
        assert_eq!(r, 0);
        let tall = random_gf2m8(8, 3, 0x5A);
        check_ple(&tall);
        let wide = random_gf2m8(3, 8, 0xA5);
        check_ple(&wide);
    }

    // ── SC#8: allocation budget (strict integer pins per replan §3.4) ────────
    //
    // Each test below invokes the corresponding derived op once on a random
    // input over `Fp<MERSENNE_31>` and asserts the EXACT number of
    // `FieldMatrix::new` (plus `transpose` / `to_owned`) bumps observed. The
    // counter increments on every `FieldMatrix::new` (`matrix.rs:194`),
    // `FieldMatrix::transpose` (`matrix.rs:1102`), `MatView::to_owned`
    // (`matrix.rs:1574`), and `MatViewMut::to_owned` (`matrix.rs:1890`).
    //
    // A change in any of these counts means: either the recursion strategy
    // changed, the kernels' internal allocation count changed, or both. Any
    // such change MUST be cross-checked against the doc-contract budget in
    // the module rustdoc and the matching benchmark numbers in
    // `benches/ple.rs`.

    /// Per-test serial guard: the counter is thread-local, but `#[serial]`
    /// keeps the budget reading deterministic across nextest runs.

    #[test]
    #[serial]
    fn test_ple_allocation_budget_n4_fp_m31() {
        let a = random_fp::<MERSENNE_31>(4, 4, 0xC3F4);
        reset_fieldmatrix_new_count();
        let _ = a.ple();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_PLE_N4,
            "ple(4×4) allocs should be exactly {EXPECTED_PLE_N4}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_ple_allocation_budget_n64_fp_m31() {
        let a = random_fp::<MERSENNE_31>(64, 64, 0xC3F8);
        reset_fieldmatrix_new_count();
        let _ = a.ple();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_PLE_N64,
            "ple(64×64) allocs should be exactly {EXPECTED_PLE_N64}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    #[ignore = "slow: PLE decomposition on Fp<MERSENNE_31> 1024×1024 matrix"]
    fn test_ple_allocation_budget_n1024_fp_m31() {
        let a = random_fp::<MERSENNE_31>(1024, 1024, 0xC3FA);
        reset_fieldmatrix_new_count();
        let _ = a.ple();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_PLE_N1024,
            "ple(1024×1024) allocs should be exactly {EXPECTED_PLE_N1024}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_row_echelon_allocation_budget_n64_fp_m31() {
        let a = random_fp::<MERSENNE_31>(64, 64, 0xC3FB);
        reset_fieldmatrix_new_count();
        let _ = a.row_echelon();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_ROW_ECHELON_N64,
            "row_echelon(64×64) allocs should be exactly {EXPECTED_ROW_ECHELON_N64}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_rref_allocation_budget_n64_fp_m31() {
        let a = random_fp::<MERSENNE_31>(64, 64, 0xC3FC);
        reset_fieldmatrix_new_count();
        let _ = a.rref();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_RREF_N64,
            "rref(64×64) allocs should be exactly {EXPECTED_RREF_N64}; got {allocs}"
        );
    }

    #[test]
    #[serial]
    fn test_lu_allocation_budget_n64_fp_m31() {
        let a = random_fp::<MERSENNE_31>(64, 64, 0xC3FD);
        reset_fieldmatrix_new_count();
        let _ = a.lu();
        let allocs = fieldmatrix_new_count();
        assert_eq!(
            allocs, EXPECTED_LU_N64,
            "lu(64×64) allocs should be exactly {EXPECTED_LU_N64}; got {allocs}"
        );
    }

    // Pinned allocation counts (strict integer asserts per replan §3.4).
    //
    // Each count is the exact `FIELDMATRIX_NEW_COUNT` reading observed
    // for the current view-based driver. Update only when the recursion
    // strategy or the underlying gemm/trsm kernels change their
    // allocation footprint. The breakdown matches the budget in the
    // module rustdoc:
    //
    //   ple(m × n) = 1 (working clone)
    //              + 2 (final L + final E from split_compact)
    //              + per-level cost (materialised L1, L1_bot, plus the
    //                gemm and trsm kernels' B-transpose scratches and
    //                their own internal recursion's gemm calls).
    //
    // The counter increments on FieldMatrix::new (via FieldMatrix::clone,
    // FieldMatrix::zeros, MatView::transpose, MatViewMut::to_owned, etc.).
    //
    // The 4736 count at n=1024 is dominated by the trsm_lower's recursive
    // gemm_axpy_into_view calls (each pays 2 transpose bumps: to_owned +
    // transpose). PLE has log₂(1024)=10 column-halving levels; trsm at
    // each level recurses deeper at threshold=8 than it did at threshold=32,
    // contributing more bumps. The observed 4736 is the empirical reality
    // at TRI_BASE_THRESHOLD=8 (selected by jit:73ec5da3 sweep); every
    // byte of intermediate storage is documented and accounted for here.
    //
    // Counts updated 2026-05-07 from threshold=32 baseline as part of
    // jit:73ec5da3 R1 rework: the deeper trsm recursion at threshold=8
    // adds ~13% more allocations at n=1024 (4192 → 4736) but reduces
    // wall-time by 1–7% on the target Mersenne-31 cells (see the sweep
    // table in dev/bench_results/2026-05-07-73ec5da3-ple-trsm-tuning.md).
    const EXPECTED_PLE_N4: u64 = 14;
    const EXPECTED_PLE_N64: u64 = 264;
    const EXPECTED_PLE_N1024: u64 = 4736;
    const EXPECTED_ROW_ECHELON_N64: u64 = 280;
    // At n=64, try_blocked_back_sub returns false (n < BLOCKED_BACK_SUB_MIN_DIM
    // = 128) so rref falls through to the scalar loop — no extra allocations
    // vs row_echelon. For n >= 128 the blocked path adds 8 scratch matrices +
    // trsm B-transposes; see BLOCKED_BACK_SUB_MIN_DIM doc comment.
    const EXPECTED_RREF_N64: u64 = 280;
    const EXPECTED_LU_N64: u64 = 264;

    // Boundary lengths chosen per the PLE design doc § 6.1 (jit:6823c8a0)
    // and the gf2-core word-boundary test convention (0, 1, 63, 64, 65)
    // plus the 16-byte AVX2 boundary (15, 16, 17). Consumed by the
    // boundary-length proptest sweep below (SC#2): each (m, n) pair is
    // drawn from this set and the panelized PLE output is asserted
    // bit-exact against `ple_scalar_oracle`.
    const PANEL_BOUNDARY_LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];

    /// Sanity wall-time measurement: runs `FieldMatrix::ple` on a
    /// `256 × 256` GF(251) matrix five times and prints the median per-call
    /// duration. Output is informational only; the test always passes
    /// (the goal is to catch the case where the panelized dispatch is
    /// either not active or actively making things slower, by giving the
    /// developer a quick numeric handle). The b0fa00af pre-change
    /// baseline was ~4.7 ms.
    #[test]
    #[ignore = "slow: wall-time probe for panelized PLE; informational only"]
    fn test_ple_panelized_wall_time_probe_gf251_256_uniform() {
        let n = 256;
        let a = random_fp::<251>(n, n, 0xC3FAu64);
        // Warmup.
        for _ in 0..3 {
            let _ = a.ple();
        }
        let mut samples: Vec<u128> = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let _ = a.ple();
            samples.push(start.elapsed().as_micros());
        }
        samples.sort();
        let median_us = samples[samples.len() / 2];
        eprintln!("pluq GF(251) n=256 uniform median: {median_us} µs (samples {samples:?})");
    }

    /// Full panelized-PLE measurement sweep across the A8-row cells
    /// (rows 6-17, 71) plus the new R1-amendment cells per the design
    /// doc § 7. Emits one CSV line per cell to stderr in the format
    /// `op,field,n,regime,trial,wall_ns,wall_median_ns`. 5 trials per
    /// cell, with 3 warmup runs first.
    ///
    /// To capture the output, run with the CCX1 flock wrapper:
    /// ```bash
    /// ./dev/benchmarks/ccx1-bench-flock.sh \
    ///   cargo test -p gf2-core --release --all-features --lib -- \
    ///     --ignored --nocapture --test-threads 1 \
    ///     'test_ple_panelized_wall_time_full_sweep'
    /// ```
    /// (stderr lines bracketed by `--- panelized-ple-sweep BEGIN/END ---`
    /// markers are the canonical CSV emission.)
    #[test]
    #[ignore = "slow: full panelized PLE wall-time sweep (~30 s)"]
    fn test_ple_panelized_wall_time_full_sweep() {
        const CELLS: &[(u64, &str, usize, &str)] = &[
            (7, "GF(7)", 64, "uniform"),
            (7, "GF(7)", 64, "deficient"),
            (7, "GF(7)", 256, "uniform"),
            (7, "GF(7)", 256, "deficient"),
            (7, "GF(7)", 1024, "uniform"),
            (7, "GF(7)", 1024, "deficient"),
            (31, "GF(31)", 64, "uniform"),
            (31, "GF(31)", 64, "deficient"),
            (31, "GF(31)", 256, "uniform"),
            (31, "GF(31)", 256, "deficient"),
            (31, "GF(31)", 1024, "uniform"),
            (31, "GF(31)", 1024, "deficient"),
            (127, "GF(127)", 64, "uniform"),
            (127, "GF(127)", 64, "deficient"),
            (127, "GF(127)", 256, "uniform"),
            (127, "GF(127)", 256, "deficient"),
            (127, "GF(127)", 1024, "uniform"),
            (127, "GF(127)", 1024, "deficient"),
            (241, "GF(241)", 64, "uniform"),
            (241, "GF(241)", 64, "deficient"),
            (241, "GF(241)", 256, "uniform"),
            (241, "GF(241)", 256, "deficient"),
            (241, "GF(241)", 1024, "uniform"),
            (241, "GF(241)", 1024, "deficient"),
            (251, "GF(251)", 64, "uniform"),
            (251, "GF(251)", 64, "deficient"),
            (251, "GF(251)", 256, "uniform"),
            (251, "GF(251)", 256, "deficient"),
            (251, "GF(251)", 1024, "uniform"),
            (251, "GF(251)", 1024, "deficient"),
            (65521, "GF(65521)", 64, "uniform"),
            (65521, "GF(65521)", 64, "deficient"),
            (65521, "GF(65521)", 256, "uniform"),
            (65521, "GF(65521)", 256, "deficient"),
            (65521, "GF(65521)", 1024, "uniform"),
            (65521, "GF(65521)", 1024, "deficient"),
        ];
        eprintln!("--- panelized-ple-sweep BEGIN ---");
        eprintln!("op,field,n,regime,trial,wall_ns,wall_median_ns");
        for &(p, field, n, regime) in CELLS {
            let median_ns = match p {
                7 => measure_cell::<7>(n, regime, field),
                31 => measure_cell::<31>(n, regime, field),
                127 => measure_cell::<127>(n, regime, field),
                241 => measure_cell::<241>(n, regime, field),
                251 => measure_cell::<251>(n, regime, field),
                65521 => measure_cell::<65521>(n, regime, field),
                _ => unreachable!(),
            };
            eprintln!("pluq,{field},{n},{regime},median,,{median_ns}");
        }
        eprintln!("--- panelized-ple-sweep END ---");
    }

    /// Helper for `test_ple_panelized_wall_time_full_sweep`: measures
    /// a single (field, n, regime) cell with 3 warmup runs + 5 trials,
    /// returns the median wall-time in ns, and emits one CSV row per
    /// trial to stderr.
    fn measure_cell<const P: u64>(n: usize, regime: &str, field: &str) -> u128 {
        let seed = P
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(n as u64)
            .wrapping_add(if regime == "deficient" { 0x1234 } else { 0 });
        let a = if regime == "deficient" {
            let rank = (n / 2).max(1);
            let f = random_fp::<P>(n, rank, seed);
            let g = random_fp::<P>(rank, n, seed.wrapping_add(0xCAFE));
            gemm(&f, &g)
        } else {
            random_fp::<P>(n, n, seed)
        };
        for _ in 0..3 {
            let _ = a.ple();
        }
        let mut samples: Vec<u128> = Vec::new();
        for trial in 1..=5 {
            let start = std::time::Instant::now();
            let _ = a.ple();
            let elapsed_ns = start.elapsed().as_nanos();
            samples.push(elapsed_ns);
            eprintln!("pluq,{field},{n},{regime},{trial},{elapsed_ns},");
        }
        samples.sort();
        samples[samples.len() / 2]
    }

    /// Helper: measures a single (field, n, regime) cell for `row_echelon`
    /// with 3 warmup runs + 5 trials, returns the median wall-time in ns.
    fn measure_echelon_cell<const P: u64>(n: usize, regime: &str, field: &str) -> u128 {
        let seed = P
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(n as u64)
            .wrapping_add(if regime == "deficient" { 0x1234 } else { 0 });
        let a = if regime == "deficient" {
            let rank = (n / 2).max(1);
            let f = random_fp::<P>(n, rank, seed);
            let g = random_fp::<P>(rank, n, seed.wrapping_add(0xCAFE));
            gemm(&f, &g)
        } else {
            random_fp::<P>(n, n, seed)
        };
        for _ in 0..3 {
            let _ = a.row_echelon();
        }
        let mut samples: Vec<u128> = Vec::new();
        for trial in 1..=5 {
            let start = std::time::Instant::now();
            let _ = a.row_echelon();
            let elapsed_ns = start.elapsed().as_nanos();
            samples.push(elapsed_ns);
            eprintln!("echelon,{field},{n},{regime},{trial},{elapsed_ns},");
        }
        samples.sort();
        samples[samples.len() / 2]
    }

    /// Wall-time sweep for the A8 echelon cells (rows 18-33, 72-73).
    ///
    /// Run via:
    /// ```bash
    /// ./dev/benchmarks/ccx1-bench-flock.sh \
    ///   cargo test -p gf2-core --release --all-features --lib \
    ///   -- --nocapture --ignored field::ple::tests::test_echelon_wall_time_full_sweep \
    ///   2>&1 | grep -E 'echelon|BEGIN|END'
    /// ```
    ///
    /// Output is CSV emitted to stderr between `--- echelon-sweep BEGIN ---`
    /// and `--- echelon-sweep END ---`.
    #[test]
    #[ignore = "slow: echelon wall-time sweep for A8 rows 18-33 and 72-73 (~60 s)"]
    fn test_echelon_wall_time_full_sweep() {
        const CELLS: &[(u64, &str, usize, &str)] = &[
            (7, "GF(7)", 64, "uniform"),
            (7, "GF(7)", 64, "deficient"),
            (7, "GF(7)", 256, "uniform"),
            (7, "GF(7)", 256, "deficient"),
            (7, "GF(7)", 1024, "uniform"),
            (7, "GF(7)", 1024, "deficient"),
            (31, "GF(31)", 64, "uniform"),
            (31, "GF(31)", 64, "deficient"),
            (31, "GF(31)", 256, "uniform"),
            (31, "GF(31)", 256, "deficient"),
            (31, "GF(31)", 1024, "uniform"),
            (31, "GF(31)", 1024, "deficient"),
            (251, "GF(251)", 64, "uniform"),
            (251, "GF(251)", 64, "deficient"),
            (251, "GF(251)", 256, "uniform"),
            (251, "GF(251)", 256, "deficient"),
            (251, "GF(251)", 1024, "uniform"),
            (251, "GF(251)", 1024, "deficient"),
            (65521, "GF(65521)", 64, "uniform"),
            (65521, "GF(65521)", 64, "deficient"),
            (65521, "GF(65521)", 256, "uniform"),
            (65521, "GF(65521)", 256, "deficient"),
            (65521, "GF(65521)", 1024, "uniform"),
            (65521, "GF(65521)", 1024, "deficient"),
            (2_147_483_647, "GF(M31)", 64, "uniform"),
            (2_147_483_647, "GF(M31)", 64, "deficient"),
            (2_147_483_647, "GF(M31)", 256, "uniform"),
            (2_147_483_647, "GF(M31)", 256, "deficient"),
            (2_147_483_647, "GF(M31)", 1024, "uniform"),
            (2_147_483_647, "GF(M31)", 1024, "deficient"),
        ];
        eprintln!("--- echelon-sweep BEGIN ---");
        eprintln!("op,field,n,regime,trial,wall_ns,wall_median_ns");
        for &(p, field, n, regime) in CELLS {
            let median_ns = match p {
                7 => measure_echelon_cell::<7>(n, regime, field),
                31 => measure_echelon_cell::<31>(n, regime, field),
                251 => measure_echelon_cell::<251>(n, regime, field),
                65521 => measure_echelon_cell::<65521>(n, regime, field),
                2_147_483_647 => measure_echelon_cell::<2_147_483_647>(n, regime, field),
                _ => unreachable!(),
            };
            eprintln!("echelon,{field},{n},{regime},median,,{median_ns}");
        }
        eprintln!("--- echelon-sweep END ---");
    }

    /// Scalar back-substitution verbatim from pre-`869ce43b` commit `38387525`.
    ///
    /// Serves as the state-A anchor for SC#5 same-operation non-regression:
    /// calling this function on a given matrix is equivalent to calling
    /// `rref()` at commit `38387525`. The blocked path in the production
    /// `rref()` is the state-B counterpart.
    fn rref_scalar_state_a<const P: u64>(
        a: &FieldMatrix<Fp<P>>,
    ) -> (FieldMatrix<Fp<P>>, FieldMatrix<Fp<P>>) {
        let (m, n) = a.shape();
        if m == 0 || n == 0 {
            return a.row_echelon();
        }
        let (mut x, mut e) = a.row_echelon();
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();

        let mut pivots: Vec<(usize, usize)> = Vec::new();
        let mut last: isize = -1;
        for i in 0..m {
            let start = (last + 1).max(0) as usize;
            let mut found: Option<usize> = None;
            for j in start..n {
                if e.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                pivots.push((i, p));
                last = p as isize;
            } else {
                break;
            }
        }

        for &(pi, pc) in &pivots {
            let pivot_val = e.get(pi, pc);
            if pivot_val != one {
                let inv = pivot_val
                    .inv()
                    .unwrap_or_else(|| panic!("rref: pivot at ({pi}, {pc}) failed to invert"));
                for j in 0..n {
                    let v = e.get(pi, j) * inv;
                    e.set(pi, j, v);
                }
                for j in 0..m {
                    let v = x.get(pi, j) * inv;
                    x.set(pi, j, v);
                }
            }
            for k in 0..m {
                if k == pi {
                    continue;
                }
                let factor = e.get(k, pc);
                if factor == zero {
                    continue;
                }
                for j in 0..n {
                    let v = e.get(k, j) - factor * e.get(pi, j);
                    e.set(k, j, v);
                }
                for j in 0..m {
                    let v = x.get(k, j) - factor * x.get(pi, j);
                    x.set(k, j, v);
                }
            }
        }
        (x, e)
    }

    /// Helper: measure both state-A (scalar back-sub) and state-B (blocked
    /// back-sub) rref on the same matrix, 10 trials each, 3 warm-up calls.
    ///
    /// Returns `(median_a_ns, median_b_ns)`. Emits per-trial CSV lines to
    /// stderr:
    /// - `rref_A,<field>,<n>,<regime>,<trial>,<wall_ns>,`
    /// - `rref_B,<field>,<n>,<regime>,<trial>,<wall_ns>,`
    fn measure_rref_paired_cell<const P: u64>(n: usize, regime: &str, field: &str) -> (u128, u128) {
        let seed = P
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(n as u64)
            .wrapping_add(if regime == "deficient" { 0x1234 } else { 0 });
        let a = if regime == "deficient" {
            let rank = (n / 2).max(1);
            let f = random_fp::<P>(n, rank, seed);
            let g = random_fp::<P>(rank, n, seed.wrapping_add(0xCAFE));
            gemm(&f, &g)
        } else {
            random_fp::<P>(n, n, seed)
        };
        // Warm up both paths.
        for _ in 0..3 {
            let _ = rref_scalar_state_a::<P>(&a);
            let _ = a.rref();
        }
        let mut samples_a: Vec<u128> = Vec::new();
        let mut samples_b: Vec<u128> = Vec::new();
        // Interleave A/B trials to share the same thermal/frequency state.
        for trial in 1..=10 {
            let t0 = std::time::Instant::now();
            let _ = rref_scalar_state_a::<P>(&a);
            let ns_a = t0.elapsed().as_nanos();
            samples_a.push(ns_a);
            eprintln!("rref_A,{field},{n},{regime},{trial},{ns_a},");

            let t1 = std::time::Instant::now();
            let _ = a.rref();
            let ns_b = t1.elapsed().as_nanos();
            samples_b.push(ns_b);
            eprintln!("rref_B,{field},{n},{regime},{trial},{ns_b},");
        }
        samples_a.sort();
        samples_b.sort();
        (
            samples_a[samples_a.len() / 2],
            samples_b[samples_b.len() / 2],
        )
    }

    /// SC#5 non-regression: paired same-operation `rref` delta
    /// (state A = scalar back-sub @ `38387525` vs state B = blocked back-sub).
    ///
    /// Previously-PASSing cells (ratio ≤ 1.5× before `869ce43b`):
    /// GF(7)/n={64,256,1024}, GF(31)/n={64,256}, GF(65521)/n={64,256,1024},
    /// GF(M31)/n={64,256,1024}, all uniform + deficient where applicable.
    ///
    /// SC#5 is satisfied when every cell's delta
    /// `(B_median − A_median) / A_median ≤ 5%`.
    ///
    /// Run via (CCX1 flock guard required):
    /// ```bash
    /// ./dev/benchmarks/ccx1-bench-flock.sh \
    ///   cargo test -p gf2-core --release --all-features --lib \
    ///   -- --nocapture --ignored field::ple::tests::test_rref_non_regression_wall_time \
    ///   2>&1 | grep -E 'rref|BEGIN|END'
    /// ```
    ///
    /// Output is CSV emitted to stderr between `--- rref-nonreg BEGIN ---`
    /// and `--- rref-nonreg END ---`.
    #[test]
    #[ignore = "slow: rref SC#5 non-regression paired 10-trial sweep (~30 s)"]
    fn test_rref_non_regression_wall_time() {
        // Previously-PASSing cells only (echelon ratio ≤ 1.5× at 38387525).
        const CELLS: &[(u64, &str, usize, &str)] = &[
            (7, "GF(7)", 64, "uniform"),
            (7, "GF(7)", 64, "deficient"),
            (7, "GF(7)", 256, "uniform"),
            (7, "GF(7)", 256, "deficient"),
            (7, "GF(7)", 1024, "uniform"),
            (7, "GF(7)", 1024, "deficient"),
            (31, "GF(31)", 64, "uniform"),
            (31, "GF(31)", 64, "deficient"),
            (31, "GF(31)", 256, "uniform"),
            (65521, "GF(65521)", 64, "uniform"),
            (65521, "GF(65521)", 64, "deficient"),
            (65521, "GF(65521)", 256, "uniform"),
            (65521, "GF(65521)", 256, "deficient"),
            (65521, "GF(65521)", 1024, "uniform"),
            (65521, "GF(65521)", 1024, "deficient"),
            (2_147_483_647, "GF(M31)", 64, "uniform"),
            (2_147_483_647, "GF(M31)", 64, "deficient"),
            (2_147_483_647, "GF(M31)", 256, "uniform"),
            (2_147_483_647, "GF(M31)", 256, "deficient"),
            (2_147_483_647, "GF(M31)", 1024, "uniform"),
        ];
        eprintln!("--- rref-nonreg BEGIN ---");
        eprintln!("op,field,n,regime,trial,wall_ns,wall_median_ns");
        for &(p, field, n, regime) in CELLS {
            let (med_a, med_b) = match p {
                7 => measure_rref_paired_cell::<7>(n, regime, field),
                31 => measure_rref_paired_cell::<31>(n, regime, field),
                65521 => measure_rref_paired_cell::<65521>(n, regime, field),
                2_147_483_647 => measure_rref_paired_cell::<2_147_483_647>(n, regime, field),
                _ => unreachable!(),
            };
            eprintln!("rref_A,{field},{n},{regime},median,,{med_a}");
            eprintln!("rref_B,{field},{n},{regime},median,,{med_b}");
            let delta_pct = (med_b as f64 - med_a as f64) / med_a as f64 * 100.0;
            eprintln!("rref_delta,{field},{n},{regime},,{delta_pct:.2}%,");
        }
        eprintln!("--- rref-nonreg END ---");
    }

    #[test]
    fn test_ple_panelized_dispatch_active_for_small_primes() {
        // Sanity probe: confirm `PLE_PANEL_COLS` and
        // `has_simd_ple_panel_base` resolve to the expected values for
        // each in-scope field.
        assert_eq!(<Fp<7> as FiniteField>::PLE_PANEL_COLS, 256);
        assert_eq!(<Fp<31> as FiniteField>::PLE_PANEL_COLS, 256);
        assert_eq!(<Fp<127> as FiniteField>::PLE_PANEL_COLS, 256);
        assert_eq!(<Fp<241> as FiniteField>::PLE_PANEL_COLS, 256);
        assert_eq!(<Fp<251> as FiniteField>::PLE_PANEL_COLS, 256);
        // GF(65521) now uses the medium-prime u16-lane PLE base case
        // (issue `68db401b`); `PLE_PANEL_COLS = 128` matches `KC_U16`.
        assert_eq!(<Fp<65521> as FiniteField>::PLE_PANEL_COLS, 128);
        // Mersenne-31 has P >= 65536; the panel base case is unchanged.
        assert_eq!(<Fp<MERSENNE_31> as FiniteField>::PLE_PANEL_COLS, 1);

        // `has_simd_ple_panel_base()` should be true for P <= 251 AND
        // for 252 <= P < 65536 (medium primes, e.g. GF(65521); issue
        // `68db401b`) on any AVX2 host with the `simd` feature.
        // Without the simd feature it always returns false (the kernel
        // dispatch is feature-gated). Detect both axes.
        #[cfg(feature = "simd")]
        {
            if std::arch::is_x86_feature_detected!("avx2") {
                assert!(<Fp<7> as FiniteField>::has_simd_ple_panel_base());
                assert!(<Fp<31> as FiniteField>::has_simd_ple_panel_base());
                assert!(<Fp<127> as FiniteField>::has_simd_ple_panel_base());
                assert!(<Fp<241> as FiniteField>::has_simd_ple_panel_base());
                assert!(<Fp<251> as FiniteField>::has_simd_ple_panel_base());
                assert!(<Fp<65521> as FiniteField>::has_simd_ple_panel_base());
            }
        }
        #[cfg(not(feature = "simd"))]
        {
            // Without `simd`, every prime should report false.
            assert!(!<Fp<7> as FiniteField>::has_simd_ple_panel_base());
            assert!(!<Fp<251> as FiniteField>::has_simd_ple_panel_base());
            assert!(!<Fp<65521> as FiniteField>::has_simd_ple_panel_base());
        }
        // P >= 65536 must NEVER advertise the panel kernel — both
        // byte-lane (P <= 251) and u16-lane (252..65536) kernels exclude
        // it.
        assert!(!<Fp<MERSENNE_31> as FiniteField>::has_simd_ple_panel_base());
    }

    /// Helper: build a rank-deficient matrix of shape (m, n) over Fp<P>
    /// with rank exactly `min(m, n) / 2`, via outer-product `F · G`.
    fn random_fp_rank_deficient<const P: u64>(
        m: usize,
        n: usize,
        rank: usize,
        seed: u64,
    ) -> FieldMatrix<Fp<P>> {
        let f = random_fp::<P>(m, rank, seed);
        let g = random_fp::<P>(rank, n, seed.wrapping_add(0x1234_5678));
        gemm(&f, &g)
    }

    /// Helper: run `check_ple` on a rank-deficient input for every
    /// (m, n) boundary pair where rank-deficient construction is possible.
    fn rank_deficient_sweep_fp<const P: u64>() {
        for &m in PANEL_BOUNDARY_LENS {
            for &n in PANEL_BOUNDARY_LENS {
                let min_dim = m.min(n);
                if min_dim < 2 {
                    continue;
                }
                let rank = min_dim / 2;
                if rank == 0 {
                    continue;
                }
                let seed = (P.wrapping_mul(0xC2B2_AE3D) ^ (m as u64).wrapping_mul(0x9E37))
                    .wrapping_add(n as u64);
                let a = random_fp_rank_deficient::<P>(m, n, rank, seed);
                let r = check_ple(&a);
                assert!(
                    r <= rank,
                    "rank-deficient construction violated: P={P} m={m} n={n} rank≤{rank} got {r}"
                );
            }
        }
    }

    #[test]
    fn test_ple_panelized_rank_deficient_fp7() {
        rank_deficient_sweep_fp::<7>();
    }

    #[test]
    fn test_ple_panelized_rank_deficient_fp31() {
        rank_deficient_sweep_fp::<31>();
    }

    #[test]
    fn test_ple_panelized_rank_deficient_fp127() {
        rank_deficient_sweep_fp::<127>();
    }

    #[test]
    fn test_ple_panelized_rank_deficient_fp241() {
        rank_deficient_sweep_fp::<241>();
    }

    #[test]
    fn test_ple_panelized_rank_deficient_fp251() {
        rank_deficient_sweep_fp::<251>();
    }

    #[test]
    fn test_ple_panelized_rank_deficient_fp65521() {
        rank_deficient_sweep_fp::<65521>();
    }

    /// Scalar PLE oracle: runs the full PLE factorisation on `a` using
    /// `ple_in_place_window_no_panel`, which explicitly bypasses the
    /// SIMD panel-base dispatch path. Used by proptests as the bit-exact
    /// reference to compare the panelized output against.
    ///
    /// The oracle is semantically identical to `FieldMatrix::ple` but
    /// calls `ple_in_place_window_no_panel` instead of `ple_in_place`
    /// so the panel-base SIMD kernel is never invoked, giving a pure
    /// scalar result.
    pub(super) fn ple_scalar_oracle<F: FiniteField>(
        a: &FieldMatrix<F>,
    ) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize) {
        let (m, n) = a.shape();
        if m == 0 || n == 0 {
            let l = zero_matrix_like(m, 0, a);
            let e = zero_matrix_like(0, n, a);
            return (Permutation::identity(m), l, e, 0);
        }
        let mut working = a.clone();
        let mut perm: Vec<usize> = (0..m).collect();
        let max_rank = m.min(n);
        let mut pivot_cols: Vec<usize> = Vec::with_capacity(max_rank);
        let rank = ple_in_place_window_no_panel::<F>(
            working.submat_mut(.., ..),
            0,
            n,
            &mut perm,
            &mut pivot_cols,
        );
        let (l, e) = split_compact(&working, rank, &pivot_cols);
        let inverse_perm = invert_perm(&perm);
        (Permutation::from_indices(inverse_perm), l, e, rank)
    }

    // Proptest: property-based sweep for the panelized PLE path over
    // all 6 small primes per SC#2. 32 cases per field at m, n in [1, 96].
    //
    // Each test:
    //   1. Generates a random matrix `a`.
    //   2. Runs the panelized PLE (`a.ple()`) — may invoke the AVX2 panel
    //      kernel for P ≤ 251.
    //   3. Runs the scalar oracle (`ple_scalar_oracle(&a)`) — forces the
    //      `ple_in_place_window_no_panel` path, bypassing all SIMD dispatch.
    //   4. Asserts bit-exact equality of
    //      `(P_panel, L_panel, E_panel, r_panel) == (P_scalar, L_scalar, E_scalar, r_scalar)`.
    //   5. Also verifies the `P · L · E == A` contract as a sanity check.
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 32, .. proptest::test_runner::Config::default() })]

        #[test]
        fn prop_ple_panelized_matches_contract_fp7(
            m in 1usize..96,
            n in 1usize..96,
            seed in 0u64..1_000_000,
        ) {
            let a = random_fp::<7>(m, n, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            // Sanity: P · L · E == A.
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_matches_contract_fp31(
            m in 1usize..96,
            n in 1usize..96,
            seed in 0u64..1_000_000,
        ) {
            let a = random_fp::<31>(m, n, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_matches_contract_fp127(
            m in 1usize..96,
            n in 1usize..96,
            seed in 0u64..1_000_000,
        ) {
            let a = random_fp::<127>(m, n, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_matches_contract_fp241(
            m in 1usize..96,
            n in 1usize..96,
            seed in 0u64..1_000_000,
        ) {
            let a = random_fp::<241>(m, n, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_matches_contract_fp251(
            m in 1usize..96,
            n in 1usize..96,
            seed in 0u64..1_000_000,
        ) {
            let a = random_fp::<251>(m, n, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_matches_contract_fp65521(
            m in 1usize..96,
            n in 1usize..96,
            seed in 0u64..1_000_000,
        ) {
            let a = random_fp::<65521>(m, n, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }
    }

    // Cross-field rank-deficient proptest: builds rank-deficient matrices
    // via outer-product construction and verifies panelized PLE output
    // is bit-exact vs the scalar oracle, reports `rank <= constructed_rank`,
    // and honours `P · L · E == A`.
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 16, .. proptest::test_runner::Config::default() })]

        #[test]
        fn prop_ple_panelized_rank_deficient_fp7(
            m in 2usize..32,
            n in 2usize..32,
            seed in 0u64..1_000_000,
        ) {
            let rank = m.min(n) / 2;
            if rank == 0 { return Ok(()); }
            let a = random_fp_rank_deficient::<7>(m, n, rank, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert!(r_panel <= rank, "rank bound violated");
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_rank_deficient_fp31(
            m in 2usize..32,
            n in 2usize..32,
            seed in 0u64..1_000_000,
        ) {
            let rank = m.min(n) / 2;
            if rank == 0 { return Ok(()); }
            let a = random_fp_rank_deficient::<31>(m, n, rank, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert!(r_panel <= rank, "rank bound violated");
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_rank_deficient_fp127(
            m in 2usize..32,
            n in 2usize..32,
            seed in 0u64..1_000_000,
        ) {
            let rank = m.min(n) / 2;
            if rank == 0 { return Ok(()); }
            let a = random_fp_rank_deficient::<127>(m, n, rank, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert!(r_panel <= rank, "rank bound violated");
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_rank_deficient_fp241(
            m in 2usize..32,
            n in 2usize..32,
            seed in 0u64..1_000_000,
        ) {
            let rank = m.min(n) / 2;
            if rank == 0 { return Ok(()); }
            let a = random_fp_rank_deficient::<241>(m, n, rank, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert!(r_panel <= rank, "rank bound violated");
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_rank_deficient_fp251(
            m in 2usize..32,
            n in 2usize..32,
            seed in 0u64..1_000_000,
        ) {
            let rank = m.min(n) / 2;
            if rank == 0 { return Ok(()); }
            let a = random_fp_rank_deficient::<251>(m, n, rank, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert!(r_panel <= rank, "rank bound violated");
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }

        #[test]
        fn prop_ple_panelized_rank_deficient_fp65521(
            m in 2usize..32,
            n in 2usize..32,
            seed in 0u64..1_000_000,
        ) {
            let rank = m.min(n) / 2;
            if rank == 0 { return Ok(()); }
            let a = random_fp_rank_deficient::<65521>(m, n, rank, seed);
            let (p_panel, l_panel, e_panel, r_panel) = a.ple();
            let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
            proptest::prop_assert!(r_panel <= rank, "rank bound violated");
            proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch");
            proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch");
            proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch");
            proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch");
            let le = if r_panel == 0 {
                zero_matrix_like(a.rows(), a.cols(), &a)
            } else {
                gemm(&l_panel, &e_panel)
            };
            let rebuild = p_panel.apply(&le);
            proptest::prop_assert_eq!(rebuild, a);
        }
    }

    // Boundary-length proptest sweep per SC#2 (jit:6823c8a0): for every
    // small prime in `{7, 31, 127, 241, 251, 65521}` the inner test body
    // exhaustively iterates **all** `(m, n)` pairs drawn from
    // `PANEL_BOUNDARY_LENS = {0, 1, 15, 16, 17, 63, 64, 65}` and asserts
    // bit-exact equality of `(P, L, E, rank)` between the panelized PLE
    // and the scalar oracle. The empty `(0, 0)` matrix is excluded
    // (`a.ple()` returns rank 0 trivially); every other boundary pair —
    // including degenerate-shape `m=0` or `n=0` rows/columns — is
    // covered every proptest case.
    //
    // The proptest macro drives **seed variance** rather than `(m, n)`
    // sampling: each of the 8 cases per prime fans out the matrix
    // generator's seed over `0u64..1_000_000`, so every boundary pair is
    // tested 8 times against 8 different random matrices in addition to
    // the standalone single-seed coverage. This gives both deterministic
    // exhaustive boundary coverage AND randomized matrix variance — both
    // halves of "proptest sweep at boundary lengths" per the SC.
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

        #[test]
        fn prop_ple_panelized_boundary_sweep_fp7(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    if m == 0 && n == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<7>(m, n, mseed);
                    let (p_panel, l_panel, e_panel, r_panel) = a.ple();
                    let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
                    proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch m={} n={}", m, n);
                }
            }
        }

        #[test]
        fn prop_ple_panelized_boundary_sweep_fp31(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    if m == 0 && n == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<31>(m, n, mseed);
                    let (p_panel, l_panel, e_panel, r_panel) = a.ple();
                    let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
                    proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch m={} n={}", m, n);
                }
            }
        }

        #[test]
        fn prop_ple_panelized_boundary_sweep_fp127(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    if m == 0 && n == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<127>(m, n, mseed);
                    let (p_panel, l_panel, e_panel, r_panel) = a.ple();
                    let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
                    proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch m={} n={}", m, n);
                }
            }
        }

        #[test]
        fn prop_ple_panelized_boundary_sweep_fp241(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    if m == 0 && n == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<241>(m, n, mseed);
                    let (p_panel, l_panel, e_panel, r_panel) = a.ple();
                    let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
                    proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch m={} n={}", m, n);
                }
            }
        }

        #[test]
        fn prop_ple_panelized_boundary_sweep_fp251(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    if m == 0 && n == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<251>(m, n, mseed);
                    let (p_panel, l_panel, e_panel, r_panel) = a.ple();
                    let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
                    proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch m={} n={}", m, n);
                }
            }
        }

        #[test]
        fn prop_ple_panelized_boundary_sweep_fp65521(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    if m == 0 && n == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<65521>(m, n, mseed);
                    let (p_panel, l_panel, e_panel, r_panel) = a.ple();
                    let (p_scalar, l_scalar, e_scalar, r_scalar) = ple_scalar_oracle(&a);
                    proptest::prop_assert_eq!(r_panel, r_scalar, "rank mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&p_panel, &p_scalar, "P mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&l_panel, &l_scalar, "L mismatch m={} n={}", m, n);
                    proptest::prop_assert_eq!(&e_panel, &e_scalar, "E mismatch m={} n={}", m, n);
                }
            }
        }
    }

    // ── RREF blocked back-substitution correctness — proptests ────────────────
    //
    // SC#2 (jit:869ce43b): bit-exact correctness of blocked RREF vs the scalar
    // oracle across GF(7), GF(31), GF(127), GF(241), GF(251), GF(65521),
    // GF(2^31-1) at all boundary (m, n) ∈ PANEL_BOUNDARY_LENS² and for both
    // uniform and rank-deficient regimes.
    //
    // Structure (matches sibling PLE proptests above):
    // - `proptest!` macro drives seed variance over 0..1_000_000, 8 cases.
    // - Inner `for m in BOUNDARY_LENS { for n in BOUNDARY_LENS { ... } }` loop
    //   provides exhaustive boundary-length coverage.
    // - `rref_scalar_oracle` bypasses any blocked dispatch path, giving the
    //   bit-exact scalar reference.
    // - Uniform regime: random matrix of shape (m, n).
    // - Rank-deficient regime: `m×n` matrix of rank ⌊min(m,n)/2⌋ via outer-product.

    /// Scalar RREF oracle: computes `self.rref()` via the unmodified scalar
    /// back-substitution loop, bypassing the `try_blocked_back_sub` fast path.
    /// Used by proptest sweep as the bit-exact reference.
    ///
    /// Implementation: calls `row_echelon()` (which already uses the panelized
    /// PLE), then applies the scalar pivot-column loop verbatim from the original
    /// `rref()` body (the fallback branch that `try_blocked_back_sub` bypasses).
    fn rref_scalar_oracle<F: FiniteField>(a: &FieldMatrix<F>) -> (FieldMatrix<F>, FieldMatrix<F>) {
        let (m, n) = a.shape();
        if m == 0 || n == 0 {
            return a.row_echelon();
        }
        let (mut x, mut e) = a.row_echelon();
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        // Identify pivots.
        let mut pivots: Vec<(usize, usize)> = Vec::new();
        let mut last: isize = -1;
        for i in 0..m {
            let start = (last + 1).max(0) as usize;
            let mut found: Option<usize> = None;
            for j in start..n {
                if e.get(i, j) != zero {
                    found = Some(j);
                    break;
                }
            }
            if let Some(p) = found {
                pivots.push((i, p));
                last = p as isize;
            } else {
                break;
            }
        }
        // Scalar back-substitution (original rref fallback, verbatim).
        for &(pi, pc) in &pivots {
            let pivot_val = e.get(pi, pc);
            if pivot_val != one {
                let inv = pivot_val.inv().unwrap();
                for j in 0..n {
                    let v = e.get(pi, j) * inv.clone();
                    e.set(pi, j, v);
                }
                for j in 0..m {
                    let v = x.get(pi, j) * inv.clone();
                    x.set(pi, j, v);
                }
            }
            for k in 0..m {
                if k == pi {
                    continue;
                }
                let factor = e.get(k, pc);
                if factor == zero {
                    continue;
                }
                for j in 0..n {
                    let v = e.get(k, j) - factor.clone() * e.get(pi, j);
                    e.set(k, j, v);
                }
                for j in 0..m {
                    let v = x.get(k, j) - factor.clone() * x.get(pi, j);
                    x.set(k, j, v);
                }
            }
        }
        (x, e)
    }

    // Boundary-length proptest sweep for blocked RREF across all 7 primes.
    // Cases: 8 (seed variance); (m, n) exhaustive over PANEL_BOUNDARY_LENS.
    // Regime: uniform.
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

        // § 6.3 §7.1 — GF(7) uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_fp7(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<7>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }

        // GF(31) uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_fp31(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<31>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }

        // GF(127) uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_fp127(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<127>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }

        // GF(241) uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_fp241(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<241>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }

        // GF(251) uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_fp251(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<251>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }

        // GF(65521) uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_fp65521(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<65521>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }

        // GF(2^31-1) Mersenne31 uniform boundary sweep
        #[test]
        fn prop_blocked_rref_boundary_sweep_uniform_mersenne31(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp::<MERSENNE_31>(m, n, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} seed={}", m, n, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} seed={}", m, n, mseed);
                }
            }
        }
    }

    // Rank-deficient regime proptest sweep.
    // For each (m, n) pair where rank-deficient construction is possible
    // (min(m, n) >= 2), generate a matrix of rank ⌊min(m,n)/2⌋ and assert
    // bit-exact RREF output.
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_fp7(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<7>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_fp31(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<31>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_fp127(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<127>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_fp241(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<241>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_fp251(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<251>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_fp65521(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<65521>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }

        #[test]
        fn prop_blocked_rref_boundary_sweep_deficient_mersenne31(seed in 0u64..1_000_000) {
            for &m in PANEL_BOUNDARY_LENS {
                for &n in PANEL_BOUNDARY_LENS {
                    let min_dim = m.min(n);
                    if min_dim < 2 { continue; }
                    let rank = min_dim / 2;
                    if rank == 0 { continue; }
                    let mseed = seed
                        .wrapping_add((m as u64).wrapping_mul(0x9E37_79B9))
                        .wrapping_add((n as u64).wrapping_mul(0x517C_C1B7));
                    let a = random_fp_rank_deficient::<MERSENNE_31>(m, n, rank, mseed);
                    let (x_blocked, r_blocked) = a.rref();
                    let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
                    proptest::prop_assert_eq!(&r_blocked, &r_scalar,
                        "R mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                    proptest::prop_assert_eq!(&x_blocked, &x_scalar,
                        "X mismatch m={} n={} rank={} seed={}", m, n, rank, mseed);
                }
            }
        }
    }
}
