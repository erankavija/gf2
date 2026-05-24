# bd9c6e13 — Canonical RREF Fix in `FieldMatrix::rref` (PLE Path)

Design note for `crates/gf2-core/src/field/ple.rs` PLE-recursion fix.

## Root cause

The block-recursive PLE driver (`ple_in_place_window`) splits each column
window in half. After processing the left half and finding `r1` pivots, it
builds two scratch buffers from the working matrix:

- `L1` — the `r1 × r1` unit-lower-triangular leading L-factor.
- `L1_bot` — the `(m − r1) × r1` strict-lower-trapezoidal L-factor below
  the pivot rows.

Both feed the inter-block trsm + Schur-complement update on the right
half.

Pre-fix code:

```rust
let l1     = materialise_l1_unit(&a.as_view(), 0, col_lo, r1);
let l1_bot = materialise_block(&a.as_view(), r1, col_lo, m - r1, r1);
```

The `materialise_*` helpers read column `j` from `a[*, col_lo + j]`. This
is correct **only if the left half's pivots happen to land in the
contiguous prefix** `col_lo, col_lo + 1, …, col_lo + r1 − 1`. The PLE
storage convention is that L's `k`-th column multipliers live at the
absolute pivot column `pivot_cols[k]` (not at index `k`). When the left
half is **rank-deficient** (one or more columns in `[col_lo, mid)` had no
pivot), `pivot_cols[k]` exceeds `col_lo + k` for some `k`, and the
contiguous read returns pre-Schur-eliminated zeros (or, for some shapes,
the wrong pivot's multipliers from a different column).

The corrupted `L1` and `L1_bot` then feed `trsm_lower` and
`gemm_axpy_into_view`, which silently produce a *non-equivalent* Schur
complement on the right half. The downstream recursion finds fewer
pivots than canonical (an actual rank under-count) or finds the
right rank but with non-canonical column choices.

## Discovery

The bug was discovered during 5ce13bae Markowitz-sparse test
development; the issue description cites a 15×17 GF(7), density 0.05,
seed 1 input as the named example. Empirically (R0 evidence § 10) the
*literal* `dense_random_fp_seeded::<7>(15, 17, 0.05, 1)` cell produces
identical pivots pre- and post-fix (`[1, 2, 4, 5, 9, 10, 11, 15]`) —
the named seed agrees by chance under the generator instantiation
this codebase ships. The structural bug is real, however: sweeping the
same seed/shape/density grid as `test_rref_markowitz_sweep_fp7`
(32 seeds × 6 shapes × 5 densities, GF(7)) revealed **47 divergent
cells** out of 960 pre-fix; post-fix sweep is divergence-free. Five
of those 47 cells are now hardcoded into the regression-guard test
`test_rref_canonical_known_buggy_cells_jit_bd9c6e13`. The named-seed
case is retained as a structural-correctness check (renamed
`test_rref_canonical_15x17_gf7_seed1_structural_correctness`) — not as
a regression guard.

## Fix

`ple_in_place_window` now snapshots `pivot_cols.len()` before the
left-half recursion and uses the slice
`pivot_cols[pivot_cols_start..pivot_cols_start + r1]` (the absolute
pivot columns the left half found) to build `L1` and `L1_bot` via two
new helpers:

- `materialise_l1_unit_at_cols(a, row_off, pivot_cols)` — reads
  `a[row_off + i, pivot_cols[j]]` for the strict-lower part.
- `materialise_block_at_cols(a, row_off, pivot_cols, rows)` — reads
  `a[row_off + i, pivot_cols[j]]` for the rectangular block.

The legacy `materialise_l1_unit` / `materialise_block` helpers (with the
broken contiguous-prefix read) are removed (they were not called from
any other site in the crate).

## Preservation of downstream uses

The fix is internal to `ple_in_place_window`. The public PLE/rref/lu/
nullspace API surface is unchanged. `split_compact` already extracts L's
columns by reading `working[i, pivot_cols[k]]` — it was correct against
the storage convention; only the inter-block step had drifted from it.

- **Rank computation** — `ple()`'s fourth return is unchanged by API; it
  now reports the correct rank on previously-bug-affected inputs (47
  cells in the discovery sweep had a strictly-lower-than-canonical
  rank).
- **Rank-deficient handling** — already covered by existing tests
  (`test_ple_rank_deficient_*`, `prop_ple_rank_deficient_factored`);
  no signature change.
- **Solver paths / `lu()`** — `lu()` requires `rank == min(m, n)` so
  full-rank inputs were unaffected; the fix only changes behaviour on
  rank-deficient inputs which `lu()` rejects with `None` regardless.
- **Allocation budget** — `materialise_l1_unit_at_cols` and
  `materialise_block_at_cols` produce the same shapes as their
  predecessors, so the strict allocation-budget asserts in
  `test_*_allocation_budget_*` are unchanged.

## Validation

Added four new tests in `crates/gf2-core/src/field/ple.rs::tests`:

1. `test_rref_canonical_15x17_gf7_seed1_structural_correctness` —
   structural-correctness check on the issue-named seed (15×17 GF(7),
   density 0.05, seed 1). The literal cell agrees pre/post-fix by
   chance under this generator instantiation, so the test verifies
   canonical RREF correctness rather than acting as a regression guard.
2. `test_rref_canonical_known_buggy_cells_jit_bd9c6e13` — the actual
   regression guard. Hardcodes 5 cells from the 47-cell pre-fix
   divergence list with their expected canonical pivots:
     - seed=0x8, 3×5, 0.50 → canonical `[1, 2, 4]`
     - seed=0x19, 8×8, 0.05 → canonical `[1, 3, 5]`
     - seed=0x1f, 8×8, 0.05 → canonical `[1, 2, 4, 5]`
     - seed=0x4, 8×8, 0.25 → canonical `[0, 2, 3, 4, 6, 7]`
     - seed=0xc, 8×8, 0.25 → canonical `[0, 2, 4, 5, 6, 7]`
3. `proptest_field_matrix_rref_canonical_rank_deficient_jit_bd9c6e13` —
   128-case `proptest!` block restricted to rank-deficient inputs (via
   outer-product construction), covers GF(7) and GF(251); each case
   asserts bit-exact equality with the canonical oracle.
4. `test_rref_canonical_markowitz_grid_sweep_fp7` — full replica of the
   Markowitz sweep grid in `sparse_matrix.rs` (32 seeds × 6 shapes ×
   5 densities, GF(7)). Pre-fix had 47 divergent cells; post-fix
   asserts zero.

The textbook oracle and seeded sparse-random generator are shared
across `ple.rs` and `sparse_matrix.rs` test modules via
`crates/gf2-core/src/field/test_random_matrix.rs` (R1 SSOT refactor:
`direct_rref_oracle_fp` and `dense_random_fp_sparse`). Both modules
import the shared helpers.

## Quality gates

| Gate | Command | Result |
|---|---|:---:|
| fmt | `cargo fmt --all -- --check` | PASS |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| gf2-core | `cargo nextest run -p gf2-core --release --all-features --profile ci` | 2032/2032 |
| Workspace | `cargo nextest run --workspace --all-features --release --profile ci` | 3818/3818 |
| Doctests | `cargo test --release --doc -p gf2-core` | 546/546 |
| PLE/RREF subset | `cargo nextest run -p gf2-core --release -E 'test(ple) \| test(rref) \| test(row_echelon) \| test(nullspace) \| test(lu_)' --profile ci` | 136/136 |
