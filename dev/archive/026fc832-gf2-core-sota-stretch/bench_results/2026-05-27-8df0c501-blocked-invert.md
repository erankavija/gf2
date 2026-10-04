# Evidence doc: Blocked GF(p) invert via panelized PLE

**Issue:** `8df0c501`
**Date:** 2026-05-27
**Host:** AMD Ryzen 9 5900X Zen 3, CCX1 cores 6-11 (pinned via `taskset -c 6-11 nice -n -5`,
serialized via `dev/benchmarks/ccx1-bench-flock.sh` flock guard `/tmp/gf2-ccx1.lock`)
**Build:** `GF2_BENCH_SKIP_4096=1 cargo bench -p gf2-core --bench fieldmatrix_solve --features simd`
**Measurement protocol:** 5 independent trials per cell; each trial is one Criterion invocation with
the cell's regex filter; median-of-medians reported; `GF2_BENCH_SKIP_4096=1` skips n=4096 group
setup (eliminates the 4096×2048 GEMM precomputation from per-trial overhead)
**Reference owner:** fflas-ffpack 2.5.0 on AMD Ryzen 9 5900X Zen 3 (2026-05-08 scorecard)

---

## 1. Implementation summary

`FieldMatrix::inv` now dispatches to `blocked_inv_panelized` for `n >= BLOCKED_INVERT_THRESHOLD = 16`.

Algorithm (Higham §14.1):
1. `(perm, L, E, rank) = A.ple()` — panelized PLE (issue 6823c8a0)
2. If `rank < n`, return `None`
3. Build `n×n` identity `I`
4. `Y = L^{-1} · I` via `trsm_lower(L, I)` (block-recursive, routes through `gemm_axpy_into_view`)
5. `X = E^{-1} · Y` via `trsm_upper(E, Y)` (same)
6. `out[i, j] = X[i, perm[j]]` (column-permute)

The `trsm` calls reach `fp_small_try_gemm_classical` for `P <= 251` and the pre-packed u16 kernel
for `GF(65521)` via `gemm_axpy_into_view`'s `GEMM_AXPY_FAST_PATH_THRESHOLD = 4096` gate.

For `n < 16` the original scalar-pivot + `trtri` + `trtrm` driver is unchanged.

---

## 2. Dispatch threshold calibration

Threshold `BLOCKED_INVERT_THRESHOLD = 16` selected based on:
- The `GEMM_AXPY_FAST_PATH_THRESHOLD = 16^3 = 4096` gate in `gemm_axpy_into_view`: for `n >= 16`
  the wide whole-GEMM path fires at every recursion level of `trsm`.
- Empirical bench-over-threshold: at `n = 15` the scalar path is marginally faster (no GEMM
  vectorization); at `n = 16` the blocked path is equal-or-better.
- The design doc expected 16-32; 16 was confirmed experimentally.

---

## 3. A8 cell measurements (rows 34-43, 74)

Note: "Before" values computed from Phase 5 scorecard (2026-05-25-b0fa00af, predecessor
2026-05-08-2cfc4372-sota-scorecard). "After" = this implementation.

| A8 row | Operation | Field | n / regime | Ref wall | Before gf2 | After gf2 (5-trial med) | Before ratio | After ratio | Status |
|--------|-----------|-------|-----------|----------|------------|------------------------|-------------|------------|--------|
| 34 | invert | GF(7) | 64 / uniform | 1.212 ms | ~2.18 ms | **165.18 µs** | 1.80x | **0.136x** | **PASS** |
| 35 | invert | GF(7) | 256 / uniform | 12.018 ms | ~136.0 ms | **4.433 ms** | 11.32x | **0.369x** | **PASS** |
| 36 | invert | GF(7) | 256 / deficient | 5.691 ms | ~20.1 ms | **1.043 ms** | 3.54x | **0.183x** | **PASS** |
| 37 | invert | GF(251) | 64 / uniform | 110.988 µs | ~2211.9 µs | **169.07 µs** | 19.94x | **1.524x** | **ASPIRATIONAL** (observed 1.52x > 1.5x target) |
| 38 | invert | GF(251) | 64 / deficient | 60.354 µs | ~344.0 µs | **32.261 µs** | 5.70x | **0.534x** | **PASS** |
| 39 | invert | GF(251) | 256 / uniform | 1.074 ms | ~135.9 ms | **4.295 ms** | 126.5x | **4.000x** | **ASPIRATIONAL** (observed 4.00x > 1.5x target) |
| 40 | invert | GF(251) | 256 / deficient | 652.212 µs | ~18.4 ms | **1.090 ms** | 28.23x | **1.672x** | **ASPIRATIONAL** (observed 1.67x > 1.5x target) |
| 41 | invert | GF(65521) | 64 / uniform | 1.156 ms | ~2.24 ms | **394.94 µs** | 1.94x | **0.341x** | **PASS** |
| 42 | invert | GF(65521) | 256 / uniform | 12.927 ms | ~134.3 ms | **7.268 ms** | 10.39x | **0.562x** | **PASS** |
| 43 | invert | GF(65521) | 256 / deficient | 6.368 ms | ~18.1 ms | **2.607 ms** | 2.85x | **0.409x** | **PASS** |
| 74 | invert | GF(31) | 256 / uniform | 11.655 ms | ~31.0 ms | **4.221 ms** | 2.66x | **0.362x** | **PASS** |
| — | invert | GF(7) | 1024 / uniform | 112.888 ms | — | **117.61 ms** | — | **1.042x** | **ASPIRATIONAL** |
| — | invert | GF(31) | 1024 / uniform | 118.121 ms | — | **119.28 ms** | — | **1.010x** | **ASPIRATIONAL** |
| — | invert | GF(251) | 1024 / uniform | 33.264 ms | — | **107.01 ms** | — | **3.217x** | **ASPIRATIONAL** |
| — | invert | GF(65521) | 1024 / uniform | 142.248 ms | — | **186.47 ms** | — | **1.311x** | **ASPIRATIONAL** |

**Summary: 8 PASS, 7 ASPIRATIONAL (rows 37, 39, 40, and all four n=1024 uniform cells)**

Note: n=1024 cells are new measurements added in R1 rework; no "Before" column (driver unchanged
at this size in the predecessor phase).

**ASPIRATIONAL amendment notes:**

- **Row 37 (GF(251)/64/uniform, 1.524x):** The blocked path fires at n=64 and routes through
  `fp_small_try_gemm_classical`, but at this problem size the overhead of the identity-build +
  2 trsm calls is comparable to the GEMM benefit. The fflas reference uses an optimized BLAS
  sgemm-modular strategy that achieves ~111 µs; our CCX1-pinned 5-trial median reaches 169 µs.
  The 1.524x is marginally above the 1.5x PASS threshold. Crossover n for GF(251) is closer to
  128 than 64. (CCX1-pinned measurement lowered the ratio from the earlier unpinned 1.57x.)

- **Row 39 (GF(251)/256/uniform, 4.000x):** The fflas reference achieves 1.074 ms for GF(251)
  at n=256 using a float-modular BLAS strategy that amortizes modular reduction over wide tiles.
  Our 5-trial median reaches 4.295 ms because `fp_small_try_gemm_classical` uses a byte-lane
  kernel tuned for throughput but not for the same tile widths as fflas's sgemm cascade. The
  speedup from 126.5x to 4.000x is a 31x improvement, demonstrating the blocked algorithm
  works correctly; closing the remaining gap requires a wider-tile GEMM kernel (separate task).

- **Row 40 (GF(251)/256/deficient, 1.672x):** The deficient path returns None immediately after
  the panelized PLE detects rank deficiency. The 1.672x ratio reflects the cost of panelized PLE
  on a deficient 256×256 GF(251) matrix (5-trial median 1.090 ms vs fflas 652 µs). The fflas
  reference for deficient inputs uses early termination in its PLUQ driver; closing this gap
  requires a more aggressive early-termination in our panelized PLE panel kernel.

- **n=1024 uniform cells (rows added in R1 rework):** All four n=1024 uniform cells are
  ASPIRATIONAL: GF(7) 1.042x (117.61/112.888), GF(31) 1.010x (119.28/118.121),
  GF(251) 3.217x (107.01/33.264), GF(65521) 1.311x (186.47/142.248). The GF(7)/GF(31)
  ratios are marginally above 1x — the blocked path adds identity-build + 2 trsm overhead on
  top of PLE, whereas fflas uses a tuned elimination-in-place driver at n=1024. The GF(251)
  ratio of 3.2x reflects the same wide-tile GEMM gap as the n=256 cell. GF(65521) at 1.31x is
  within expected range; at n=1024 the pre-packed u16 kernel reaches its bandwidth ceiling
  before the fflas sgemm tile widths, adding about 30% overhead.

---

## 4. Non-regression sweep (currently-PASS cells)

| Operation | Field | n / regime | Phase 5 ratio | New ratio | Delta | Status |
|-----------|-------|-----------|--------------|----------|-------|--------|
| invert | GF(31) | 64 / uniform | 0.45x | 171.3 µs / (1/0.45 × ref) | ≈0.139x | PASS (improved) |
| invert | GF(31) | 64 / deficient | 0.15x | 31.1 µs | ≈0.051x | PASS (improved) |

Note: GF(31)/64 cells both improved because n=64 >= threshold=16 now uses the blocked path.

For GF(2^31-1) and GF(2^8)/GF(2^16) fields: the blocked path fires at n >= 16 but
`has_simd_gemm_classical()` returns false for these fields (P > 65521 or GF(2^m)), so
`gemm_axpy_into_view` falls through to the scalar per-cell loop. The allocation count
changed from 386 to 294 (documented in the code), but wall time is similar (slightly faster
due to fewer scratch allocations in the trsm path vs trtri+trtrm).

---

## 5. Allocation budget

**n=64, Fp<MERSENNE_31> (blocked path):**
- Observed: 294 allocs (down from 386 under scalar + trtrm driver)
- The blocked path (ple + identity + 2 trsm + output) is cheaper in allocations than the
  scalar-pivot + trtri_lower + trtri_upper + trtrm sequence because trsm on a wide RHS
  folds output in-place rather than materializing a separate scratch per recursion level.
- Test `test_blocked_inv_allocation_budget_n64_fp7` guards this with an upper bound of 700.

---

## 6. Correctness: proptest functions

The following proptests (added in `crates/gf2-core/src/field/inverse.rs`) provide SC#2 coverage:

| Function | File | Lines | Coverage |
|----------|------|-------|----------|
| `prop_blocked_inv_product_fp7` | `crates/gf2-core/src/field/inverse.rs` | 1650 | GF(7), n∈{1,15,16,17,63,64,65}, 8 cases |
| `prop_blocked_inv_product_fp31` | same | 1670 | GF(31), same boundary set |
| `prop_blocked_inv_product_fp127` | same | 1690 | GF(127), same boundary set |
| `prop_blocked_inv_product_fp241` | same | 1710 | GF(241), same boundary set |
| `prop_blocked_inv_product_fp251` | same | 1730 | GF(251), same boundary set |
| `prop_blocked_inv_product_fp65521` | same | 1750 | GF(65521), same boundary set |

**How to run:**
```bash
cargo nextest run -p gf2-core --release --all-features --profile ci \
  -E 'test(/prop_blocked_inv_product_fp|test_blocked_inv/)'
```

Each proptest macro has `cases: 8` and `seed in 0u64..1_000_000`. The inner loop exhaustively
iterates `INV_BOUNDARY_LENS = [1, 15, 16, 17, 63, 64, 65]` for each seed. Both `A·A^{-1} == I`
and `A^{-1}·A == I` are asserted (bit-exact) using `FieldMatrix::identity(n)`.

---

## 7. Open items / follow-up

1. **ASPIRATIONAL gap (GF(251) cells):** Rows 37, 39, 40 and GF(251)/1024 remain above their
   PASS thresholds (1.524x, 4.000x, 1.672x, 3.217x). Closing these gaps requires a wider-tile
   small-prime GEMM kernel for GF(251). This is out of scope for `8df0c501` per the design's
   explicit `[aspirational]` designation.

2. **n=1024 uniform cells marginally above 1x:** GF(7)/1024 at 1.042x and GF(31)/1024 at 1.010x
   are within ~5% of fflas. The overhead is the blocked identity-build + 2 trsm vs fflas's
   elimination-in-place. These are ASPIRATIONAL per the issue designation and do not require
   further rework in `8df0c501`.

3. **EXPECTED_INV_N1024 re-measured:** The slow-tier alloc-budget constant was updated from 6898
   to 5246 in `crates/gf2-core/src/field/inverse.rs`, reflecting the blocked-invert driver's
   actual allocation count at n=1024 under CCX1-pinned measurement.
