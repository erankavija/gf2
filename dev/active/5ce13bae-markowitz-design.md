# Markowitz-degree pivot selection for sparse RREF — design note

| Field | Value |
|---|---|
| JIT issue | `5ce13bae` |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Predecessor | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` § 4 #5 (uniform 0.38x-0.51x of LinBox `GaussDomain::NoReordering`) |
| Targets | `SpBitMatrix::rref` (`crates/gf2-core/src/sparse.rs`), `SparseFieldMatrix::rref` (`crates/gf2-core/src/field/sparse_matrix.rs`) |

## Problem statement

gf2-core's current sparse RREF is a straight-line column-sweep: for each column `col = 0..n`, pick the first un-used row whose first stored entry equals `col`. No pivot-priority / fill-control. The result is a 1.97x-2.61x cross-ratio vs LinBox `GaussDomain::NoReordering` (which uses Markowitz-product pivot priority + early-out on dependent rows) across the 10 sparse-elim cells in the scorecard.

## Algorithm — column-restricted Markowitz pivot selection

For each elimination step we select the **(row, col)** pair that minimises the **Markowitz product**

```
markowitz(r, c) = (row_nnz(r) - 1) * (col_nnz(c) - 1)
```

**subject to the constraint that `col` is the smallest column index that is still the leading entry of some un-used row.** The intuition: eliminating with a row of `row_nnz` non-zeros and a column with `col_nnz` non-zero rows creates **at most** `(row_nnz - 1) * (col_nnz - 1)` new fill-ins (the structural upper bound on the size of the resulting Schur-complement block). Picking the (row, col) that minimises this bound minimises fill.

The **canonical-RREF constraint** is critical: the set of pivot columns of an RREF is uniquely determined (the leftmost columns that are linearly independent), so to preserve byte-equality with the dense reference we must walk the pivot columns in ascending order. Pure-Markowitz (LinBox-style, where any (row, col) with the smallest product may be selected) produces a valid REF but not the *canonical* RREF, breaking the existing test suite.

Once we restrict to the smallest leading column among un-used rows, `col_nnz[pc]` is identical for all candidate rows at column `pc`, so the Markowitz product collapses to "minimise `row_nnz - 1`" — equivalently, pick the un-used row with the **smallest row_nnz** whose leading entry is `pc`. We therefore do not need to maintain `col_nnz` explicitly; only `row_nnz` is required.

LinBox's `GaussDomain::NoReordering` also adds an **early-out on dependent rows**: a row whose row_nnz drops to zero (eliminated entirely) does not need to participate in subsequent pivot selection. This is a natural consequence of the row_nnz incremental update — we just skip rows with `row_nnz == 0` (equivalently, `rows[i].is_empty()`).

### Pseudocode

```
state:
  rows[i]: sorted Vec of column indices (GF(2)) or (col, val) pairs (GF(p))
  row_nnz[i] = rows[i].len()                       # row population
  row_used[i] = false                              # was row i picked as pivot
  pivots: Vec<(orig_row, pivot_col)> = []          # pick order

repeat until no eligible pivot:
  # Stage 1: find the smallest column that is still some un-used row's
  # leading entry. This is the canonical pivot column.
  pc = +infinity
  for i in 0..m where !row_used[i] and !rows[i].is_empty():
    c = rows[i][0]    # the leading column of row i
    if c < pc: pc = c
  if pc == +infinity: break

  # Stage 2: among un-used rows whose leading entry == pc, pick the row
  # with the smallest row_nnz (Markowitz-equivalent: col_nnz[pc] is
  # constant across these candidates so the product collapses to
  # row_nnz - 1).
  pi = argmin_{i : !row_used[i], rows[i][0] == pc} row_nnz[i]

  # Scale pivot row (GF(p) only — GF(2) leading entry is 1 by definition)
  if F is not GF(2):
    inv = pivot_val(rows[pi], pc).inv()
    scale_row(rows[pi], inv)    # support preserved; row_nnz unchanged

  row_used[pi] = true
  pivots.push((pi, pc))

  # Eliminate column pc from every other row that has a non-zero in pc
  for k in 0..m where k != pi:
    if !rows[k].contains(pc): continue
    factor = rows[k][pc_pos] (GF(p)) or _ (GF(2))
    rows[k] ← axpy(rows[k], rows[pi], factor)
    row_nnz[k] = rows[k].len()

# Sort pivots by pivot_col ascending (canonical RREF row order); flatten
# back to CSR. Un-pivoted rows go below as zero rows.
```

### Pivot column choice (key subtlety)

The pseudocode picks `c = rows[i][0]` as the candidate pivot column for row `i`. **Why is this OK?** In RREF, every column with a pivot must end up "1 on one row, 0 elsewhere." The set of pivot columns we end up using is a subset of `{0..n}`. For a Markowitz strategy, in principle we could pick ANY non-zero column in row `i`, not just the leftmost. But the leftmost has two structural advantages:

1. **It IS a candidate column** — column `c` cannot already have been used as a pivot (already-used columns have zeros in all un-pivoted rows, by the elimination invariant).
2. **It produces a canonical pivot-column order** — pivot columns end up sorted ascending in the output, which is the RREF canonical form expected by all existing tests (which compare against the dense `crate::alg::rref::rref` reference).

Concretely: after `rows[pi]` is picked and `pc = rows[pi][0]`, we eliminate `pc` from all other rows. The next un-pivoted row `i'` still has `rows[i'][0] >= pc + 1` because any entries `<= pc` would have been eliminated. So the next iteration's candidate columns are all `> pc`. By induction the pivot columns end up strictly ascending.

This is the same property the existing straight-line algorithm relies on. The Markowitz product just changes WHICH un-pivoted row we choose at each step — not the set of pivot columns or their order.

### Complexity

- **Pivot search**: O(m) per step (scan un-used rows for the minimum Markowitz product).
- **Elimination**: identical to the existing straight-line algorithm — for each of the m candidate rows containing column `pc`, the merge takes O(row_nnz_max) field operations.
- **col_nnz update**: each elimination changes at most `O(row_nnz[pi] + row_nnz[k])` columns; total work across all elims is bounded by the total nnz processed during axpy, which is the same big-O as the elimination itself.
- **Overall**: same big-O as the existing straight-line algorithm: O(m * (m + nnz_per_step)) ≤ O(m^2 * d_max) where `d_max` is the maximum row-degree encountered. The **constant factor** wins because (a) total fill-in is structurally lower, (b) the early-out on `row_nnz == 0` skips dependent rows in subsequent steps, exactly mirroring LinBox's strategy.

### Incremental nnz maintenance — must NOT re-scan

The `row_nnz` array MUST be maintained incrementally during axpy. After each XOR / sparse-axpy on row `k`, set `row_nnz[k] = new_row_k.len()` (or update by `Δ = new_len − old_len`). Re-scanning the matrix to recompute `row_nnz` at each step would destroy the speedup (O(m · nnz) extra work) and recreate the asymptotic constant of the straight-line algorithm.

**No `col_nnz` is maintained.** As recorded in the Algorithm section above, the canonical-RREF constraint pins the pivot column set to the leftmost linearly-independent columns; walking pivot columns in ascending order means the only un-used rows that can contain entries at pivot column `pc` are those whose leading column equals `pc` (others have entries only at columns `> pc` by the sorted-list invariant). At a fixed `pc`, `col_nnz[pc]` is therefore identical across all candidate rows, and the Markowitz product `(row_nnz - 1) * (col_nnz - 1)` collapses to "minimise `row_nnz`". Materialising `col_nnz` would be dead work.

### Correctness vs the dense reference

The RREF of a matrix is unique once we require the canonical form:
1. Leading entry of each non-zero row is 1.
2. Each pivot column has zeros above and below the pivot.
3. Pivot columns appear in strictly ascending order top-to-bottom.
4. All-zero rows are at the bottom.

The Markowitz selection changes only the internal order in which pivots are chosen; the final canonical RREF is the same matrix the dense reference produces. The existing `dense_rref_reference` cross-check tests therefore continue to PASS verbatim.

## Integration with existing layout

### GF(2) (`SpBitMatrix::rref`)

The existing code already materialises each row as a `Vec<usize>` of column indices. We add:
- `row_nnz: Vec<usize>` — initially `rows[i].len()`.

The pivot-search loop walks pivot columns `pc` in ascending order; within each `pc` it does an `argmin` over un-used rows of `row_nnz[i]` (restricted to rows whose leading column equals `pc`). After elimination, set `row_nnz[k] = new_row_k.len()` for each modified row. `col_nnz` is not materialised — see "Incremental nnz maintenance" above for the collapse argument.

### GF(p) (`SparseFieldMatrix::rref`)

Identical pattern, except rows are `Vec<(usize, F)>` and the column-index comparison is on the `usize` component only. The pivot row is scaled to a leading `F::one()` before elimination so each axpy is `target := target − target[pc] * pivot_row`.

## Test plan

1. **Existing tests must continue to PASS** — they check byte-equality against the dense reference, which is the canonical RREF. Markowitz produces the same canonical form.
2. **New proptests** — `proptest_rref_markowitz_byte_equality_*` over a range of shapes (0x0, 1x1, square sparse, tall+wide, word-boundary 64/65) confirms the dense-reference equality across the entire random parameter space.
3. **Benchmark** — re-measure the 10 sparse-elim cells (5 fields x {n=256, n=1024}) under the existing `bench_sparse_csv_emitter` machinery, compare wall-time ratios to LinBox baselines from `2026-05-04-47698404-sparse-reference.csv`.

## References

- `2026-05-04-47698404-sparse-scorecard.md` § 4 #5: the 0.38x-0.51x gap that motivates this issue.
- LinBox `linbox/algorithms/gauss/gauss-nullspace.inl` — `GaussDomain::NoReordering` source (reference algorithm).
- Wikipedia: [Markowitz pivot](https://en.wikipedia.org/wiki/Sparse_matrix#Pivoting); Davis, *Direct Methods for Sparse Linear Systems* (2006) §6.3.
