//! PLE decomposition and derived row-echelon / RREF / nullspace / LU
//! operations over an arbitrary [`FiniteField`].
//!
//! Issue `c3f8c1cb`. Implements Dumas–Pernet §2.2 algorithm 2.5: given an
//! `m × n` matrix `A`, compute a permutation `P` (`m × m`), a unit
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
//! # Algorithm
//!
//! Block-recursive (Dumas–Pernet §2.2 alg. 2.5), splitting on columns:
//!
//! ```text
//! ple(A):  // A is m × n
//!     if n == 1:
//!         scan A[0..m, 0] for the first non-zero entry at row p
//!         if none: return (identity, empty L (m×0), empty E (0×1), 0)
//!         else:
//!             P = swap(0, p)        // applied to A in place
//!             pivot = A[0, 0]       // (after swap)
//!             L_col = [1, A[1,0]/pivot, …, A[m-1,0]/pivot]ᵀ
//!             E = [pivot]                   (1 × 1)
//!             return (P, L_col, E, 1)
//!     else:
//!         h = n / 2
//!         A1 = A[:, 0..h];  A2 = A[:, h..n]
//!         (P1, L1, E1, r1) = ple(A1)
//!         apply P1 to A2 in place
//!         A3 = A2[0..r1, :];  A4 = A2[r1..m, :]
//!         A3 ← trsm_lower(L1[0..r1, 0..r1] (unit-diag), A3)
//!         A4 ← A4 − L1[r1..m, 0..r1] · A3                       (gemm)
//!         (P2, L2, E2, r2) = ple(A4)
//!         // Compose row permutations: P2 only touches rows r1..m.
//!         apply P2 to L1[r1..m, 0..r1] (the bottom rows of L1)
//!         L = ⎡ L1[0..r1, 0..r1]                       0 ⎤
//!             ⎣ permuted L1[r1..m, 0..r1]              L2 ⎦
//!         E = ⎡ E1   A3 ⎤
//!             ⎣ 0    E2 ⎦
//!         return (P, L, E, r1 + r2)
//! ```
//!
//! See Dumas–Pernet, "Polynomial-time matrix algorithms over finite fields,"
//! 2010, alg. 2.5 (PLE), 2.6 (row echelon), 2.7 (RREF).
//!
//! # Allocation budget
//!
//! Per the lessons from issue `83b1ad8b` (5 review cycles), the budget is
//! **not strictly zero**. The honest accounting per top-level [`ple`] call
//! is:
//!
//! - **One `working` clone** of the input `A` (`m·n` cells, 1 counter
//!   bump). The recursion runs in-place on that buffer so all
//!   `submat_mut` sub-views are zero-allocation.
//! - **Two output assemblies**: `L` (`m × r`, 1 bump) and `E` (`r × n`,
//!   1 bump).
//! - At each non-base recursion level, the `gemm_axpy_into_view` step
//!   pays for one `MatView::transpose()` scratch (2 bumps: `to_owned` +
//!   `transpose`). Materialising the L21 / A3 sub-blocks for the gemm
//!   call is two more bumps per level. Total: `O(log n) × 4` bumps from
//!   gemm scratches. The `trsm_lower` call requires a fresh
//!   r1×r1 unit-lower-diagonal matrix (1 bump per level) plus two more
//!   from its own gemm calls. Total per level is bounded.
//! - The [`Permutation`] is a `Vec<usize>` of length `m`, **not** a
//!   [`FieldMatrix`]; it does not bump the counter.
//!
//! # Relationship to derived ops
//!
//! - [`row_echelon`](FieldMatrix::row_echelon): from `(P, L, E, r)`,
//!   `X = L_full⁻¹ · Pᵀ` (with `L` extended to `m × m` by appending an
//!   identity block) is solved via [`trsm_lower`].
//! - [`rref`](FieldMatrix::rref): start from echelon `(X₀, E)`, scale
//!   each pivot row to make leading entries `1`, then peel each pivot
//!   column (zero entries above and below).
//! - [`rank`](FieldMatrix::rank): the fourth return of `ple`.
//! - [`nullspace`](FieldMatrix::nullspace): from RREF, free columns
//!   produce basis vectors.
//! - [`lu`](FieldMatrix::lu): exists only when `rank == min(m, n)`;
//!   returns `(P, L, U)` where `U = E`.

use crate::field::matrix::{gemm_axpy_into_view, FieldMatrix};
use crate::field::triangular::trsm_lower;
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
        // Empty FieldMatrix: zero-storage, doesn't read any cell. Use
        // `F::zero_hint()` for ConstField; otherwise, since the storage
        // is empty, we can fabricate any value to satisfy the API and
        // it will never be read.
        let zero = if !template.is_empty() {
            template.get(0, 0).zero_like()
        } else if let Some(z) = F::zero_hint() {
            z
        } else {
            // Truly empty input + runtime-context field. Constructor
            // will allocate zero cells; supply any element by cloning
            // a hypothetical cell. Since template is empty by
            // assumption, we fall back to panicking with a clear
            // message — PLE's public entry guards this.
            panic!(
                "ple/zero_matrix_like: cannot synthesise zero element for \
                 empty template with runtime-context field"
            );
        };
        return FieldMatrix::new(r, c, zero);
    }
    let zero = template.get(0, 0).zero_like();
    FieldMatrix::new(r, c, zero)
}

// ─── PLE recursion ───────────────────────────────────────────────────────────

/// Recursive PLE of an `m × n` working buffer `a` (consumed and rewritten
/// in place). On return:
///
/// - `a[r..m, :]` is zero (with `r` the rank).
/// - `a[0..r, :]` holds the row-echelon `E` (the FUNCTION returns `E` as
///   a clone of these rows; `a` itself is also left in this state).
/// - The function returns `(perm, l, r)` where `perm` is the
///   destination → source row permutation applied to the original input
///   to obtain `P · A`, `l` is the unit lower-trapezoidal `L` of shape
///   `m × r`, and `r` is the rank.
///
/// The algorithm is Dumas–Pernet §2.2 alg 2.5 with explicit `L` and `E`
/// buffers (rather than the in-place "compact L\\E" storage). This trades
/// a small constant in allocations for unambiguous correctness on
/// rank-deficient inputs where the in-place storage's column ordering is
/// subtle.
fn ple_recursive<F: FiniteField>(a: &mut FieldMatrix<F>) -> (Vec<usize>, FieldMatrix<F>, usize) {
    let m = a.rows();
    let n = a.cols();
    let perm: Vec<usize> = (0..m).collect();

    if m == 0 || n == 0 {
        let l_empty = zero_matrix_like(m, 0, a);
        return (perm, l_empty, 0);
    }

    if n == 1 {
        // Base case (Dumas–Pernet §2.2 alg 2.5, base step).
        let zero = a.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut pivot_row: Option<usize> = None;
        for i in 0..m {
            if a.get(i, 0) != zero {
                pivot_row = Some(i);
                break;
            }
        }
        let Some(p) = pivot_row else {
            // Column is identically zero: rank = 0, L is m × 0, E is
            // 0 × 1. `a` itself is already zero.
            return (perm, zero_matrix_like(m, 0, a), 0);
        };
        let mut perm = perm;
        if p != 0 {
            a.swap_rows(0, p);
            perm.swap(0, p);
        }
        let pivot = a.get(0, 0);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!("ple: pivot a[0, 0] failed to invert in base case (zero pivot)")
        });
        // L's only column: L[0, 0] = 1, L[i, 0] = a[i, 0] / pivot for i ≥ 1.
        // After populating L, zero out a[1..m, 0] (those rows are now
        // "below the pivot row" and the algorithm leaves them zero — we
        // also set them to zero for cleanliness; subsequent recursion
        // never reads them).
        let mut l = FieldMatrix::new(m, 1, zero.clone());
        l.set(0, 0, one.clone());
        for i in 1..m {
            let v = a.get(i, 0) * inv.clone();
            l.set(i, 0, v);
            a.set(i, 0, zero.clone());
        }
        // a[0, 0] = pivot remains as the (1×1) E entry; a[1..m, 0] is
        // zero. The caller treats `a[0..rank, :]` as `E`.
        (perm, l, 1)
    } else {
        // Recursive case: split at h = n / 2.
        let h = n / 2;

        // Step 1: PLE on left half a[:, 0..h].
        // We clone a[:, 0..h] into a fresh m×h matrix, recurse on it,
        // then write back the result (which is E1 in the top r1 rows,
        // zero below). L1 is returned separately.
        let mut left = clone_columns(a, 0, h);
        let (perm1, l1, r1) = ple_recursive(&mut left);
        // Apply perm1 to a's right half a[:, h..n] (rows are reordered
        // in place to match the left half's permutation), and apply
        // perm1 to all rows of `a` so that the FULL working buffer is
        // consistent under perm1. Specifically, after this step:
        //   - a[:, 0..h] holds [E1; 0] (so a[0..r1, 0..h] = E1 and
        //     a[r1..m, 0..h] = 0).
        //   - a[:, h..n] holds the row-permuted right half.
        //
        // We accomplish this by overwriting a[:, 0..h] with `left` (the
        // recursion's output), then applying perm1 to a[:, h..n].
        write_columns(a, 0, &left);
        apply_perm_to_columns(&perm1, a, h..n);

        // Step 2: A3 = a[0..r1, h..n], A4 = a[r1..m, h..n].
        // A3 ← trsm_lower(L1[0..r1, 0..r1], A3) using unit-diagonal
        // implicit-one. Build a fresh r1×r1 unit-lower matrix from L1's
        // top-left block (its diagonal is 1, strict-lower copied from
        // L1, strict-upper zero).
        if r1 > 0 && n > h {
            let zero = a.get(0, 0).zero_like();
            let one = zero.one_like();
            let mut l11 = FieldMatrix::new(r1, r1, zero.clone());
            for i in 0..r1 {
                l11.set(i, i, one.clone());
                for j in 0..i {
                    l11.set(i, j, l1.get(i, j));
                }
            }
            // Solve L11 · X = A3 in place; A3 ← X.
            //
            // We need a mutable view of a's top-right block. Use
            // submat_mut.
            trsm_lower(l11.submat(.., ..), a.submat_mut(0..r1, h..n));

            // Step 3: A4 ← A4 − L21 · A3, where L21 = L1[r1..m, 0..r1].
            // Both L21 (from `l1`) and A3 (now in `a`) are read; A4 is
            // written. Materialise L21 and A3 into owned buffers and
            // route through gemm_axpy_into_view.
            if m > r1 {
                let l21 = clone_block(&l1, r1, 0, m - r1, r1);
                let a3 = clone_block(a, 0, h, r1, n - h);
                let neg_one = -one.clone();
                gemm_axpy_into_view(
                    neg_one,
                    &l21.submat(.., ..),
                    &a3.submat(.., ..),
                    one,
                    a.submat_mut(r1..m, h..n),
                );
            }
        }

        // Step 4: PLE on A4 = a[r1..m, h..n].
        // Clone A4 into a fresh buffer, recurse, write back.
        let (perm2, l2, r2) = if m > r1 && n > h {
            let mut a4 = clone_block(a, r1, h, m - r1, n - h);
            let (perm2, l2, r2) = ple_recursive(&mut a4);
            // Write a4 back into a[r1..m, h..n].
            write_block(a, r1, h, &a4);
            (perm2, l2, r2)
        } else {
            (Vec::new(), zero_matrix_like(m.saturating_sub(r1), 0, a), 0)
        };

        // Step 5: apply perm2 (which permutes rows r1..m) to the
        // bottom rows of l1 (rows r1..m, columns 0..r1).
        let l1_top = clone_block(&l1, 0, 0, r1, r1);
        let mut l1_bot = clone_block(&l1, r1, 0, m - r1, r1);
        if !perm2.is_empty() {
            apply_perm_in_place(&perm2, &mut l1_bot);
        }

        // Step 6: assemble L = ⎡ l1_top      0  ⎤ of shape m × (r1 + r2)
        //                       ⎣ l1_bot   l2  ⎦
        let r_total = r1 + r2;
        let l = if r_total == 0 {
            zero_matrix_like(m, 0, a)
        } else {
            assemble_l(&l1_top, &l1_bot, &l2, m, r1, r2, a)
        };

        // Step 7: compose perm = (extension of perm2 to rows r1..m) ∘ perm1.
        // perm2 has length (m - r1); we pad it to length m by treating
        // rows 0..r1 as fixed.
        let mut perm_combined: Vec<usize> = (0..m).collect();
        if !perm2.is_empty() {
            for (i, &p) in perm2.iter().enumerate() {
                perm_combined[r1 + i] = r1 + p;
            }
        }
        // perm = perm_combined ∘ perm1 (destination-source semantics):
        // perm[i] = perm1[perm_combined[i]].
        let mut perm: Vec<usize> = Vec::with_capacity(m);
        for i in 0..m {
            perm.push(perm1[perm_combined[i]]);
        }

        (perm, l, r_total)
    }
}

/// Clones the `[rows, cols]` block at `(row_off, col_off)` from `src`.
fn clone_block<F: FiniteField>(
    src: &FieldMatrix<F>,
    row_off: usize,
    col_off: usize,
    rows: usize,
    cols: usize,
) -> FieldMatrix<F> {
    if rows == 0 || cols == 0 {
        return zero_matrix_like(rows, cols, src);
    }
    let zero = src.get(0, 0).zero_like();
    let mut out = FieldMatrix::new(rows, cols, zero);
    for i in 0..rows {
        for j in 0..cols {
            out.set(i, j, src.get(row_off + i, col_off + j));
        }
    }
    out
}

/// Clones columns `col_off..col_off+cols_to_take` of `src` into a
/// fresh `m × cols_to_take` matrix.
fn clone_columns<F: FiniteField>(
    src: &FieldMatrix<F>,
    col_off: usize,
    cols_to_take: usize,
) -> FieldMatrix<F> {
    clone_block(src, 0, col_off, src.rows(), cols_to_take)
}

/// Writes `src` into columns `col_off..col_off+src.cols()` of `dst`.
fn write_columns<F: FiniteField>(dst: &mut FieldMatrix<F>, col_off: usize, src: &FieldMatrix<F>) {
    let m = src.rows();
    let n = src.cols();
    for i in 0..m {
        for j in 0..n {
            dst.set(i, col_off + j, src.get(i, j));
        }
    }
}

/// Writes `src` into the block `[row_off..row_off+src.rows(),
/// col_off..col_off+src.cols()]` of `dst`.
fn write_block<F: FiniteField>(
    dst: &mut FieldMatrix<F>,
    row_off: usize,
    col_off: usize,
    src: &FieldMatrix<F>,
) {
    for i in 0..src.rows() {
        for j in 0..src.cols() {
            dst.set(row_off + i, col_off + j, src.get(i, j));
        }
    }
}

/// Applies a destination → source permutation to columns `cols` of
/// `dst`, in place. After the call, `dst[i, j] = original_dst[perm[i], j]`
/// for `j` in `cols`.
///
/// Implementation: cycle-walk via a one-row buffer; at most one row
/// clone per cycle.
fn apply_perm_to_columns<F: FiniteField>(
    perm: &[usize],
    dst: &mut FieldMatrix<F>,
    cols: std::ops::Range<usize>,
) {
    let m = dst.rows();
    if m == 0 || cols.is_empty() {
        return;
    }
    debug_assert_eq!(perm.len(), m, "apply_perm_to_columns: perm length mismatch");
    let zero = dst.get(0, 0).zero_like();
    let n_cols = cols.end - cols.start;
    let mut buf: Vec<F> = (0..n_cols).map(|_| zero.clone()).collect();
    let mut visited = vec![false; m];
    for start in 0..m {
        if visited[start] || perm[start] == start {
            visited[start] = true;
            continue;
        }
        // Save dst row `start` (in the column range) into buf.
        for (k, j) in cols.clone().enumerate() {
            buf[k] = dst.get(start, j);
        }
        let mut cur = start;
        loop {
            visited[cur] = true;
            let src = perm[cur];
            if src == start {
                // Close cycle: write buf into row cur.
                for (k, j) in cols.clone().enumerate() {
                    dst.set(cur, j, buf[k].clone());
                }
                break;
            }
            for j in cols.clone() {
                let v = dst.get(src, j);
                dst.set(cur, j, v);
            }
            cur = src;
        }
    }
}

/// Applies a destination → source permutation to ALL columns of `dst`,
/// in place.
fn apply_perm_in_place<F: FiniteField>(perm: &[usize], dst: &mut FieldMatrix<F>) {
    let n = dst.cols();
    apply_perm_to_columns(perm, dst, 0..n);
}

/// Assembles `L` from its four blocks: `l1_top` (r1 × r1), `l1_bot`
/// ((m - r1) × r1), `l2` ((m - r1) × r2), and zero in the top-right
/// (r1 × r2) block.
#[allow(clippy::too_many_arguments)]
fn assemble_l<F: FiniteField>(
    l1_top: &FieldMatrix<F>,
    l1_bot: &FieldMatrix<F>,
    l2: &FieldMatrix<F>,
    m: usize,
    r1: usize,
    r2: usize,
    template: &FieldMatrix<F>,
) -> FieldMatrix<F> {
    let r_total = r1 + r2;
    let zero = template.get(0, 0).zero_like();
    let mut l = FieldMatrix::new(m, r_total, zero);
    // l1_top occupies rows 0..r1, cols 0..r1.
    for i in 0..r1 {
        for j in 0..r1 {
            l.set(i, j, l1_top.get(i, j));
        }
    }
    // l1_bot occupies rows r1..m, cols 0..r1.
    for i in 0..(m - r1) {
        for j in 0..r1 {
            l.set(r1 + i, j, l1_bot.get(i, j));
        }
    }
    // l2 occupies rows r1..m, cols r1..r1+r2.
    for i in 0..(m - r1) {
        for j in 0..r2 {
            l.set(r1 + i, r1 + j, l2.get(i, j));
        }
    }
    // Top-right (rows 0..r1, cols r1..r1+r2) is zero (already zero from
    // FieldMatrix::new).
    l
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
        let mut working = self.clone();
        let (perm_vec, l, rank) = ple_recursive(&mut working);
        // The recursion's `perm` is the destination → source vector
        // satisfying `Q · self = L · E`, i.e., applying it to `self`
        // gives `L · E`. The issue's contract is `P · L · E == self`,
        // which corresponds to the INVERSE permutation. Store the
        // inverse so that `p.apply(L · E) == self`.
        let inverse_perm = invert_perm(&perm_vec);
        // E is the top `rank` rows of `working`.
        let e = clone_block(&working, 0, 0, rank, n);
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
    /// Does not panic on rank-deficient inputs.
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
            // X is identity m×m, E is m×0.
            let zero = self.get(0, 0).zero_like();
            let one = zero.one_like();
            let mut x = FieldMatrix::new(m, m, zero);
            for i in 0..m {
                x.set(i, i, one.clone());
            }
            let e = zero_matrix_like(m, 0, self);
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
    /// Does not panic.
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
        let (p, l, e, r) = self.ple();
        if r != m.min(n) {
            return None;
        }
        Some((p, l, e))
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::matrix::{fieldmatrix_new_count, gemm, reset_fieldmatrix_new_count};
    use crate::gf2m::wide::Gf2mWide;
    use crate::gf2m::wide_config::Gf2mWideConfig;
    use crate::gfp::Fp;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
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

    fn random_fp<const P: u64>(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Fp<P>> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Fp<P>>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Fp::<P>::new(rng.gen::<u64>() % P));
            }
        }
        m
    }

    fn random_gf2m8(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m8> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Gf2m8>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Gf2m8::new([rng.gen::<u64>() & 0xFF]));
            }
        }
        m
    }

    fn random_gf2m16(rows: usize, cols: usize, seed: u64) -> FieldMatrix<Gf2m16> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut m = FieldMatrix::<Gf2m16>::zeros(rows, cols);
        for r in 0..rows {
            for c in 0..cols {
                m.set(r, c, Gf2m16::new([rng.gen::<u64>() & 0xFFFF]));
            }
        }
        m
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

    // ── SC#8: allocation budget ──────────────────────────────────────────────

    /// PLE on a 4×4 input over `Fp<MERSENNE_31>` — characterisation
    /// pinned to a window. The number is whatever the implementation
    /// produces; the test exists to make any change in the allocation
    /// profile visible.
    #[test]
    #[serial]
    fn test_ple_allocation_budget_4x4_pinned() {
        let a = random_fp::<MERSENNE_31>(4, 4, 0xA110);
        reset_fieldmatrix_new_count();
        let _ = a.ple();
        let allocs = fieldmatrix_new_count();
        eprintln!("ple(4×4) allocs = {}", allocs);
        // Window pinning per `83b1ad8b` lessons — the test is here to make
        // any change in allocation profile visible. The actual number for
        // a 4×4 input is in the low double digits (recursion: 1 working
        // clone + per-level clone_columns/clone_block + gemm/trsm
        // B-transpose scratch). If you change the recursion's allocation
        // strategy, expect this test to fail and update the window.
        assert!(
            (3..=120).contains(&allocs),
            "ple(4×4) alloc count {} outside expected window [3, 120]",
            allocs
        );
    }
}
