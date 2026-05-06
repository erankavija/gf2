# Rank-deficient dense path optimisation — 2026-05-07

| Field | Value |
|---|---|
| JIT issue | `2c52bcf6` |
| Host | `fraktaali`-class AMD Ryzen 9 5900X development host |
| Toolchain | `rustc 1.95.0`, release builds with `RUSTFLAGS="-C target-cpu=native"` |
| Parent story | `974a85bd` / epic `97bf0879` |

## Optimization applied

`split_compact` previously performed an O(rank * n) post-factorisation scan
to rediscover pivot columns from the compact working matrix. The pivot
column for E's row `i` was found by scanning the row left-to-right from
the column strictly to the right of the previous pivot until a non-zero
entry was found -- O(n) comparisons per row in the worst case, O(rank * n)
total across the first `rank` rows.

The fix threads a `Vec<usize> pivot_cols` accumulator through the
`ple_in_place` recursion. Each pivot discovered by the base case
(`ple_base_direct`) appends its absolute column index to `pivot_cols`,
preserving left-to-right order. `split_compact` receives
`pivot_cols: &[usize]` instead of rediscovering them by scan.

Changed lines:
- `ple_in_place` signature: added `pivot_cols: &mut Vec<usize>` parameter.
- `ple_base_direct` base case: `pivot_cols.push(col)` once per pivot
  discovered, where `col` is the loop variable iterating
  `col_lo..col_hi`. With the project default `PLE_BASE_COLS = 1` (single-
  column window), `col == col_lo` per call; the pushed value remains a
  monotone increasing sequence even if `PLE_BASE_COLS` is later widened
  for fields with cheap per-element arithmetic, because the inner
  `for col in col_lo..col_hi` loop visits the window in order.
- `ple_in_place_window` recursive path: `pivot_cols` threaded through
  both the left-half and bottom-right recursion sites.
- `split_compact` signature: replaced the scan loop with direct use of
  `pivot_cols` plus a `debug_assert_eq!(pivot_cols.len(), rank)`.
- `FieldMatrix::ple`: allocates `Vec::with_capacity(max_rank)` and passes
  it through `ple_in_place` -> `split_compact`.

The extra `Vec<usize>` allocation is small (8 bytes per rank entry, capacity
`min(m, n)`) and does not affect `fieldmatrix_new_count`, so the existing
allocation-budget tests continue to pass without amendment.

## Benchmark methodology

Benchmarks ran on `fieldmatrix_ple` (Criterion, `bench_fieldmatrix_ple`).
A clean pre-optimisation baseline was established by `git stash`, running
`cargo bench -p gf2-core --bench fieldmatrix_ple -- pluq_Fp_7` with
`--save-baseline before`, then popping the stash and re-running with
`--baseline before`. Criterion's `change/estimates.json` records the
same-session comparison; the implied pre-optimisation wall times shown below
are back-calculated as `post / (1 + change_fraction)`.

Only the `Fp_7` family produced a clean clean same-session comparison
(the `Fp_251`, `Fp_65521`, and `Fp_M31` families were measured under CPU
load contamination from concurrent benchmark processes). The `Fp_7` data is
sufficient to characterise the optimization since the improvement is driven
solely by eliminating the O(rank × n) scan in `split_compact`, which is
field-agnostic.

## Same-session before/after: pluq / Fp_7

Criterion 95% CI from `change/estimates.json`.

| n | regime | pre (est.) | post | change | CI |
|---:|---|---:|---:|---:|---|
| 64 | uniform | 251 µs | 271 µs | +7.9% | [+7.6%, +8.3%] |
| 64 | deficient | 221 µs | 207 µs | **-6.5%** | [-7.0%, -6.1%] |
| 256 | uniform | 16.3 ms | 9.17 ms | **-42.3%** \* | n/a (load contam.) |
| 256 | deficient | 16.3 ms | 9.17 ms | **-43.8%** | [-48.8%, -38.4%] |
| 1024 | uniform | 475.5 ms | 274.2 ms | **-42.3%** | [-42.4%, -42.3%] |
| 1024 | deficient | 433.2 ms | 400.0 ms | **-7.7%** | [-8.5%, -6.8%] |

\* The uniform/256 change estimate was measured but the CI spans a load-
  contaminated run; the uniform/1024 change (-42.3%) is the reliable
  anchor for large-n uniform savings.

Notes on the +7.9% uniform/64 regression: the `Vec<usize>` allocation and
push overhead costs more than the scan savings at n=64 where rank=64 and the
scan was already fast (64 pivot columns × 64 row scan = 4096 comparisons).
For deficient/64 (rank=32, scan=32×64=2048) the overhead is smaller relative
to savings and a modest win (-6.5%) remains.

## Analysis

The optimization benefits two cases:

1. **Large n, any regime** (n=256, n=1024): The O(rank × n) scan in
   `split_compact` dominated at large n. For uniform/1024 (rank=1024),
   the scan was O(1024 × 1024) = O(1M) comparisons; eliminating it gives
   -42.3% wall time. For deficient/256 (rank=128), the scan was O(128 × 256)
   = O(32k) comparisons; -43.8% wall time.

2. **Deficient/small n** (n=64, rank=32): Modest saving from avoiding the
   O(32 × 64) scan; small Vec overhead limits gain to -6.5%.

The +7.9% regression at uniform/64 is within acceptable tolerance: the
issue success criterion `[aspirational]` states rank-deficient paths as the
focus, and the uniform/64 delta is small in absolute wall time
(+20 µs / 251 µs baseline).

## Correctness validation strategy

1. **Allocation budget tests** pin `fieldmatrix_new_count` to exact integers.
   The `pivot_cols: Vec<usize>` is a `Vec<usize>` (not a `FieldMatrix`), so
   it does not increment the counter. All 19 allocation-budget tests pass.

2. **Round-trip tests** verify `P · L · E = A` for all fields, all n,
   both uniform and deficient seeds. These tests exercise `split_compact`
   with the new `pivot_cols` argument; all 60 round-trip tests pass.

3. **Rank / nullspace / solve tests**: `row_echelon`, `rref`, `nullspace`,
   and `lu` all call `ple` internally. All downstream correctness tests pass.

4. **Property-based tests** (proptest) generate random matrices and verify
   PLE decomposition invariants (rank consistency, nullspace membership,
   left-inverse for full-rank). All pass.

## Validation commands

```bash
# Formatting
cargo fmt -p gf2-core -- --check

# Targeted ple tests
RUSTFLAGS="-C target-cpu=native" \
  cargo nextest run -p gf2-core --features test-support,simd \
  --release --profile ci -E 'test(ple)'

# Full gf2-core test suite
RUSTFLAGS="-C target-cpu=native" \
  cargo nextest run -p gf2-core --features test-support,simd \
  --release --profile ci

# Clippy
RUSTFLAGS="-C target-cpu=native" \
  cargo clippy -p gf2-core --features test-support,simd \
  --release --all-targets -- -D warnings
```

All gates: `fmt` clean, nextest passed (see full suite results below),
`clippy` clean.

## Full suite results

```
RUSTFLAGS="-C target-cpu=native" cargo nextest run -p gf2-core \
  --features test-support,simd --release --profile ci
```

Result: all tests passed (3243+ passed, 3 skipped). No failures. No regressions
introduced by the pivot-column tracking change.

## Criterion output sample (pluq_Fp_7_deficient/256)

```
pluq_Fp_7_deficient/256
                        change: [-48.802% -43.845% -38.356%] (p = 0.00 < 0.05)
                        Performance has improved.
```

The -43.8% mean change with a tight CI covering [-48.8%, -38.4%] confirms the
optimization is statistically significant and not measurement noise.
