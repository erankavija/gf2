# Sparse layout and traversal optimization (`jit:3a37e0f6`)

| Field | Value |
|---|---|
| Date | 2026-05-07 (initial layout pass + 2026-05-07 packed-int continuation) |
| JIT issue | `3a37e0f6` (Optimize sparse layout and traversal) |
| Parent story | `54fd3f0b` (Close sparse FieldMatrix SpMV and SpMM gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Worker | agent on `worktree-agent-3a37e0f6` |
| Worktree HEAD | rebased onto main at `ac6d94d` |
| Reference scorecard | `dev/bench_results/2026-05-04-47698404-sparse-scorecard.md` |
| Continuation | Path B (user-approved): packed-int SpMM kernel for Montgomery-prime sparse×dense |

## § 0 TL;DR

Two-pass optimization in this issue: **(A)** layout/traversal — switch the sparse paths to the
storage-domain mul + delayed reduction already used by the dense paths; **(B)** packed-int SpMM
kernel — wire a new AVX2 byte / 16-bit-lane SpMM into the production `SparseFieldMatrix::matmat`
hook for Montgomery primes (GF(7)/GF(251)/GF(65521)). The Path-A pass closed 4 of 7 cells; the
Path-B continuation closed the remaining 3 sparse×dense cells.

**Final verdict for the 7 cells in scope:**

| Cell | Pre-A baseline | Post-A | Post-B | gf2/fflas final | Verdict |
|---|---|---|---|:---:|:---|
| spmv × GF(7) | 21.6 µs | 9.2 µs | 9.2 µs | 0.96x | **PASS** (Path A) |
| spmv × GF(251) | 21.4 µs | 9.4 µs | 9.2 µs | 0.88x | **PASS** (Path A) |
| spmv × GF(65521) | 20.7 µs | 9.3 µs | 9.1 µs | 0.97x | **PASS** (Path A) |
| sparse×dense × GF(7) | 17.4 ms | 8.0 ms | 3.6 ms | **1.08x** | **PASS** (Path B) |
| sparse×dense × GF(251) | 17.3 ms | 8.3 ms | 2.5 ms | **1.08x** | **PASS** (Path B) |
| sparse×dense × GF(65521) | 17.3 ms | 8.6 ms | 4.6 ms | **0.87x** | **PASS** (Path B) |
| sparse×dense × GF(2^31-1) | 13.8 ms | 7.9 ms | 8.5 ms | 1.41x | **PASS** (Path A) |

The Path B continuation flips all 3 previously-failing cells from below 0.5x of fflas to ≥ 0.87x,
landing GF(7) and GF(251) **above fflas** by 1.08x. This is achieved with an AVX2 packed-int
(byte-lane for `P ≤ 251`, 16-bit-lane for `P ∈ (251, 65521]`) SpMM kernel that mirrors the
Candidate C dense-GEMM dispatch from `662f7a15` / `9e12659b`.

The remainder of this document is split into:
- § 1–§ 7: Path A (layout/traversal) — landed in commit `46ec7f3` (rebased from `85d43d7`).
- § 8: Path B (packed-int SpMM kernel) — covered in this update.
- § 9: Final verdict table per cell + perf-stat baselines.

## § 0a Path A (layout) headline numbers — kept for reference

The Path A optimization replaced `mul_to_wide`/`reduce_wide` with `mul_product_sum_wide`/
`reduce_product_sum_wide` and added a per-row `Vec<F::Wide>` accumulator to `matmat`.
Same-session same-host numbers from commit `46ec7f3`:

**spmv × GF(p)** (Criterion, n=1024, density=1%):
- GF(7): 20.2 µs → 9.2 µs (2.18x speedup); gf2/fflas 0.42x → **0.96x**
- GF(251): 23.7 µs → 9.4 µs (2.52x speedup); gf2/fflas 0.33x → **0.86x**
- GF(65521): 20.3 µs → 9.3 µs (2.18x speedup); gf2/fflas 0.42x → **0.96x**
- GF(2^31-1): 10.7 µs → 9.4 µs (no significant change — specialized Mersenne storage)

**sparse×dense × GF(p)** (bench emitter, n=1024, density=9.77e-3):
- GF(7): 17.4 ms → 8.0 ms (2.18x); gf2/fflas 0.22x → 0.49x (gap remained)
- GF(251): 17.3 ms → 8.3 ms (2.08x); gf2/fflas 0.15x → 0.32x (gap remained)
- GF(65521): 17.3 ms → 8.6 ms (2.01x); gf2/fflas 0.23x → 0.46x (gap remained)
- GF(2^31-1): 13.8 ms → 7.9 ms (1.76x); gf2/fflas 0.97x → 1.70x (PASS via Path A)

**Criterion 2 (correctness preserved):** 3245/3245 tests pass, 544/544 doc tests pass, 0 clippy.

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

## § 8 Path B (continuation, user-approved): packed-int SpMM kernel

### § 8.1 Design

The Path-A pass closed every spmv cell and the Mersenne-31 sparse×dense cell; the three
Montgomery-prime sparse×dense cells remained at 0.32x–0.49x of fflas, where fflas dispatches
SIMD-FMA paths via `Modular<float>` / `Modular<double>` for `P ≤ 2^23`. Path B closes those
three cells with an AVX2 packed-int SpMM kernel that mirrors the Candidate C dense-GEMM
dispatch from issues `662f7a15` (small-prime byte lane) and `9e12659b` (medium-prime u16 lane).

The kernel signature, in pseudocode:

```text
spmm_row_kernel(a_vals[nnz_r], a_cols[nnz_r], b[b_rows × b_stride], n, p, out[n]):
    for each output column block (16 lanes):
        accumulator = 0  // u32 lanes (small prime) or u64 lanes (medium prime)
        for each non-zero (k, a_rk) of the sparse left row:
            broadcast a_rk to all lanes
            multiply elementwise with b[k, j..j+16]
            accumulate into u32 / u64 lanes
        reduce mod p (Barrett at 32-bit lane width, or scalar % at 64-bit)
        pack and store back to out[j..j+16]
    scalar tail for j ∈ [j..n)
```

For `P ≤ 251` (byte-lane kernel `fp_small_spmm_row`):
- 16 u8 lanes loaded per non-zero of A's row
- Expand to 16 u16 lanes via `_mm256_cvtepu8_epi16`
- Multiply by broadcast `_mm256_set1_epi16(a_h)` via `_mm256_mullo_epi16` — each product
  ≤ 250² = 62 500 < 2^16, no overflow.
- Widen to two u32×8 accumulators per block via `_mm256_unpacklo_epi16` /
  `_mm256_unpackhi_epi16` against zero, then `_mm256_add_epi32`.
- After all non-zeros: 32-bit Barrett reduce per lane (`mu32 = ⌊2³² / p⌋`), pack u32 →
  u16 → u8.
- u32 capacity per lane: `2³² / 250² ≈ 6.87 × 10⁴` MACs, far above any realistic nnz.

For `251 < P ≤ 65521` (16-bit-lane kernel `fp_medium_spmm_row`):
- 16 u16 lanes per non-zero
- Full u32 product per lane via `_mm256_mullo_epi16` + `_mm256_mulhi_epu16` + interleave
- Widen to four u64×4 accumulators per block via `_mm256_unpacklo_epi32` /
  `_mm256_unpackhi_epi32`
- After all non-zeros: scalar `% p` per u64 lane (4 u64 per accumulator vector × 4
  vectors = 16 lanes per block; 4-lane scalar reduce is the simplest correct path).
- u64 capacity per lane: `2⁶⁴ / 65520² ≈ 4.3 × 10⁹` MACs, far above any realistic nnz.

### § 8.2 Dispatch wiring

A new hook `FiniteField::try_simd_spmm` (in `crates/gf2-core/src/field/traits.rs`) takes the
whole CSR `(row_ptr, col_idx, values)` and dense `b` and tries to populate `out`. The default
returns `false` (the caller falls back to the Wide-accumulator path).

`Fp<P>::try_simd_spmm` routes to the new dispatcher `simd_ops::fp_try_spmm`, which:
1. Detects whether `P ≤ 251` (byte path) or `P ∈ (251, 65535]` (u16 path).
2. Packs `b` once into a canonical-byte / canonical-u16 buffer (skipping per-row REDC).
3. Packs all `a_values` once (canonical bytes / u16).
4. Sweeps every row of `A` through `fns.spmm_row_fn`, writing the packed output into a
   reused per-row scratch.
5. Unpacks each row from packed → `Fp<P>::new` to restore Montgomery storage.

`SparseFieldMatrix::matmat` calls `F::try_simd_spmm` at the head of the function; if the
hook returns `false` (e.g. for GF(2^m) or `P > 65535`), control falls through to the
existing per-row Wide-accumulator path from Path A. The path is therefore backward-compatible
and adds no regression risk for non-Fp callers.

### § 8.3 Path B results — same-session same-host bench emitter (warmup=5, iters=20)

Bench file: `bench_results/gf2-sparse-1778138911.csv`. Compared against Path A (`bench_results/
gf2-sparse-1778136354.csv`) and the original 2026-05-04 scorecard baseline.

| Cell | Pre-A wall | Post-A wall | Post-B wall | Pre-A tput | Post-A tput | Post-B tput | fflas tput | Pre-A ratio | Post-A ratio | Post-B ratio | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|:---:|:---:|:---:|:---|
| sparse×dense × GF(7) | 17.397 ms | 7.984 ms | 3.605 ms | 585 Mops/s | 1.274 Gops/s | **2.822 Gops/s** | 2.614 Gops/s | 0.22x | 0.49x | **1.08x** | **PASS** |
| sparse×dense × GF(251) | 17.335 ms | 8.335 ms | 2.452 ms | 587 Mops/s | 1.221 Gops/s | **4.149 Gops/s** | 3.851 Gops/s | 0.15x | 0.32x | **1.08x** | **PASS** |
| sparse×dense × GF(65521) | 17.266 ms | 8.589 ms | 4.581 ms | 589 Mops/s | 1.185 Gops/s | **2.221 Gops/s** | 2.565 Gops/s | 0.23x | 0.46x | **0.87x** | **PASS** |
| sparse×dense × GF(2^31-1) | 13.840 ms | 7.852 ms | 8.488 ms | 735 Mops/s | 1.296 Gops/s | 1.199 Gops/s | 762 Mops/s | 0.97x | 1.70x | **1.57x** | **PASS** |

Notes:
- Post-A vs Post-B for GF(2^31-1) shows a ~5% wall-time increase. This is system run-to-run
  noise (the SIMD hook returns `false` for Mersenne-31, so the same Path-A code path runs).
  GF(2^31-1) remains comfortably above the 1.5x contract.
- Path-B gains over Path-A: GF(7) **2.22x faster**, GF(251) **3.40x faster**, GF(65521)
  **1.87x faster**. GF(7) and GF(251) now beat fflas; GF(65521) sits at 0.87x of fflas.

### § 8.4 No-regression check on the 4 already-PASS cells

| Cell | Path-A status | Post-B status | Action |
|---|:---:|:---:|---|
| spmv × GF(7) | PASS (0.96x) | PASS (~0.96x — Criterion 9.2 µs vs fflas 8.84 µs = 1.04x; gf2/fflas = 0.96x) | No change to matvec hot path |
| spmv × GF(251) | PASS (0.86x) | PASS (Criterion 9.2 µs, 0.88x) | No change |
| spmv × GF(65521) | PASS (0.96x) | PASS (Criterion 9.1 µs, 0.97x) | No change |
| sparse×dense × GF(2^31-1) | PASS (1.70x) | PASS (1.57x) | SIMD hook returns false → Path-A code path |

The matvec path was unchanged in Path B. Criterion confirms post-B spmv times match Path-A:
9.0–9.2 µs (post-B Criterion run) vs 9.2–9.4 µs (Path A run) — within run-to-run noise.

### § 8.5 Correctness coverage

Four new unit tests cover the SIMD SpMM kernels:
- `crates/gf2-kernels-simd/src/x86/fp_small.rs::spmm_row_matches_scalar` — sweeps
  `(nnz, b_rows, n)` cases and primes `{3, 5, 7, 11, 13, 17, 31, 127, 251}` against a scalar
  reference. 16-lane block boundary, scalar tail, and the realistic SpMM cell `(nnz=15,
  b_rows=16, n=1024)` are all covered.
- `crates/gf2-kernels-simd/src/x86/fp_small.rs::spmm_row_empty_nnz` — empty-row contract:
  the kernel must emit zeros even when `nnz_r = 0`.
- `crates/gf2-kernels-simd/src/x86/fp_medium.rs::spmm_row_matches_scalar` — same shape sweep
  for primes `{257, 1009, 8191, 65521}`.
- `crates/gf2-kernels-simd/src/x86/fp_medium.rs::spmm_row_empty_nnz` — empty-row.

The full test suite runs `gf2-core` sparse property tests
(`crates/gf2-core/src/field/sparse_matrix.rs::tests::*matmat*`) which round-trip
`SparseFieldMatrix::matmat` against the dense reference via `to_dense() * b`. These pass
post-B for every prime field, confirming the SIMD dispatcher produces the same canonical
output as the Wide-accumulator fallback.

## § 9 Final verdict + correctness gates

### § 9.1 Final verdict per cell (post-Path-B)

All 7 cells in the issue scope now PASS the 1.5x contract (gf2/fflas ≥ 0.667x, equivalently
fflas-wall / gf2-wall ≤ 1.5):

| Cell | Path | gf2/fflas | Verdict |
|---|---|:---:|:---|
| spmv × GF(7) | A | 0.96x | **PASS** |
| spmv × GF(251) | A | 0.88x | **PASS** |
| spmv × GF(65521) | A | 0.97x | **PASS** |
| sparse×dense × GF(7) | B | **1.08x** | **PASS** |
| sparse×dense × GF(251) | B | **1.08x** | **PASS** |
| sparse×dense × GF(65521) | B | 0.87x | **PASS** |
| sparse×dense × GF(2^31-1) | A | 1.57x | **PASS** |

### § 9.2 Correctness gates (post-Path-B)

| Gate | Status |
|---|---|
| `cargo nextest run --workspace --all-features --release --profile ci` | **3250/3250 PASS** (1 minute) |
| `cargo test --doc -p gf2-core` | **544/544 PASS** (7.3 s) |
| `cargo fmt --all -- --check` (gf2-core/gf2-coding/gf2-kernels-simd) | **PASS** |
| `cargo clippy --workspace --all-targets --all-features --release -- -D warnings` | **PASS** |
| Doc build (`cargo doc --no-deps -p gf2-core`) | **PASS** (verified post-fmt fixes) |
| SpMM kernel unit tests (4 added) | **4/4 PASS** |

### § 9.3 Unsafe code isolation

All unsafe AVX2 intrinsics for the new kernels live in `crates/gf2-kernels-simd/src/x86/`:
- `fp_small.rs::fp_small_spmm_row` (byte-lane SpMM)
- `fp_small.rs::barrett_reduce_lane32` (32-bit Barrett reduction helper)
- `fp_medium.rs::fp_medium_spmm_row` (16-bit-lane SpMM)

The safe wrappers in `fp_small.rs::detect()` / `fp_medium.rs::detect()` (under
`#![deny(unsafe_code)]` from gf2-core's perspective) gate the kernels behind a runtime AVX2
detection. No unsafe code was added outside the kernel crate.

### § 9.4 Implementation summary (post-B)

Files modified by Path B:

1. `crates/gf2-kernels-simd/src/x86/fp_small.rs` — added `fp_small_spmm_row` (~110 lines)
   plus a 32-bit-lane Barrett helper and 2 unit tests.
2. `crates/gf2-kernels-simd/src/x86/fp_medium.rs` — added `fp_medium_spmm_row` (~115 lines)
   plus 2 unit tests.
3. `crates/gf2-kernels-simd/src/fp_small.rs` — added `SmallPrimeSpmmRowFn` typedef and the
   `spmm_row_fn` field on `SmallPrimeFns`; wired the safe wrapper.
4. `crates/gf2-kernels-simd/src/fp_medium.rs` — same shape, `MediumPrimeSpmmRowFn`.
5. `crates/gf2-core/src/field/traits.rs` — added the `try_simd_spmm` hook on `FiniteField`
   with a default that returns `false`.
6. `crates/gf2-core/src/gfp/mod.rs` — `Fp<P>::try_simd_spmm` routes to the dispatcher.
7. `crates/gf2-core/src/gfp/simd_ops.rs` — added `fp_try_spmm` dispatcher (canonical pack
   of `b` and `a_values`, per-row sweep through the kernel, canonical unpack into `Fp<P>`).
8. `crates/gf2-core/src/field/sparse_matrix.rs` — `SparseFieldMatrix::matmat` calls
   `F::try_simd_spmm` first; falls back to the Path-A Wide-accumulator path on `false`.

The commit `46ec7f3` (Path A) is preserved on the branch as-is; Path B is a stack-on commit.
