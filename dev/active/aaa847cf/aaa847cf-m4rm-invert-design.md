# M4RM-style invert path for BitMatrix — design note

| Field | Value |
|---|---|
| Date | 2026-05-24 |
| JIT issue | `aaa847cf` (M4RI-style invert path for BitMatrix) |
| Parent epic | `026fc832` (Continue gf2-core SOTA catch-up) |
| Successor of | `97bf0879` umbrella amendment A8 rows 44–46 (GF(2) invert FAIL cells) |

## 1. Goal

Bring `BitMatrix::invert` at n ∈ {64, 256, 1024} within 1.5× of M4RI's
`mzd_invert_m4ri` (M4RI 20260122). The current Gauss–Jordan path in
`crates/gf2-core/src/alg/gauss.rs::invert` is roughly 3.5×/8.4×/16.9× slower
than M4RI at those sizes per the
2026-05-08 SOTA scorecard § 2.3 (rows 210–212).

## 2. Why the current path is slow

The current `invert(m)` does, for an n×n input:

1. Allocate an augmented `n × 2n` `BitMatrix`.
2. For each pivot column `col ∈ 0..n`:
   - Find a pivot row, swap.
   - For every **other** row r with bit `col` set, allocate a `Vec<u64>` copy of
     the pivot row's word slice and `xor_inplace` it into row `r`.
3. Slice out the right half.

This is the textbook O(n³/w) Gauss–Jordan where `w = 64` (word width). It
issues n×(n-1) row-XOR operations each touching ~`2n/64` u64 words. M4RI
amortises the column work by processing `k = log₂(n)` columns at a time and
batching the eliminations through a Gray-code table of 2ᵏ precomputed row
combinations. The asymptotic improvement is exactly the `log₂(n)` factor
visible in the scorecard ratios (≈3.5× at n=64, ≈10–17× at n=1024).

## 3. Strategy

The existing `crates/gf2-core/src/alg/rref.rs` already ships an M4RI-style
blocked schedule for RREF (see `eliminate_block`, `block_table_index`). The
plan is to add a sibling routine `invert_m4ri` in `alg/gauss.rs` that:

1. Allocates the augmented matrix `[A | I]` as today (a single `BitMatrix`
   with stride `2*n` bits).
2. Walks column blocks of width `k_block`:
   - For each block, find `k_block` pivots inside the block by running the
     same `find_block_pivot` / row-swap loop as `rref.rs` (with the
     adaptation that this is `invert`, so the matrix is square and we never
     short-circuit on rank deficiency — we either complete a full block or
     return `None`).
   - After establishing the `k_block` pivots, build a Gray table over the
     `k_block` pivot rows for the **entire** augmented row suffix (from the
     first block word to the end of the right half).
   - Apply the table to **all** other rows (both above and below the block —
     this is the Gauss–Jordan twist that distinguishes invert from pluq/echelon)
     via `row_xor_slice_from`.
3. Once every column has been eliminated, the right half is `A⁻¹`.

The Gauss–Jordan property (we eliminate rows above the pivot as well as
below) is the only structural difference from the existing `rref.rs`
block-elimination — the kernels and table construction are identical.

`k_block` choice (matches M4RI's `m4ri_optk` rule of thumb, capped by the
existing `rref::default_block_size` policy):

- n ≤ 64 → k = 4
- 65 ≤ n ≤ 512 → k = 4–6 (empirically tuned)
- n > 512 → k = 8

`log₂(n)` gives 6 at n=64, 8 at n=256, 10 at n=1024. We start with k = 4 at
n=64 (table cost dominates at small n, M4RI itself uses k ≈ 3 there) and
saturate at k = 8 for n ≥ 1024 (the same cap RREF uses; bigger tables blow
out of L2). The exact threshold is read from `default_block_size_invert` and
can be tuned empirically post-implementation if a cell still fails.

## 4. Production code path & dispatch

- New function: `pub fn invert_m4ri(m: &BitMatrix) -> Option<BitMatrix>` in
  `crates/gf2-core/src/alg/gauss.rs`.
- The existing `pub fn invert` is updated to dispatch to `invert_m4ri` for
  n ≥ INVERT_M4RI_THRESHOLD (default 8) and to the existing scalar path for
  smaller matrices where the M4RI Gray-table setup is overhead-dominated.
- The doc comment on `invert` is rewritten to describe the new dispatch and
  drop the obsolete "augmented matrix [A | I]" specifics that no longer
  match every code path. `invert_m4ri` carries the algorithm description.
- No new unsafe — the M4RM Gray-table builder already exists in
  `alg/m4rm.rs::build_gray_table_flat` and is SIMD-dispatched there; we
  invoke it directly so we inherit any AVX2/AVX-512 fast path that lands
  later.

## 5. Correctness oracle

- Bit-exact equality with `invert` (renamed to `invert_gauss_scalar` for the
  reference path, kept `pub(crate)`/`#[doc(hidden)]` as a property-test
  oracle) on:
  - Identity at n ∈ {0, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129}
  - Random invertible matrices at the same set of n via `BitMatrix::random_seeded`
  - All singular-input cases (zero rows, duplicate rows) → both return
    `None`
- All existing `tests/inversion.rs` tests pass unchanged (they call the
  public `invert` symbol).
- New proptest `prop_invert_m4ri_equals_gauss` covers random binary
  matrices at sizes that span the word boundary and the dispatch threshold.

## 6. Measurement plan

`crates/gf2-core/benches/matrix_inversion.rs` already covers n ∈ {64, 128,
256, 512, 1024}; the same harness measures the new path because the
benchmark calls the public `invert` symbol.

For the evidence doc, 5-trial CCX1-pinned runs (`taskset -c 6-11 nice -n -5`)
at n ∈ {64, 256, 1024} against:
- gf2-core post-rework
- M4RI `mzd_invert_m4ri` (canonical `benchmarks/reference/m4ri_bench`)

Each trial is a separate criterion invocation; we record the criterion
median per trial and report the 5-trial median of medians per cell. Ratio
target: gf2/m4ri ≤ 1.5.

## 7. Risk & escape hatches

| Risk | Mitigation |
|---|---|
| Gray-table builder cost dominates at n=64 | k_block = 4 (16 entries), dispatch threshold biases to scalar path under that size |
| Block elimination above + below the pivot doubles row traffic vs RREF | The savings are amortised by the table; M4RI takes the same hit and still wins by `log₂(n)` |
| stride 2n at n=64 is 2 words (128 bits) — too thin for SIMD dispatch | Acceptable — n=64 is small enough that the asymptotic savings target dominates the SIMD-vs-scalar gap |
| Cell still misses 1.5× after the implementation | Surface as an open question in the evidence doc; do not silently amend the criterion |

If after a careful walk-through one cell still misses 1.5× I'll surface that
as an open question, with a specific structural reason (e.g., M4RI's PLE
decomposition exploits a memory layout gf2-core doesn't expose) before any
amendment is requested.
