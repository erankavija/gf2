//! PLE decomposition `P · L · E = A` over an arbitrary [`FiniteField`]
//! (`@/citation/DumasPernet2012` §2.2, alg. 2.5) and the row-echelon, RREF,
//! rank, nullspace and LU operations derived from it.

use crate::field::matrix::{
    gemm_axpy_into_view, gemm_axpy_into_view_tiled, FieldMatrix, MatView, MatViewMut,
    ObservationPolicy, RecordObservations, GEMM_COL_TILE, GEMM_ROW_TILE,
};
use crate::field::triangular::{trsm_lower, trsm_lower_with_policy, trsm_upper};
use crate::field::vec::FieldVec;
use crate::field::{FiniteField, PlePanelLane};
use crate::tuning;

/// Conservative default for `ple.scalar_base_max_cols()`: the widest column
/// window [`FieldMatrix::ple`]'s block-recursive driver hands to the direct
/// column-by-column base case.
pub(crate) const PLE_SCALAR_BASE_MAX_COLS_DEFAULT: usize = 1;

/// A row permutation produced by [`FieldMatrix::ple`], stored as a
/// destination → source index vector: applying it to `A` yields `B` with
/// `B[i, *] = A[perm[i], *]`, that is `B = P · A` for `P[i, perm[i]] = 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permutation {
    perm: Vec<usize>,
}

impl Permutation {
    /// The identity permutation on `n` rows.
    pub fn identity(n: usize) -> Self {
        Self {
            perm: (0..n).collect(),
        }
    }

    /// Builds a permutation from a destination → source index vector.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if `perm` is not a permutation of
    /// `0..perm.len()`; release builds trust the caller.
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

    /// The destination → source index vector.
    pub fn indices(&self) -> &[usize] {
        &self.perm
    }

    /// Number of rows the permutation acts on.
    pub fn len(&self) -> usize {
        self.perm.len()
    }

    /// Returns `true` if this permutation has length zero.
    pub fn is_empty(&self) -> bool {
        self.perm.is_empty()
    }

    /// Returns the inverse permutation `P⁻¹`.
    pub fn inverse(&self) -> Permutation {
        let n = self.perm.len();
        let mut inv = vec![0usize; n];
        for (i, &j) in self.perm.iter().enumerate() {
            inv[j] = i;
        }
        Permutation { perm: inv }
    }

    /// Returns `P · m`: output row `i` is row `self.indices()[i]` of `m`.
    ///
    /// # Panics
    ///
    /// Panics if `m.rows() != self.len()`.
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

/// Direct column-by-column elimination of the column window
/// `[col_lo, col_hi)`, the [`PleBaseRoute::ScalarBase`] arm, in the compact
/// storage of [`split_compact`]. Returns the number of pivots found.
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
        let mut pivot_row: Option<usize> = None;
        for i in rank..m {
            if a.get(i, col) != zero {
                pivot_row = Some(i);
                break;
            }
        }
        let Some(p) = pivot_row else {
            continue;
        };

        // A full-row swap keeps the already-processed columns consistent
        // with `perm`.
        if p != rank {
            a.swap_rows(rank, p);
            perm.swap(rank, p);
        }

        // `a[rank, col]` keeps the pivot (E's entry); the rows below become
        // L's multipliers.
        let pivot = a.get(rank, col);
        let inv = pivot.inv().unwrap_or_else(|| {
            panic!("ple_base_direct: pivot a[{rank}, {col}] failed to invert (zero pivot)")
        });
        for k in (rank + 1)..m {
            let v = a.get(k, col) * inv.clone();
            a.set(k, col, v);
        }

        for c in (col + 1)..col_hi {
            let pivot_c = a.get(rank, c);
            if pivot_c == zero {
                continue;
            }
            for k in (rank + 1)..m {
                let mult = a.get(k, col);
                let v = a.get(k, c) - mult.clone() * pivot_c.clone();
                a.set(k, c, v);
            }
        }

        // `split_compact` reads `pivot_cols` instead of rescanning for pivots.
        pivot_cols.push(col);
        rank += 1;
    }

    rank
}

/// Widest column window handed to the panel kernel since the last
/// [`reset_max_effective_panel_dispatch_cols`] call. Zero means "none".
/// Route-observation tests read this to prove the installed per-lane
/// ceiling bounds every panel dispatch, including the sub-panels the
/// recursive splitter produces — output equality cannot show that.
#[cfg(any(test, feature = "test-support"))]
static MAX_EFFECTIVE_PANEL_DISPATCH_COLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Reads the widest panel-kernel dispatch window since the last reset.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn max_effective_panel_dispatch_cols() -> Option<usize> {
    match MAX_EFFECTIVE_PANEL_DISPATCH_COLS.load(std::sync::atomic::Ordering::SeqCst) {
        0 => None,
        w => Some(w),
    }
}

/// Clears [`max_effective_panel_dispatch_cols`].
#[cfg(any(test, feature = "test-support"))]
pub fn reset_max_effective_panel_dispatch_cols() {
    MAX_EFFECTIVE_PANEL_DISPATCH_COLS.store(0, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn record_ple_panel_cols(cols: usize) {
    MAX_EFFECTIVE_PANEL_DISPATCH_COLS.fetch_max(cols, std::sync::atomic::Ordering::SeqCst);
}

/// Runs the field's `try_simd_ple_panel_base` hook on the column window
/// `[col_lo, col_hi)` of the view's rows; `None` when the kernel declines.
fn try_panel_base_dispatch<F: FiniteField, O: ObservationPolicy>(
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
    let row_start = row_offset * parent_cols;
    let row_end = row_start + rows * parent_cols;
    let sub = &mut data[row_start..row_end];
    let result =
        F::try_simd_ple_panel_base(sub, parent_cols, rows, col_lo, col_hi, perm, pivot_cols);
    if result.is_some() {
        O::ple_panel_cols(col_hi - col_lo);
    }
    result
}

/// The PLE column-width selectors resolved once per [`FieldMatrix::ple`] call
/// from the active [`crate::tuning::CoreTuning`].
///
/// [`ple_in_place`] builds one at the panel entry and threads it by parameter
/// through [`ple_in_place_window`], [`ple_panel_recursive_window`] and
/// [`ple_in_place_window_no_panel`], so no recursion node reads the profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PleWidths {
    /// `ple.scalar_base_max_cols()` — the widest window handed to the direct
    /// column-by-column base case.
    scalar_base_max_cols: usize,
    /// `ple.panel_base_max_cols()` — the widest window the panel base handles
    /// without a sub-panel walk.
    panel_base_max_cols: usize,
    /// The active width for the carrier's own panel-kernel lane class, or
    /// `None` when the carrier registers no panel kernel.
    panel_lane_max_cols: Option<usize>,
}

impl PleWidths {
    /// Resolves the three widths for carrier `F` from the active profile.
    fn resolve<F: FiniteField>() -> Self {
        let tuning = tuning::active();
        let ple = tuning.ple();
        let widths = Self {
            scalar_base_max_cols: ple.scalar_base_max_cols(),
            panel_base_max_cols: ple.panel_base_max_cols(),
            panel_lane_max_cols: ple_lane_max_cols(ple, F::simd_ple_panel_lane()),
        };
        // A lane width below the scalar base leaves the panel branch dead
        // rather than wrong: the scalar base case has already returned for
        // every window that narrow.
        debug_assert!(
            widths
                .panel_lane_max_cols
                .is_none_or(|lane_max_cols| lane_max_cols >= widths.scalar_base_max_cols),
            "ple panel lane width ({:?}) must be >= ple.scalar_base_max_cols ({})",
            widths.panel_lane_max_cols,
            widths.scalar_base_max_cols
        );
        widths
    }
}

/// The active profile's panel width for `lane`: `ple.panel_byte_lane_max_cols()`
/// for [`PlePanelLane::Byte`] and `ple.panel_u16_lane_max_cols()` for
/// [`PlePanelLane::U16`]. A carrier with no registered panel kernel has no panel
/// width, so `None` carries through.
fn ple_lane_max_cols(
    ple: &crate::tuning::PleSelectors,
    lane: Option<PlePanelLane>,
) -> Option<usize> {
    lane.map(|lane| match lane {
        PlePanelLane::Byte => ple.panel_byte_lane_max_cols(),
        PlePanelLane::U16 => ple.panel_u16_lane_max_cols(),
    })
}

/// In-place PLE of `a` in the compact storage of [`split_compact`]. Records
/// destination → source row swaps in `perm`, appends the pivot columns to
/// `pivot_cols` and returns the rank.
///
/// `a` spans the full column range of the working matrix, so `swap_rows`
/// moves whole rows.
fn ple_in_place<F: FiniteField, O: ObservationPolicy>(
    mut a: MatViewMut<'_, F>,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
) -> usize {
    let n = a.cols();
    let widths = PleWidths::resolve::<F>();
    let rank = ple_in_place_window::<F, O>(a.reborrow(), 0, n, perm, pivot_cols, widths);
    O::ple_base_route(
        widths.scalar_base_max_cols,
        ple_base_route_code(ple_base_route_resolved(widths.scalar_base_max_cols, n)),
    );
    rank
}

/// Conservative default for `ple.panel_base_max_cols()`: the widest column
/// window the panel base handles directly before [`ple_in_place_window`]
/// splits it into sub-panels.
pub(crate) const PLE_PANEL_RECURSIVE_BASE: usize = 128;

/// The selected arm of the [`FieldMatrix::ple`] base-case dispatcher for one
/// column window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PleBaseRoute {
    /// The column window is eliminated directly, column by column, by
    /// `ple_base_direct`.
    ScalarBase,
    /// The column window enters the block-recursive driver: one of the panel
    /// arms [`ple_panel_route`] reports, or the binary-halving trsm + gemm
    /// split.
    BlockRecursive,
}

/// Reports the [`FieldMatrix::ple`] base-case arm for a column-window width.
///
/// The comparison uses the active `ple.scalar_base_max_cols()` profile value:
/// a window at or below it is eliminated directly. The conservative default is
/// `PLE_SCALAR_BASE_MAX_COLS_DEFAULT`.
#[must_use]
pub fn ple_base_route(win: usize) -> PleBaseRoute {
    ple_base_route_resolved(tuning::active().ple().scalar_base_max_cols(), win)
}

/// Reports the base-case arm for a column-window width against an
/// already-resolved `scalar_base_max_cols`.
fn ple_base_route_resolved(scalar_base_max_cols: usize, win: usize) -> PleBaseRoute {
    if win <= scalar_base_max_cols {
        PleBaseRoute::ScalarBase
    } else {
        PleBaseRoute::BlockRecursive
    }
}

#[inline(always)]
const fn ple_base_route_code(route: PleBaseRoute) -> usize {
    match route {
        PleBaseRoute::ScalarBase => 1,
        PleBaseRoute::BlockRecursive => 2,
    }
}

#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_PLE_BASE_MAX_COLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(any(test, feature = "test-support"))]
static LAST_EFFECTIVE_PLE_BASE_ROUTE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn record_ple_base_route(scalar_base_max_cols: usize, route: usize) {
    LAST_EFFECTIVE_PLE_BASE_MAX_COLS
        .store(scalar_base_max_cols, std::sync::atomic::Ordering::SeqCst);
    LAST_EFFECTIVE_PLE_BASE_ROUTE.store(route, std::sync::atomic::Ordering::SeqCst);
}

/// Clears [`last_effective_ple_base_route`].
#[cfg(any(test, feature = "test-support"))]
pub fn reset_last_effective_ple_base_route() {
    LAST_EFFECTIVE_PLE_BASE_ROUTE.store(0, std::sync::atomic::Ordering::SeqCst);
    LAST_EFFECTIVE_PLE_BASE_MAX_COLS.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// Returns the resolved `ple.scalar_base_max_cols()` and the top-level
/// [`PleBaseRoute`] of the most recent [`FieldMatrix::ple`] decomposition
/// since the last reset.
///
/// The decomposition publishes once, after the whole column window
/// completes. [`ple_base_route`] alone publishes nothing.
#[cfg(any(test, feature = "test-support"))]
#[must_use]
pub fn last_effective_ple_base_route() -> Option<(usize, PleBaseRoute)> {
    let scalar_base_max_cols =
        LAST_EFFECTIVE_PLE_BASE_MAX_COLS.load(std::sync::atomic::Ordering::SeqCst);
    if scalar_base_max_cols == 0 {
        return None;
    }
    let route = match LAST_EFFECTIVE_PLE_BASE_ROUTE.load(std::sync::atomic::Ordering::SeqCst) {
        1 => PleBaseRoute::ScalarBase,
        2 => PleBaseRoute::BlockRecursive,
        _ => return None,
    };
    Some((scalar_base_max_cols, route))
}

/// The selected arm of the [`FieldMatrix::ple`] panel dispatcher for one
/// column window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlePanelRoute {
    /// The carrier's SIMD panel kernel takes the whole column window: the
    /// carrier registers a [`crate::field::PlePanelLane`] and the window fits
    /// both the profile's panel base width and that lane's own width. The
    /// kernel may still decline at run time, in which case the window falls
    /// through to the binary-halving split.
    PanelBase,
    /// The column window is wider than the profile's panel base width and is
    /// walked in narrow sub-panels — each at most that width, capped at the
    /// resolved lane ceiling when an installed profile sets it lower — every
    /// sub-panel dispatched to the panel base and followed by a wide trsm +
    /// gemm update of the right tail.
    SubPanelRecursion,
    /// The column window is halved and driven by the recursive trsm + gemm
    /// split: the carrier registers no panel kernel, or the window lies
    /// between the resolved lane ceiling and the profile's panel base width
    /// (too wide for one panel dispatch, not wide enough for the sub-panel
    /// walk).
    RecursiveSplit,
}

/// Reports the [`FieldMatrix::ple`] panel arm for a carrier's lane class
/// ([`FiniteField::simd_ple_panel_lane`]) and a column-window width, against
/// the active profile's `ple.panel_base_max_cols()` and the lane's own width.
#[must_use]
pub fn ple_panel_route(lane: Option<PlePanelLane>, win: usize) -> PlePanelRoute {
    let tuning = tuning::active();
    let ple = tuning.ple();
    ple_panel_route_resolved(ple.panel_base_max_cols(), ple_lane_max_cols(ple, lane), win)
}

/// Reports the panel arm for a column-window width against already-resolved
/// widths.
fn ple_panel_route_resolved(
    panel_base_max_cols: usize,
    panel_lane_max_cols: Option<usize>,
    win: usize,
) -> PlePanelRoute {
    let Some(lane_max_cols) = panel_lane_max_cols else {
        return PlePanelRoute::RecursiveSplit;
    };
    if win > panel_base_max_cols {
        PlePanelRoute::SubPanelRecursion
    } else if win <= lane_max_cols {
        PlePanelRoute::PanelBase
    } else {
        PlePanelRoute::RecursiveSplit
    }
}

/// Recursive driver of [`ple_in_place`] over the column window
/// `[col_lo, col_hi)`; `swap_rows` also permutes the cells outside it.
/// Appends the window's pivot columns (absolute indices) to `pivot_cols` in
/// discovery order and returns their count.
///
/// L's multipliers live in the pivot columns themselves, which are
/// non-contiguous when the left half is rank-deficient, so `L1` and `L1_bot`
/// are read from `pivot_cols`, never from the prefix `[col_lo, col_lo + r1)`.
fn ple_in_place_window<F: FiniteField, O: ObservationPolicy>(
    mut a: MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
    widths: PleWidths,
) -> usize {
    let m = a.rows();
    let win = col_hi.saturating_sub(col_lo);
    debug_assert_eq!(perm.len(), m, "ple_in_place_window: perm length mismatch");
    if m == 0 || win == 0 {
        return 0;
    }

    if ple_base_route_resolved(widths.scalar_base_max_cols, win) == PleBaseRoute::ScalarBase {
        return ple_base_direct(&mut a, col_lo, col_hi, perm, pivot_cols);
    }

    match ple_panel_route_resolved(widths.panel_base_max_cols, widths.panel_lane_max_cols, win) {
        PlePanelRoute::SubPanelRecursion => {
            return ple_panel_recursive_window::<F, O>(a, col_lo, col_hi, perm, pivot_cols, widths);
        }
        PlePanelRoute::PanelBase => {
            if let Some(rank) =
                try_panel_base_dispatch::<F, O>(&mut a, col_lo, col_hi, perm, pivot_cols)
            {
                return rank;
            }
        }
        PlePanelRoute::RecursiveSplit => {}
    }

    let h = win / 2;
    let mid = col_lo + h;

    let pivot_cols_start = pivot_cols.len();
    let r1 = ple_in_place_window::<F, O>(a.reborrow(), col_lo, mid, perm, pivot_cols, widths);

    // Row-major storage admits no simultaneous mutable views over disjoint
    // column ranges, so the read-side operands `L1` and `L1_bot` are
    // materialised into owned buffers. `L1` carries an explicit unit
    // diagonal, which `trsm_lower` reads.
    if r1 > 0 && mid < col_hi {
        let left_pivots: &[usize] = &pivot_cols[pivot_cols_start..pivot_cols_start + r1];
        let l1 = materialise_l1_unit_at_cols(&a.as_view(), 0, left_pivots);
        trsm_lower_with_policy::<F, O>(l1.submat(.., ..), a.submat_mut(0..r1, mid..col_hi));

        // a[r1..m, mid..col_hi] -= L1_bot · a[0..r1, mid..col_hi].
        if r1 < m {
            let l1_bot = materialise_block_at_cols(&a.as_view(), r1, left_pivots, m - r1);
            let zero = a.get(0, col_lo).zero_like();
            let one = zero.one_like();
            let neg_one = zero - one.clone();
            let right = a.submat_mut(.., mid..col_hi);
            let (a3_mut, a4_mut) = right.split_rows_mut(r1);
            let a3_view = a3_mut.as_view();
            gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
                neg_one,
                &l1_bot.submat(.., ..),
                &a3_view,
                one,
                a4_mut,
            );
        }
    }

    // The recursive view keeps the full parent column range, so its row
    // swaps stay full-row.
    let r2 = if r1 < m && mid < col_hi {
        let (_top, a4) = a.split_rows_mut(r1);
        ple_in_place_window::<F, O>(a4, mid, col_hi, &mut perm[r1..], pivot_cols, widths)
    } else {
        0
    };

    r1 + r2
}

/// Left-looking sub-panel walk of the column window `[col_lo, col_hi)`, the
/// [`PlePanelRoute::SubPanelRecursion`] arm: a column-axis-only form of the
/// recursive PLUQ of `@/citation/DumasPernetSultan2017`.
///
/// Each sub-panel goes to the panel kernel on the rows below the running
/// rank; a trsm and a Schur-complement gemm then update the right tail. The
/// kernel reports absolute pivot columns and propagates its row swaps across
/// the full parent column range, left and right of the sub-panel.
fn ple_panel_recursive_window<F: FiniteField, O: ObservationPolicy>(
    mut a: MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
    widths: PleWidths,
) -> usize {
    let m = a.rows();
    let mut col_cur = col_lo;
    let mut rank_total = 0usize;

    while col_cur < col_hi && rank_total < m {
        // An installed profile may set the lane ceiling below the split
        // width; a sub-panel honours both.
        let sub_panel_cols = widths
            .panel_lane_max_cols
            .map_or(widths.panel_base_max_cols, |lane_max_cols| {
                lane_max_cols.min(widths.panel_base_max_cols)
            });
        let sub_hi = (col_cur + sub_panel_cols).min(col_hi);

        let pivot_cols_start = pivot_cols.len();

        // The kernel's row permutation indexes from row 0 of the view it is
        // given, so it receives the rows below `rank_total`.
        let r_i_opt = if rank_total == 0 {
            try_panel_base_dispatch::<F, O>(&mut a.reborrow(), col_cur, sub_hi, perm, pivot_cols)
        } else {
            let (_top, mut bot) = a.reborrow().split_rows_mut(rank_total);
            try_panel_base_dispatch::<F, O>(
                &mut bot,
                col_cur,
                sub_hi,
                &mut perm[rank_total..],
                pivot_cols,
            )
        };

        let r_i = match r_i_opt {
            Some(r) => r,
            None => {
                if rank_total == 0 {
                    ple_in_place_window_no_panel::<F, O>(
                        a.reborrow(),
                        col_cur,
                        sub_hi,
                        perm,
                        pivot_cols,
                        widths,
                    )
                } else {
                    let (_top, bot) = a.reborrow().split_rows_mut(rank_total);
                    ple_in_place_window_no_panel::<F, O>(
                        bot,
                        col_cur,
                        sub_hi,
                        &mut perm[rank_total..],
                        pivot_cols,
                        widths,
                    )
                }
            }
        };

        if r_i > 0 && sub_hi < col_hi {
            let new_pivots: Vec<usize> =
                pivot_cols[pivot_cols_start..pivot_cols_start + r_i].to_vec();

            let l1 = materialise_l1_unit_at_cols(&a.as_view(), rank_total, &new_pivots);
            trsm_lower_with_policy::<F, O>(
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
                gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
                    neg_one,
                    &l1_bot.submat(.., ..),
                    &a3_view,
                    one,
                    a4_mut,
                );
            }
        }

        rank_total += r_i;
        col_cur = sub_hi;
    }

    rank_total
}

/// [`ple_in_place_window`] without the panel arms, for a sub-panel whose
/// kernel dispatch declined; of `widths` it reads only
/// `scalar_base_max_cols`.
fn ple_in_place_window_no_panel<F: FiniteField, O: ObservationPolicy>(
    mut a: MatViewMut<'_, F>,
    col_lo: usize,
    col_hi: usize,
    perm: &mut [usize],
    pivot_cols: &mut Vec<usize>,
    widths: PleWidths,
) -> usize {
    let m = a.rows();
    let win = col_hi.saturating_sub(col_lo);
    if m == 0 || win == 0 {
        return 0;
    }
    if ple_base_route_resolved(widths.scalar_base_max_cols, win) == PleBaseRoute::ScalarBase {
        return ple_base_direct(&mut a, col_lo, col_hi, perm, pivot_cols);
    }

    let h = win / 2;
    let mid = col_lo + h;
    let pivot_cols_start = pivot_cols.len();
    let r1 =
        ple_in_place_window_no_panel::<F, O>(a.reborrow(), col_lo, mid, perm, pivot_cols, widths);

    if r1 > 0 && mid < col_hi {
        let left_pivots: &[usize] = &pivot_cols[pivot_cols_start..pivot_cols_start + r1];
        let l1 = materialise_l1_unit_at_cols(&a.as_view(), 0, left_pivots);
        trsm_lower_with_policy::<F, O>(l1.submat(.., ..), a.submat_mut(0..r1, mid..col_hi));
        if r1 < m {
            let l1_bot = materialise_block_at_cols(&a.as_view(), r1, left_pivots, m - r1);
            let zero = a.get(0, col_lo).zero_like();
            let one = zero.one_like();
            let neg_one = zero - one.clone();
            let right = a.submat_mut(.., mid..col_hi);
            let (a3_mut, a4_mut) = right.split_rows_mut(r1);
            let a3_view = a3_mut.as_view();
            gemm_axpy_into_view_tiled::<F, GEMM_ROW_TILE, GEMM_COL_TILE, O>(
                neg_one,
                &l1_bot.submat(.., ..),
                &a3_view,
                one,
                a4_mut,
            );
        }
    }

    let r2 = if r1 < m && mid < col_hi {
        let (_top, a4) = a.split_rows_mut(r1);
        ple_in_place_window_no_panel::<F, O>(a4, mid, col_hi, &mut perm[r1..], pivot_cols, widths)
    } else {
        0
    };
    r1 + r2
}

/// Materialises the `r1 × r1` unit-lower-triangular `L1`: entry `(i, j)` for
/// `j < i` is `a[row_off + i, pivot_cols[j]]` and the diagonal is `1`.
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
    }
    l1
}

/// Materialises the `rows × r1` block `L1_bot`: entry `(i, j)` is
/// `a[row_off + i, pivot_cols[j]]`.
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

/// Splits the compact storage of `working` into `L` (`m × rank`) and `E`
/// (`rank × n`).
///
/// Compact storage: for pivot index `k` with pivot column `pivot_cols[k]`,
/// `working[k, pivot_cols[k]..n]` is row `k` of `E` and
/// `working[i, pivot_cols[k]]` for `i > k` is `L[i, k]`. `L`'s unit diagonal
/// is synthesised here; the cells of row `k` left of its pivot column hold
/// earlier multipliers or zero and are excluded from `E`.
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

    let mut e = FieldMatrix::new(rank, n, zero.clone());
    for (i, &pci) in pivot_cols.iter().enumerate().take(rank) {
        for j in pci..n {
            e.set(i, j, working.get(i, j));
        }
    }
    let mut l = FieldMatrix::new(m, rank, zero);
    for k in 0..rank {
        l.set(k, k, one.clone());
    }
    for (j, &pcj) in pivot_cols.iter().enumerate().take(rank) {
        for i in (j + 1)..m {
            l.set(i, j, working.get(i, pcj));
        }
    }
    (l, e)
}

impl<F: FiniteField> FieldMatrix<F> {
    /// Computes the PLE decomposition `P · L · E = self`
    /// (`@/citation/DumasPernet2012` §2.2, alg. 2.5).
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
    /// # Complexity
    ///
    /// `O(m · n · min(m, n))` field operations.
    pub fn ple(&self) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize) {
        self.ple_with_policy::<RecordObservations>()
    }

    /// [`ple`](Self::ple) without the test-support route, panel and GEMM
    /// observation writes; the computation is identical.
    #[cfg(any(test, feature = "test-support"))]
    pub fn ple_quiet_for_test(&self) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize) {
        self.ple_with_policy::<crate::field::matrix::QuietObservations>()
    }

    pub(crate) fn ple_with_policy<O: ObservationPolicy>(
        &self,
    ) -> (Permutation, FieldMatrix<F>, FieldMatrix<F>, usize) {
        let (m, n) = self.shape();
        if m == 0 || n == 0 {
            let l = zero_matrix_like(m, 0, self);
            let e = zero_matrix_like(0, n, self);
            return (Permutation::identity(m), l, e, 0);
        }
        let mut working = self.clone();
        let mut perm: Vec<usize> = (0..m).collect();
        let max_rank = m.min(n);
        let mut pivot_cols: Vec<usize> = Vec::with_capacity(max_rank);
        let rank = ple_in_place::<F, O>(working.submat_mut(.., ..), &mut perm, &mut pivot_cols);
        let (l, e) = split_compact(&working, rank, &pivot_cols);
        // The recursion's `perm` is the destination → source map: applying
        // it to `self` row-wise gives the matrix that decomposes as L · E.
        // The contract `P · L · E = self` requires the inverse permutation.
        let inverse_perm = invert_perm(&perm);
        (Permutation::from_indices(inverse_perm), l, e, rank)
    }

    /// Row-echelon form with its transform: returns `(X, E)` with
    /// `X · self = E` and `E` an `m × n` row-echelon matrix
    /// (`@/citation/DumasPernet2012` §2.2, alg. 2.6).
    ///
    /// `X = L_full⁻¹ · Pᵀ` is `m × m` and non-singular, where `L_full` is the
    /// PLE factor `L` padded to `m × m` unit lower-triangular.
    ///
    /// # Panics
    ///
    /// Panics on an `m × 0` input with `m > 0` over a field whose
    /// `zero_hint` is `None`.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n) + m³)` field operations.
    pub fn row_echelon(&self) -> (FieldMatrix<F>, FieldMatrix<F>) {
        let (m, n) = self.shape();
        if m == 0 {
            let x = zero_matrix_like(0, 0, self);
            let e = zero_matrix_like(0, n, self);
            return (x, e);
        }
        if n == 0 {
            // An `m × 0` matrix has no cell to take a zero from, so the
            // identity `X` needs `F::zero_hint()`, which is `None` for
            // runtime-context fields.
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
        let l_full = pad_l_to_full(&l, m, self);

        // Pᵀ has 1 at (perm[i], i).
        let zero = self.get(0, 0).zero_like();
        let one = zero.one_like();
        let mut p_t = FieldMatrix::new(m, m, zero.clone());
        for (i, &src) in p.indices().iter().enumerate() {
            p_t.set(src, i, one.clone());
        }
        // Solve L_full · X = Pᵀ in place; result lands in p_t.
        trsm_lower(l_full.submat(.., ..), p_t.submat_mut(.., ..));

        let r = e.rows();
        let mut e_full = FieldMatrix::new(m, n, zero);
        for i in 0..r {
            for j in 0..n {
                e_full.set(i, j, e.get(i, j));
            }
        }
        (p_t, e_full)
    }

    /// Reduced row-echelon form with its transform: returns `(X, R)` with
    /// `X · self = R` and `R` in RREF (`@/citation/DumasPernet2012` §2.2,
    /// alg. 2.7).
    ///
    /// # Panics
    ///
    /// Panics where [`row_echelon`](Self::row_echelon) does.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n) + m³)` field operations.
    pub fn rref(&self) -> (FieldMatrix<F>, FieldMatrix<F>) {
        let (m, n) = self.shape();
        if m == 0 || n == 0 {
            return self.row_echelon();
        }
        let (mut x, mut e) = self.row_echelon();
        let zero = self.get(0, 0).zero_like();
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

        if try_blocked_back_sub(&mut x, &mut e, &pivots, m, n) {
            return (x, e);
        }

        for &(pi, pc) in &pivots {
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
            // Rows below `pi` are already zero in column `pc` by the echelon
            // form; the zero-factor test skips them.
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

    /// Rank of `self`, computed by [`ple`](Self::ple).
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n))` field operations.
    pub fn rank(&self) -> usize {
        self.ple().3
    }

    /// Returns a basis of the right null-space `{ v ∈ Fⁿ : self · v = 0 }`:
    /// `n − rank(self)` vectors of length `n`, one per free column `f` of the
    /// RREF `R`, with `v[f] = 1` and `v[pivot_cols[k]] = −R[k, f]`.
    ///
    /// # Panics
    ///
    /// Panics on a `0 × n` input with `n > 0` over a field whose `zero_hint`
    /// is `None`.
    ///
    /// # Complexity
    ///
    /// That of [`rref`](Self::rref).
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

    /// LU decomposition of a full-rank matrix: `Some((P, L, U))` with
    /// `P · self = L · U` if and only if `rank(self) == min(m, n)`.
    ///
    /// `L` (`m × r`, unit lower-trapezoidal) and `U = E` (`r × n`) are the
    /// [`ple`](Self::ple) factors.
    ///
    /// # Complexity
    ///
    /// `O(m · n · min(m, n))` field operations.
    pub fn lu(&self) -> Option<(Permutation, FieldMatrix<F>, FieldMatrix<F>)> {
        let (m, n) = self.shape();
        let (p_ple, l, e, r) = self.ple();
        if r != m.min(n) {
            return None;
        }
        // PLE gives `A = P_ple · L · E`; LU returns `P_lu · A = L · U`, so
        // `P_lu = P_ple⁻¹`.
        Some((p_ple.inverse(), l, e))
    }
}

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

/// Conservative default for `ple.blocked_back_sub_min_dim()`: the `max(m, n)`
/// below which `try_blocked_back_sub` returns `false` and `rref` runs its
/// scalar loop.
pub(crate) const BLOCKED_BACK_SUB_MIN_DIM: usize = 128;

/// The selected arm of [`FieldMatrix::rref`]'s back-substitution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackSubRoute {
    /// Eliminate the pivot columns with the row-by-row scalar loop.
    Scalar,
    /// Eliminate the pivot columns with the panelized trsm + gemm stages.
    Blocked,
}

/// Reports the [`FieldMatrix::rref`] back-substitution arm for a matrix
/// shape.
///
/// The comparison uses the active `ple.blocked_back_sub_min_dim()` profile
/// value against `max(m, n)`: a shape below it takes the scalar loop. A
/// pivot-free matrix is handled by the dispatcher before this selector is
/// called.
#[must_use]
pub fn back_sub_route(m: usize, n: usize) -> BackSubRoute {
    back_sub_route_resolved(tuning::active().ple().blocked_back_sub_min_dim(), m, n)
}

/// Reports the back-substitution arm for a matrix shape against an
/// already-resolved `blocked_back_sub_min_dim`.
fn back_sub_route_resolved(blocked_back_sub_min_dim: usize, m: usize, n: usize) -> BackSubRoute {
    if m.max(n) < blocked_back_sub_min_dim {
        BackSubRoute::Scalar
    } else {
        BackSubRoute::Blocked
    }
}

/// Blocked back-substitution for [`FieldMatrix::rref`] on the echelon pair
/// `(x, e)` with the `(pivot_row, pivot_col)` list `pivots`.
///
/// Scales the pivot rows to unit pivots, applies `trsm_upper` with the
/// pivot-column block to `E[pivot rows, free cols]` and `X[pivot rows, *]`,
/// subtracts `E[non-pivot rows, pivot cols]` times those blocks from the
/// non-pivot rows, and sets the pivot columns of `e` to identity columns.
///
/// Returns `false` without touching `x` or `e` when [`back_sub_route`]
/// reports [`BackSubRoute::Scalar`] for `(m, n)`; otherwise returns `true`.
pub(crate) fn try_blocked_back_sub<F: FiniteField>(
    x: &mut FieldMatrix<F>,
    e: &mut FieldMatrix<F>,
    pivots: &[(usize, usize)],
    m: usize,
    n: usize,
) -> bool {
    let r = pivots.len();
    if r == 0 {
        return true;
    }

    if back_sub_route(m, n) == BackSubRoute::Scalar {
        return false;
    }

    let zero = e.get(0, 0).zero_like();
    let one = zero.one_like();
    let neg_one = zero.clone() - one.clone();

    let pivot_rows: Vec<usize> = pivots.iter().map(|&(pi, _)| pi).collect();
    let pivot_cols: Vec<usize> = pivots.iter().map(|&(_, pc)| pc).collect();

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

    // After scaling, the pivot-column block is upper unit-triangular.
    // `trsm_upper` leaves it unmodified, so one extraction serves both
    // solves.
    let e_piv_piv = {
        let mut m_pp = FieldMatrix::new(r, r, zero.clone());
        for (ki, &pi) in pivot_rows.iter().enumerate() {
            for (kj, &pc) in pivot_cols.iter().enumerate() {
                m_pp.set(ki, kj, e.get(pi, pc));
            }
        }
        m_pp
    };

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

    // For `r == 1` the 1×1 unit block makes the solve the identity.
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

    if n_nonpiv > 0 {
        let e_nonpiv_piv = {
            let mut m_np = FieldMatrix::new(n_nonpiv, r, zero.clone());
            for (ni, &npi) in non_pivot_rows.iter().enumerate() {
                for (kj, &pc) in pivot_cols.iter().enumerate() {
                    m_np.set(ni, kj, e.get(npi, pc));
                }
            }
            m_np
        };

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

        {
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

    for &(pi, pc) in pivots {
        for k in 0..m {
            e.set(k, pc, zero.clone());
        }
        e.set(pi, pc, one.clone());
    }

    true
}

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

    /// GF(2^8) with the AES reduction polynomial (`@/citation/Nist2001`).
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

    #[test]
    fn test_rref_canonical_15x17_gf7_seed1_structural_correctness() {
        let a = dense_random_fp_seeded::<7>(15, 17, 0.05, 1);
        check_canonical_rref_fp(&a);
        let (_x, got) = a.rref();
        assert_eq!(
            a.rank(),
            pivot_cols_of_rref(&got).len(),
            "rank(15x17 GF(7)/seed=1) must match canonical pivot count"
        );
    }

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
            check_canonical_rref_fp(&a);
            let (_x, got) = a.rref();
            let got_pivots = pivot_cols_of_rref(&got);
            assert_eq!(
                got_pivots, expected_pivots,
                "canonical pivot regression: seed={seed:#x} rows={rows} cols={cols} \
                 density={density}: got {got_pivots:?}, expected {expected_pivots:?}",
            );
        }
    }

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

    // Rank-deficient inputs `A = F · G` with `F` rows×rank, `G` rank×cols and
    // `rank = min(rows, cols) − 1`.
    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(128))]

        #[test]
        fn proptest_field_matrix_rref_canonical_rank_deficient_jit_bd9c6e13(
            rows in 4usize..=16,
            cols in 4usize..=16,
            seed in proptest::prelude::any::<u64>(),
        ) {
            let rank = rows.min(cols) - 1;
            let f7 = random_fp::<7>(rows, rank, seed);
            let g7 = random_fp::<7>(rank, cols, seed.wrapping_add(1));
            let a7 = gemm(&f7, &g7);
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

    /// The 4×4 cyclic-shift matrix
    ///
    ///   [ 0 0 0 1 ]
    ///   [ 1 0 0 0 ]
    ///   [ 0 1 0 0 ]
    ///   [ 0 0 1 0 ]
    ///
    /// has rank 4 and forces a non-involutive row permutation, for which
    /// the PLE permutation and its inverse differ.
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

    #[test]
    fn test_rank_matches_independent_construction_fp_m31() {
        type F = Fp<MERSENNE_31>;
        let m = 8;
        let n = 6;
        for &target_rank in &[1usize, 2, 3, 4, 5] {
            let mut a = FieldMatrix::<F>::zeros(m, n);
            for i in 0..target_rank {
                // ∑ e_i ⊗ e_i has an r × r identity submatrix, hence rank r.
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

        // a = u1 ⊗ v1 + u2 ⊗ v2 with u1, u2 and v1, v2 linearly independent.
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
    fn test_zero_width_edge_does_not_panic() {
        let a = FieldMatrix::<Fp<MERSENNE_31>>::zeros(4, 0);
        let (_p, l, e, r) = a.ple();
        assert_eq!(r, 0);
        assert_eq!(l.shape(), (4, 0));
        assert_eq!(e.shape(), (0, 0));
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
        let (_x2, e2) = a.rref();
        assert_eq!(e2.shape(), (4, 0));
        // rank == 0 == min(4, 0).
        let lu = a.lu();
        assert!(lu.is_some(), "lu(m × 0) must return Some (rank == min)");
        let ns = a.nullspace();
        assert_eq!(ns.len(), 0);
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

    // `fieldmatrix_new_count` readings for one call with no tuning profile
    // installed.
    const EXPECTED_PLE_N4: u64 = 14;
    const EXPECTED_PLE_N64: u64 = 264;
    const EXPECTED_PLE_N1024: u64 = 4736;
    const EXPECTED_ROW_ECHELON_N64: u64 = 280;
    // Below `BLOCKED_BACK_SUB_MIN_DIM`, `rref` runs the scalar loop and
    // allocates what `row_echelon` does.
    const EXPECTED_RREF_N64: u64 = 280;
    const EXPECTED_LU_N64: u64 = 264;

    // Word boundaries (0, 1, 63, 64, 65) and the 16-byte AVX2 boundary
    // (15, 16, 17).
    const PANEL_BOUNDARY_LENS: &[usize] = &[0, 1, 15, 16, 17, 63, 64, 65];

    /// Prints the median of five `FieldMatrix::ple` timings on a `256 × 256`
    /// GF(251) matrix; asserts nothing.
    #[test]
    #[ignore = "slow: wall-time probe for panelized PLE; informational only"]
    fn test_ple_panelized_wall_time_probe_gf251_256_uniform() {
        let n = 256;
        let a = random_fp::<251>(n, n, 0xC3FAu64);
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

    /// Median wall time in ns of five `ple` calls after three warm-ups; emits
    /// one CSV row per trial to stderr.
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

    /// Median wall time in ns of five `row_echelon` calls after three
    /// warm-ups; emits one CSV row per trial to stderr.
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

    /// `rref` with scalar back-substitution only: the reference the blocked
    /// path is timed against.
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

    /// Times scalar (`rref_A`) and blocked (`rref_B`) back-substitution on
    /// one matrix; returns `(median_a_ns, median_b_ns)` and emits one CSV
    /// line per trial to stderr.
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

    /// Paired `rref` timing, scalar against blocked back-substitution, as CSV
    /// on stderr. Runs only under `GF2_BENCH=1`.
    #[test]
    #[ignore = "bench: rref SC#5 non-regression paired 10-trial sweep (~30 s); run on a quiesced host"]
    fn test_rref_non_regression_wall_time() {
        if !matches!(std::env::var("GF2_BENCH"), Ok(ref v) if v != "0") {
            eprintln!(
                "SKIP test_rref_non_regression_wall_time: wall-clock sweep — \
                 set GF2_BENCH=1 on a quiesced host to run it"
            );
            return;
        }

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
        // This binary installs no profile, so the widths are the conservative
        // defaults; route observation goes through the reporter the dispatcher
        // itself calls.
        let active_tuning = tuning::active();
        let ple = active_tuning.ple();
        assert!(ple.panel_byte_lane_max_cols() >= ple.panel_base_max_cols());
        assert!(ple.panel_u16_lane_max_cols() >= ple.panel_base_max_cols());
        for lane in [PlePanelLane::Byte, PlePanelLane::U16] {
            assert_eq!(
                ple_panel_route(Some(lane), ple.panel_base_max_cols()),
                PlePanelRoute::PanelBase
            );
            assert_eq!(
                ple_panel_route(Some(lane), ple.panel_base_max_cols() + 1),
                PlePanelRoute::SubPanelRecursion
            );
        }
        for w in [
            1,
            ple.panel_base_max_cols(),
            ple.panel_base_max_cols() + 1,
            usize::MAX,
        ] {
            assert_eq!(ple_panel_route(None, w), PlePanelRoute::RecursiveSplit);
        }
        // Per-lane width boundaries are covered by the installed-profile
        // binaries `tests/tuning_profile_ple_*_install*.rs`, because the
        // conservative panel base width is narrower than either lane width.

        // On an AVX2 host with the `simd` feature the lane is `Byte` for
        // P <= 251 and `U16` for 252 <= P < 65536.
        #[cfg(feature = "simd")]
        {
            if std::arch::is_x86_feature_detected!("avx2") {
                assert_eq!(
                    <Fp<7> as FiniteField>::simd_ple_panel_lane(),
                    Some(PlePanelLane::Byte)
                );
                assert_eq!(
                    <Fp<31> as FiniteField>::simd_ple_panel_lane(),
                    Some(PlePanelLane::Byte)
                );
                assert_eq!(
                    <Fp<127> as FiniteField>::simd_ple_panel_lane(),
                    Some(PlePanelLane::Byte)
                );
                assert_eq!(
                    <Fp<241> as FiniteField>::simd_ple_panel_lane(),
                    Some(PlePanelLane::Byte)
                );
                assert_eq!(
                    <Fp<251> as FiniteField>::simd_ple_panel_lane(),
                    Some(PlePanelLane::Byte)
                );
                assert_eq!(
                    <Fp<65521> as FiniteField>::simd_ple_panel_lane(),
                    Some(PlePanelLane::U16)
                );
            }
        }
        #[cfg(not(feature = "simd"))]
        {
            assert_eq!(<Fp<7> as FiniteField>::simd_ple_panel_lane(), None);
            assert_eq!(<Fp<251> as FiniteField>::simd_ple_panel_lane(), None);
            assert_eq!(<Fp<65521> as FiniteField>::simd_ple_panel_lane(), None);
        }
        assert_eq!(
            <Fp<MERSENNE_31> as FiniteField>::simd_ple_panel_lane(),
            None
        );
    }

    /// An `m × n` matrix of rank at most `rank`, the product `F · G`.
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

    /// `FieldMatrix::ple` through `ple_in_place_window_no_panel`, so the SIMD
    /// panel kernel never runs.
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
        let rank = ple_in_place_window_no_panel::<F, RecordObservations>(
            working.submat_mut(.., ..),
            0,
            n,
            &mut perm,
            &mut pivot_cols,
            PleWidths::resolve::<F>(),
        );
        let (l, e) = split_compact(&working, rank, &pivot_cols);
        let inverse_perm = invert_perm(&perm);
        (Permutation::from_indices(inverse_perm), l, e, rank)
    }

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

    /// `rref` through the scalar back-substitution loop, bypassing
    /// `try_blocked_back_sub`.
    fn rref_scalar_oracle<F: FiniteField>(a: &FieldMatrix<F>) -> (FieldMatrix<F>, FieldMatrix<F>) {
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

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config { cases: 8, .. proptest::test_runner::Config::default() })]

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

    #[test]
    fn test_rref_128x128_fp7_blocked_back_sub() {
        let a = random_fp::<7>(128, 128, 0xBB01);
        let (x_blocked, r_blocked) = a.rref();
        let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
        assert_eq!(r_blocked, r_scalar, "RREF mismatch 128×128 Fp<7>");
        assert_eq!(x_blocked, x_scalar, "transform mismatch 128×128 Fp<7>");
    }

    #[test]
    fn test_rref_128x64_fp7_blocked_back_sub() {
        // max(m, n) = 128 >= threshold; n = 64 < threshold.
        let a = random_fp::<7>(128, 64, 0xBB02);
        let (x_blocked, r_blocked) = a.rref();
        let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
        assert_eq!(r_blocked, r_scalar);
        assert_eq!(x_blocked, x_scalar);
    }

    #[test]
    fn test_rref_64x128_fp7_blocked_back_sub() {
        // max(m, n) = 128 >= threshold; m = 64 < threshold.
        let a = random_fp::<7>(64, 128, 0xBB03);
        let (x_blocked, r_blocked) = a.rref();
        let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
        assert_eq!(r_blocked, r_scalar);
        assert_eq!(x_blocked, x_scalar);
    }

    #[test]
    fn test_rref_128x128_rank_deficient_fp7_blocked_back_sub() {
        // Rank 64, so some pivot columns are absent.
        let a = random_fp_rank_deficient::<7>(128, 128, 64, 0xBB04);
        let (x_blocked, r_blocked) = a.rref();
        let (x_scalar, r_scalar) = rref_scalar_oracle(&a);
        assert_eq!(r_blocked, r_scalar);
        assert_eq!(x_blocked, x_scalar);
    }

    #[test]
    fn test_ple_panel_route_boundary_is_the_conservative_panel_width() {
        // The byte lane's conservative width is wider than the conservative
        // panel base width, so the panel base width is the boundary that binds.
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::Byte), PLE_PANEL_RECURSIVE_BASE),
            PlePanelRoute::PanelBase
        );
        assert_eq!(
            ple_panel_route(Some(PlePanelLane::Byte), PLE_PANEL_RECURSIVE_BASE + 1),
            PlePanelRoute::SubPanelRecursion
        );
    }

    #[test]
    fn test_ple_base_route_boundary_is_the_conservative_scalar_base() {
        assert_eq!(
            ple_base_route(PLE_SCALAR_BASE_MAX_COLS_DEFAULT),
            PleBaseRoute::ScalarBase
        );
        assert_eq!(
            ple_base_route(PLE_SCALAR_BASE_MAX_COLS_DEFAULT + 1),
            PleBaseRoute::BlockRecursive
        );
    }

    #[test]
    fn test_back_sub_route_boundary_is_the_conservative_dimension() {
        assert_eq!(
            back_sub_route(BLOCKED_BACK_SUB_MIN_DIM - 1, BLOCKED_BACK_SUB_MIN_DIM - 1),
            BackSubRoute::Scalar
        );
        // `max(m, n)` carries the comparison, so either dimension alone
        // reaching the threshold selects the blocked arm.
        assert_eq!(
            back_sub_route(BLOCKED_BACK_SUB_MIN_DIM, 1),
            BackSubRoute::Blocked
        );
        assert_eq!(
            back_sub_route(1, BLOCKED_BACK_SUB_MIN_DIM),
            BackSubRoute::Blocked
        );
    }
}
