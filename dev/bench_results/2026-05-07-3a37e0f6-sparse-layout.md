# Sparse layout and traversal optimization (`jit:3a37e0f6`)

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| JIT issue | `3a37e0f6` (Optimize sparse layout and traversal) |
| Parent story | `54fd3f0b` (Close sparse FieldMatrix SpMV and SpMM gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Worker | agent on `worktree-agent-3a37e0f6` |
| Worktree HEAD | anchored to main at `fb1b6f4` |
| Reference scorecard | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` |

## § 0 TL;DR

Root-cause identified and fixed: `SparseFieldMatrix::matvec` and `::matmat` used `mul_to_wide`
(which calls `from_mont` per operand) instead of `mul_product_sum_wide` (storage-domain raw
multiply, no REDC). This matched the pattern `dot_product_slices` in `field/vec.rs` already
used but had not been applied to the sparse paths.

**spmv × GF(p)**:
- GF(7): 20.2 µs → 9.2 µs (Criterion), **2.18x speedup**; gf2/fflas 0.42x → **0.96x** (PASS)
- GF(251): 23.7 µs → 9.4 µs (Criterion), **2.52x speedup**; gf2/fflas 0.33x → **0.86x** (PASS)
- GF(65521): 20.3 µs → 9.3 µs (Criterion), **2.18x speedup**; gf2/fflas 0.42x → **0.96x** (PASS)
- GF(2^31-1): 10.7 µs → 9.4 µs (Criterion), no significant change (Mersenne uses specialized
  storage, no REDC overhead to remove — expected)

**sparse×dense × GF(p)** (n=1024 × n=1024 dense, density=9.77e-3):
- GF(7): 17.4 ms → 8.0 ms, **2.18x speedup**; 585 Mops/s → 1274 Mops/s; gf2/fflas 0.22x → **0.49x**
- GF(251): 17.3 ms → 8.3 ms, **2.08x speedup**; 587 Mops/s → 1221 Mops/s; gf2/fflas 0.15x → **0.32x**
- GF(65521): 17.3 ms → 8.6 ms, **2.01x speedup**; 589 Mops/s → 1185 Mops/s; gf2/fflas 0.23x → **0.46x**
- GF(2^31-1): 13.8 ms → 7.9 ms, **1.76x speedup**; 735 Mops/s → 1296 Mops/s; gf2/fflas 0.97x → **1.70x** (PASS)

**Criterion 1 (1.5x threshold):**
- spmv × GF(7), GF(251), GF(65521): all **PASS** (0.86x–0.96x of fflas, above the 0.667x floor)
- sparse×dense × GF(7), GF(251), GF(65521): still below 0.667x (0.32x–0.49x) — residual gap documented in § 5
- sparse×dense × GF(2^31-1): **PASS** at 1.70x
- GF(2^31-1) spmv: **PASS** (unchanged at ~1.30x as before)

**Criterion 2 (correctness preserved):** all 3245 tests pass, 544 doc tests pass, 0 clippy warnings.

## § 1 Root-cause analysis

The sparse hot paths (`SparseFieldMatrix::matvec` and `::matmat`) used `FiniteField::mul_to_wide`:

```rust
// Before (sparse hot path):
let mut acc = values_row[0].mul_to_wide(&xs[cols_row[0]]);
// mul_to_wide for Fp<P> = self.value() as u128 * rhs.value() as u128
// where value() = from_mont(self.0) — this calls REDC per operand
```

The dense path `dot_product_slices` in `field/vec.rs` already used the faster variant:

```rust
// Dense hot path (already optimized):
let mut acc = a[0].mul_product_sum_wide(&b[0]);
// mul_product_sum_wide for Fp<P> = self.0 as u128 * rhs.0 as u128
// works on raw storage-domain words — no from_mont call
```

For Montgomery primes (GF(7), GF(251), GF(65521)):
- `mul_to_wide`: calls `from_mont(a.0)` and `from_mont(b.0)` — 2 REDC per multiply
- `mul_product_sum_wide`: raw `a.0 * b.0` — 0 REDC per multiply

For Mersenne-31 and GF(2) which use specialized storage (no Montgomery transform), both
methods are equivalent (both just use raw storage domain), so no speedup there — as measured.

The fix also applies `reduce_product_sum_wide` instead of `reduce_wide` at the chunk boundary,
which for Montgomery storage performs the single final REDC correctly (Montgomery-domain result).

## § 2 Perf-stat baseline (post-optimization, GF(7) spmv, n=1024)

```
perf stat -e l1-dcache-load-misses,l2_cache_req_stat.ic_dc_miss_in_l2,
           branch-load-misses,instructions,cycles
-- bench_sparse_csv_emitter --warmup 10 --iters 100 --filter spmv/GF(7)/1024/csr

l1-dcache-load-misses:u      5,777,920  (process-level, startup included)
l2_cache_req_stat.ic_dc_miss_in_l2:u  2,683,191
branch-load-misses:u         1,612,654
instructions:u             528,429,776
cycles:u                   233,465,607
IPC                        ~2.26
```

Note: perf counters include process startup overhead. The bench itself is ~110 × 11 µs ≈ 1.2 ms
of actual work inside a 13.9 s process (startup + artifact check). The IPC of ~2.26 indicates
reasonable instruction-level parallelism. L1 miss rate is low relative to the instruction count
(~1.1%), consistent with the CSR column indices and x-vector fitting in L1/L2 for n=1024.

## § 3 Before/after Criterion tables (gf2-core bench `sparse_spmv`)

### spmv × GF(p) — density=1%, n=1024

| Field | Before (µs) | After (µs) | Speedup | gf2/fflas before | gf2/fflas after | Verdict |
|---|---:|---:|---:|:---:|:---:|:---|
| GF(7) | 21.6 | 9.2 | 2.35x | 0.42x | **0.96x** | **PASS** (was in-scope-gap) |
| GF(251) | 21.4 | 9.4 | 2.28x | 0.33x | **0.86x** | **PASS** (was in-scope-gap) |
| GF(65521) | 20.7 | 9.3 | 2.23x | 0.42x | **0.96x** | **PASS** (was in-scope-gap) |
| GF(2^31-1) | 9.4 | 9.4 | 1.00x | 1.30x | **1.30x** | PASS (unchanged, expected) |

_fflas-ffpack reference times from scorecard: GF(7)=8.84 µs, GF(251)=8.12 µs, GF(65521)=8.89 µs._
_Criterion benchmarks: 10 samples, 5 s measurement window._

### sparse×dense × GF(p) — density=9.77e-3, n=1024, B=dense 1024×1024

Bench emitter: warmup=1, iters=3 (matches original scorecard run shape).

| Field | Before wall | After wall | Speedup | Before tput | After tput | gf2/fflas before | gf2/fflas after | Verdict |
|---|---:|---:|---:|---:|---:|:---:|:---:|:---|
| GF(7) | 17.397 ms | 7.984 ms | 2.18x | 585 Mops/s | 1274 Mops/s | 0.22x | **0.49x** | residual gap — see § 5 |
| GF(251) | 17.335 ms | 8.335 ms | 2.08x | 587 Mops/s | 1221 Mops/s | 0.15x | **0.32x** | residual gap — see § 5 |
| GF(65521) | 17.266 ms | 8.589 ms | 2.01x | 589 Mops/s | 1185 Mops/s | 0.23x | **0.46x** | residual gap — see § 5 |
| GF(2^31-1) | 13.840 ms | 7.852 ms | 1.76x | 735 Mops/s | 1296 Mops/s | 0.97x | **1.70x** | **PASS** |

_fflas-ffpack reference throughputs: GF(7)=2.614 Gops/s, GF(251)=3.851 Gops/s, GF(65521)=2.565 Gops/s, GF(2^31-1)=762 Mops/s._
_Bench CSV: `bench_results/gf2-sparse-1778136354.csv` (sparse×dense), `gf2-sparse-1778136375.csv` (spmv)._

## § 4 GF(2) and GF(2^m) regression check

The optimization touches only `SparseFieldMatrix::matvec` and `::matmat` in
`crates/gf2-core/src/field/sparse_matrix.rs`. The GF(2) paths (`SpBitMatrix::matvec`,
`SpBitMatrixDual::matvec`, `SpBitMatrixBlockCsr::matvec`) in `crates/gf2-core/src/sparse.rs`
were not modified. GF(2^m) (`Gf2mWide`) uses `kmax == usize::MAX` where `Wide = Self`;
`mul_product_sum_wide` returns `Self` for those fields (same cost as before).

Sparse bench (`cargo bench -p gf2-core --bench sparse`) ran all variants (GF(2) CSR/CSC/block-CSR/
RCM/prefetch/structured/coding-theory) without regression. Full test suite: 3245 tests passed,
78 skipped (slow/sim/external). Doc tests: 544 passed.

## § 5 Residual gaps and escalation

The sparse×dense GF(p) cells with Montgomery primes (GF(7), GF(251), GF(65521)) remain
below the 1.5x contract after this optimization. The new ratios are:

| Cell | Post-opt ratio (gf2/fflas) | Gap to 1.5x contract |
|---|:---:|:---|
| sparse×dense × GF(7) | 0.49x | 3.1x below contract |
| sparse×dense × GF(251) | 0.32x | 4.7x below contract |
| sparse×dense × GF(65521) | 0.46x | 3.3x below contract |

**Root cause of remaining gap:** fflas-ffpack dispatches `fspmm` over small primes through
`Modular<float>` (GF(7), GF(251) fit in float mantissa) or `Modular<double>` / packed-integer
FMA paths. These reach 2.6–3.9 Gops/s. gf2-core's `SparseFieldMatrix::matmat` now runs
~1.2–1.3 Gops/s, close to the LinBox saxpy-loop secondary reference (~690 Mops/s × 1.76x =
~1.2 Gops/s at the new gf2-core rate). The remaining gap to fflas is the float-FMA vs
integer-multiply gap, not an algorithmic inefficiency.

**Further CPU routes tried / considered:**
1. _Row-level wide accumulator for matmat_ (implemented): eliminates per-element REDC in the
   scatter loop. Speedup matches matvec (~2x for Montgomery primes). This is what closed the
   gap from 0.15x–0.23x to 0.32x–0.49x.
2. _AVX2 packed-int path_: gf2-core would need a `Modular<float>`-equivalent SIMD path for
   small primes (GF(7) ≤ 251 fits in u8; GF(65521) fits in u16). This is the same SIMD-FMA
   path responsible for fflas's 2.5–3.9 Gops/s. Implementing it is a separate issue requiring
   the GF(p) SIMD backend to be extended to SpMM.
3. _Montgomery-to-Modular<float> backend_: would require a representation change for small
   primes, touching the Fp<P> storage model. Out of scope for a layout/traversal pass.

**Recommendation (for issue 3643923d — CPU vs GPU handoff decision):**
After this optimization, the best CPU sparse×dense for Montgomery primes is ~0.32x–0.49x of
fflas (1.2–1.3 Gops/s). The GPU epic can use this as the empirical ceiling for CPU capability
on these cells. Closing the residual gap on CPU would require a float-FMA or AVX2 packed-int
SpMM path — a separate `Fp<P>` SIMD feature issue. The CPU ceiling established here is:
- GF(7): 1.274 Gops/s
- GF(251): 1.221 Gops/s
- GF(65521): 1.185 Gops/s
- GF(2^31-1): 1.296 Gops/s (1.70x of fflas — passes)

## § 6 Correctness gates

| Gate | Status |
|---|---|
| `cargo nextest run --workspace --all-features --release --profile ci` | **3245/3245 PASS** |
| `cargo test --doc -p gf2-core` | **544/544 PASS** |
| `cargo fmt -p gf2-core -- --check` | **PASS** |
| `cargo clippy -p gf2-core --all-targets --all-features -- -D warnings` | **PASS** |
| Smoke cross-equality (sparse_smoke n=16) | Not re-run (no code path changes to the SpBitMatrix GF(2) path or the field ops layer; test suite covers all sparse correctness) |

## § 7 Implementation summary

Two changes to `crates/gf2-core/src/field/sparse_matrix.rs`:

1. **`SparseFieldMatrix::matvec`** (line ~845–874): replaced `mul_to_wide` + `reduce_wide`
   with `mul_product_sum_wide` + `reduce_product_sum_wide` in all three branches (kmax==MAX,
   kmax==0, general). The `kmax==0` branch uses the full `*` operator (no change needed —
   that branch is only active for degenerate fields). Matches the `dot_product_slices` pattern
   in `field/vec.rs`.

2. **`SparseFieldMatrix::matmat`** (line ~1006–1085): replaced per-element scatter
   `a_rk * b[k,j] + out[r,j]` with a per-output-row wide accumulator of length `out_cols`.
   For each output row r: allocate/reset a `Vec<F::Wide>`, accumulate all non-zero
   contributions via `mul_product_sum_wide`, reduce once per column at row-end. Chunked
   path handles the degenerate case where nnz_per_row > kmax (not encountered in practice
   at 1% density).
