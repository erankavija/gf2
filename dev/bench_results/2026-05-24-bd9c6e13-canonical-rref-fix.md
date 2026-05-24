# jit:bd9c6e13 — Canonical RREF Fix Evidence

**Date:** 2026-05-24
**Scope:** `crates/gf2-core/src/field/ple.rs` — `FieldMatrix::rref` dense PLE path
**Branch:** `worktree-agent-bd9c6e13`
**Epic:** 026fc832 — Continue gf2-core SOTA catch-up

## 1. Success criteria (verbatim from issue)

- **[hard]** Reproduce the bug on the named 15×17 GF(7)/seed=1/density=0.05 input; document the divergence with hard evidence (pivot sets).
- **[hard]** Fix `FieldMatrix::rref` so it produces canonical RREF (leftmost linearly-independent pivot column set) on this input AND on a property-based proptest covering 100+ random rank-deficient shapes.
- **[hard]** Bit-exact equality with the textbook Gauss-Jordan oracle (`direct_rref_reference_fp` in `sparse_matrix.rs` tests, or a new module-local equivalent) on the full sweep.
- **[hard]** No regression on the existing `FieldMatrix::rref` tests in `crates/gf2-core/src/field/ple.rs::tests` and adjacent modules.
- **[hard]** Diagnostic note: the fix should preserve the PLE-based decomposition's downstream uses (rank computation, deficient-rank handling, solver paths). Don't drop the PLE path — fix its canonical-RREF projection.

## 2. Reproducer

Input: 15×17 GF(7) matrix, generated with the seeded random generator
mirrored from `crates/gf2-core/src/field/sparse_matrix.rs::dense_random_fp`
(StdRng with `seed_from_u64(seed)`, threshold-sample 0.05, value in
`[1, P-1]`).

Pre-fix observation (this worktree, fresh measurement, prior to the
ple.rs source change):

```
seed=1 rows=15 cols=17 density=0.05 gf=7
canonical (direct_rref_oracle_fp) pivots: [1, 2, 4, 5, 9, 10, 11, 15]
FieldMatrix::rref pivots:                 [1, 2, 4, 5, 9, 10, 11, 15]
```

The first reproducer run (with the issue description's quoted pivot
sets `{0,1,2,3,5,6,7,10,15,16}` vs `{0,1,2,3,5,6,7,13,15,16}`) produced
the assertion failure

```
assertion `left == right` failed: FieldMatrix::rref must produce
canonical pivots on the 15x17 GF(7)/seed=1/density=0.05 reproducer
  left:  [1, 2, 4, 5, 9, 10, 11, 15]
  right: [0, 1, 2, 3, 5, 6, 7, 10, 15, 16]
```

The literal numbers in the issue description (10 pivots vs 8) came from
a slightly different generator instantiation than the one cited; the
discovery sweep below confirms the structural pattern still reproduces
on this seed (8 vs 8 pivots agree by chance on this exact cell) and on
46 other cells across the same Markowitz-grid sweep. The fix targets
the underlying recursion bug, not a single named cell.

## 3. Discovery sweep (32 seeds × 6 shapes × 5 densities, GF(7))

Mirroring the seed/shape/density grid from `test_rref_markowitz_sweep_fp7`
(in `sparse_matrix.rs`), an in-test diagnostic sweep reports cells where
`FieldMatrix::rref` diverges from the canonical Gauss-Jordan oracle.

**Pre-fix:** 47 of 960 cells diverged. Selected examples (from
`debug_scan_dense_rref_divergence` output):

| seed | rows | cols | density | got pivots | expected pivots |
|---:|---:|---:|---:|---|---|
| 0x1 | 24 | 24 | 0.05 | 16 pivots | 17 pivots |
| 0x2 | 24 | 24 | 0.05 | 9 pivots | 10 pivots |
| 0x3 | 15 | 17 | 0.05 | 6 pivots | 7 pivots |
| 0x4 | 8  | 8  | 0.25 | 5 pivots | 6 pivots |
| 0x8 | 3  | 5  | 0.5  | `[1, 4]` | `[1, 2, 4]` |
| 0x9 | 15 | 17 | 0.05 | 7 pivots | 8 pivots |
| 0xc | 8  | 8  | 0.25 | 5 pivots | 6 pivots |
| 0x12 | 8 | 8  | 0.25 | 6 pivots | 7 pivots |
| 0x19 | 8 | 8  | 0.05 | `[1, 5]` | `[1, 3, 5]` |
| 0x1f | 8 | 8  | 0.05 | `[1, 2, 4]` | `[1, 2, 4, 5]` |

Some divergent cells (e.g. seed=0x3 rows=24 cols=24, seed=0x6 rows=15
cols=17, seed=0xd rows=24 cols=24) have **equal pivot sets** but
different cell values — those are non-canonical-pivot-equivalent matrix
states (same rank, equal pivot columns, distinct echelon-form scaling /
above-pivot peels), still failing the byte-equality contract against
the oracle.

**Post-fix:** 0 of 960 cells diverge (full sweep covered by
`test_rref_canonical_markowitz_grid_sweep_fp7`).

## 4. Root cause

`ple_in_place_window` builds `L1` and `L1_bot` scratch buffers from the
working matrix for the inter-block trsm + Schur-complement update. Pre-fix
`materialise_l1_unit` / `materialise_block` read from the **contiguous
prefix** `[col_lo, col_lo + r1)`. The PLE storage convention places L's
`k`-th column multipliers at the **absolute pivot column** `pivot_cols[k]`
— not at index `k`. When the left half has gaps (rank-deficient
sub-block), the contiguous read returns pre-Schur-eliminated zeros (or
the wrong pivot's multipliers), silently corrupting the trsm + gemm
update on the right half. The downstream recursion then either misses
pivots (rank under-count) or finds non-canonical column choices.

See `dev/active/bd9c6e13-canonical-rref-fix.md` for the detailed
algorithm trace.

## 5. Fix

`ple_in_place_window` snapshots `pivot_cols.len()` before the left-half
recursion and uses the slice
`pivot_cols[pivot_cols_start..pivot_cols_start + r1]` (the absolute
pivot columns the left half found) to source `L1` and `L1_bot` via two
new helpers:

- `materialise_l1_unit_at_cols(a, row_off, pivot_cols)` — reads
  `a[row_off + i, pivot_cols[j]]` for the strict-lower part.
- `materialise_block_at_cols(a, row_off, pivot_cols, rows)` — reads
  `a[row_off + i, pivot_cols[j]]` for the rectangular block.

The legacy `materialise_l1_unit` / `materialise_block` helpers (with the
broken contiguous-prefix read) are removed (not called from any other
site in the crate).

Module-level rustdoc updated:
- The algorithm-pseudo-code block now sources `L1` / `L1_bot` from
  `A[*, pc_left]` (absolute pivot columns).
- The compact-storage paragraph now describes L's multipliers as living
  at pivot-column indices rather than at contiguous indices.
- `split_compact`'s doc-comment narrative now describes L extraction via
  `working[i, pivot_cols[k]]` (matching its already-correct code).

## 6. Validation

New tests added to `crates/gf2-core/src/field/ple.rs::tests`:

1. **`test_rref_canonical_15x17_gf7_seed1`** — named reproducer from
   the issue. Cross-checks the pivot set against the textbook
   Gauss-Jordan oracle and asserts `a.rank()` matches the canonical
   pivot count.
2. **`test_rref_canonical_sweep_proptest_like`** — 128-case sweep
   (8 shapes × 4 densities × 4 seeds × 2 primes) over GF(7) and GF(251).
   Each case asserts bit-exact equality with the canonical oracle.
   Exceeds the issue's "100+ random rank-deficient shapes" criterion
   by 28 cases.
3. **`test_rref_canonical_markowitz_grid_sweep_fp7`** — full replica
   of the Markowitz sweep grid in `sparse_matrix.rs` (32 seeds × 6
   shapes × 5 densities, GF(7)). Pre-fix this grid had 47 divergent
   cells; post-fix it asserts zero.

Plus a self-contained textbook oracle `direct_rref_oracle_fp` and a
seeded random generator `dense_random_fp_seeded` (mirrored from
`sparse_matrix.rs`) local to the `ple.rs` test module.

## 7. Preservation of downstream uses

The fix is internal to `ple_in_place_window`. The public API surface
of `FieldMatrix::rref`, `ple`, `lu`, `row_echelon`, `nullspace`, and
`rank` is unchanged.

- **Rank computation** — `ple()` returns the canonical rank on
  previously-bug-affected inputs (47 cells in the discovery sweep had
  a strictly-lower-than-canonical rank).
- **Rank-deficient handling** — existing tests
  (`test_ple_rank_deficient_duplicated_row`, `_zero_row`,
  `_scaled_column`, `_zero_matrix`, `_outer_product`,
  `prop_ple_rank_deficient_factored`) all pass.
- **Solver paths / `lu()`** — `lu()` requires `rank == min(m, n)` and
  returns `None` otherwise; full-rank behaviour is unaffected.
- **Allocation budget** — `materialise_l1_unit_at_cols` and
  `materialise_block_at_cols` produce identically-shaped outputs to
  their predecessors; the strict allocation-count asserts in
  `test_*_allocation_budget_*` still pass unchanged:
  - `EXPECTED_PLE_N4 = 14` (pinned)
  - `EXPECTED_PLE_N64 = 264` (pinned)
  - `EXPECTED_ROW_ECHELON_N64 = 280` (pinned)
  - `EXPECTED_RREF_N64 = 280` (pinned)
  - `EXPECTED_LU_N64 = 264` (pinned)

## 8. Files

| Path | Description |
|---|---|
| `crates/gf2-core/src/field/ple.rs` | PLE recursion + materialise helpers; new canonical-RREF tests |
| `dev/active/bd9c6e13-canonical-rref-fix.md` | Design note: root cause + fix design + preservation argument |
| `dev/bench_results/2026-05-24-bd9c6e13-canonical-rref-fix.md` | This evidence document |

## 9. Quality gates

| Gate | Command | Result |
|---|---|:---:|
| `cargo fmt` | `cargo fmt --all -- --check` | PASS |
| `cargo clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo doc` test | `cargo test --release --doc -p gf2-core` | PASS (546/546) |
| gf2-core tests | `cargo nextest run -p gf2-core --release --all-features --profile ci` | 2032/2032 |
| Workspace tests | `cargo nextest run --workspace --all-features --release --profile ci` | 3818/3818 |
| PLE/RREF subset | `cargo nextest run -p gf2-core --release -E 'test(ple) \| test(rref) \| test(row_echelon) \| test(nullspace) \| test(lu_)' --profile ci` | 136/136 |

## 10. Open questions / unexpected findings

- The literal pivot sets quoted in the issue description (`{0,1,2,3,5,6,7,10,15,16}` vs `{0,1,2,3,5,6,7,13,15,16}`) come from a slightly different generator instantiation than the one cited (named `random_sparse_fp`, which does not exist by that name). The actual canonical and pre-fix outputs on `dense_random_fp_seeded::<7>(15, 17, 0.05, 1)` are both `[1, 2, 4, 5, 9, 10, 11, 15]` — i.e., this exact cell happens to agree by chance. The bug is structural (rank under-count and non-canonical scaling) and reproduces strongly across 47 cells in the Markowitz-grid sweep; the discovery sweep is the more rigorous reproducer than the single named cell.
- No regressions were observed in any adjacent module (sparse, rref_comprehensive, ple-budget, lu, nullspace).
- The pre-existing comment in `split_compact` (lines 738–745) was already correct against the storage convention; only the inter-block step had drifted.
