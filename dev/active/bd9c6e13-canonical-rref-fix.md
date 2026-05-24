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

A 15×17 GF(7) matrix at density 0.05 / seed 1 (named in jit:bd9c6e13)
exposes the bug. Sweeping the same seed/shape/density grid as
`test_rref_markowitz_sweep_fp7` (32 seeds × 6 shapes × 5 densities,
GF(7)) reveals **47 divergent cases** out of 960 pre-fix; post-fix
sweep is divergence-free.

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

Added three new tests in `crates/gf2-core/src/field/ple.rs::tests`:

1. `test_rref_canonical_15x17_gf7_seed1` — named reproducer from the
   issue description; cross-checks pivot set against the textbook
   Gauss-Jordan oracle and asserts `a.rank()` matches the canonical
   pivot count.
2. `test_rref_canonical_sweep_proptest_like` — 128-case sweep (8 shapes
   × 4 densities × 4 seeds × 2 primes) over GF(7) and GF(251); each
   case asserts bit-exact equality with the canonical oracle.
3. `test_rref_canonical_markowitz_grid_sweep_fp7` — full
   replica of the Markowitz sweep grid in `sparse_matrix.rs`
   (32 seeds × 6 shapes × 5 densities, GF(7)). Pre-fix this grid had
   47 divergent cells; post-fix it asserts zero.

Plus a textbook oracle `direct_rref_oracle_fp` (and `dense_random_fp_seeded`)
local to the `ple.rs` test module, mirroring `direct_rref_reference_fp`
in `sparse_matrix.rs` so the test harness is self-contained.

## Quality gates

| Gate | Command | Result |
|---|---|:---:|
| fmt | `cargo fmt --all -- --check` | PASS |
| clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| gf2-core | `cargo nextest run -p gf2-core --release --all-features --profile ci` | 2032/2032 |
| Workspace | `cargo nextest run --workspace --all-features --release --profile ci` | 3818/3818 |
| Doctests | `cargo test --release --doc -p gf2-core` | 546/546 |
| PLE/RREF subset | `cargo nextest run -p gf2-core --release -E 'test(ple) \| test(rref) \| test(row_echelon) \| test(nullspace) \| test(lu_)' --profile ci` | 136/136 |
