# M31 `gemm_axpy_into_view` Dispatch — Evidence

**JIT issue:** `6a7d4c8e`
**Date:** 2026-05-27
**Host:** AMD Ryzen 9 5900X Zen 3, CCX1 cores 6-11, `ccx1-bench-flock.sh`
**Commit:** see worktree-agent-6a7d4c8e history (ff3ab790, then bench commit)
**Branch:** `worktree-agent-6a7d4c8e`

---

## 1. Root Cause

The `m31_batch_dot_fn` kernel existed in `gf2-kernels-simd/src/mersenne.rs` and
was already used by `SimdVecOps::try_simd_dot_vec` (with the guard `P <= 251 && P
>= 3`), but `gemm_axpy_into_view` had no Mersenne31 whole-GEMM path. The
`try_simd_gemm_classical` dispatch routed `P <= 251` to `fp_small_try_gemm_classical`
and `P in (251, 65535]` to `fp_medium_try_gemm_panel`, but `Fp<M31>` (P =
2^31 - 1) uses canonical specialized storage — `fp_small_enabled` returns `false`
and `fp_medium_eligible` returns `false` — so every `gemm_axpy_into_view` call for
`Fp<M31>` fell through to the scalar `dot_product_slices` loop.

This caused A8 row 33 (GF(2^31-1)/n=1024/deficient) to FAIL at 2.37x vs fflas
because Stage 3a of the blocked back-substitution (`try_blocked_back_sub`) called
`gemm_axpy_into_view` on a ~768x768 sub-matrix and paid scalar `dot_product_slices`
for every cell.

---

## 2. Dispatch Wire-In Diff Summary

**New code in `crates/gf2-core/src/gfp/simd_ops.rs`:**

1. Thread-local scratch buffers `GEMM_M31_A_SCRATCH` and `GEMM_M31_BT_SCRATCH`
   (two `Vec<u32>` buffers, one per operand).

2. `fp_m31_try_gemm_classical<const P: u64>()` (simd) / stub (no-simd):
   - Guards `P != M31` → `false`.
   - Resolves `maybe_mersenne()` once per call.
   - Packs A (`m * k` u32 values) and B^T (`n * k` u32 values) into thread-local
     scratch using direct `raw_storage() as u32` (no REDC — M31 uses canonical
     storage).
   - For each output cell `(i, j)`: calls `m31_batch_dot_fn(a_row, bt_row)` on the
     pre-packed k-element slices. Stores result as
     `Fp::<P>::from_raw_storage(dot as u64)`.
   - Returns `true` on success.

3. `fp_m31_gemm_classical_available<const P: u64>()`: returns `P == M31 &&
   maybe_mersenne().is_some()`.

**Changes in `crates/gf2-core/src/gfp/mod.rs`:**

- `try_simd_gemm_classical`: added `fp_m31_try_gemm_classical` call FIRST (before
  `fp_small_try_gemm_classical`). M31 must be first because M31 is excluded from
  `fp_small_enabled` and `fp_medium_eligible`.

- `has_simd_gemm_classical`: now returns
  `fp_m31_gemm_classical_available() || fp_small_gemm_classical_available()`,
  so the `GEMM_AXPY_FAST_PATH_THRESHOLD` gate fires for M31 GEMM calls.

Both are guarded under `#[cfg(not(verify_lean))]` matching the existing pattern.

---

## 3. A8 Row 33 Before / After

Reference: fflas-ffpack 2.5.0 single-thread echelon time from
`dev/bench_results/2026-05-08-dece4e73-sota-aggregate-reference.csv`
(op=echelon, GF(M31), n=1024, deficient): **410,728,353 ns**.

PASS criterion: `gf2_wall_ns / fflas_wall_ns <= 1.50`.

| | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|---|---|---|---|---|
| Before (869ce43b) | 973,317,400 | 410,728,353 | 2.37× | FAIL |
| After (6a7d4c8e) | 269,035,767 | 410,728,353 | **0.655×** | **PASS** |

Improvement: **3.62× faster** (973 ms → 269 ms).

Bench command:
```bash
./dev/benchmarks/ccx1-bench-flock.sh \
  cargo test -p gf2-core --release --all-features --lib \
  -- --nocapture --ignored field::ple::tests::test_echelon_wall_time_full_sweep \
  2>&1 | grep -E 'GF\(M31\)|BEGIN|END'
```

Raw trial data: `dev/bench_results/2026-05-27-6a7d4c8e-m31-echelon-dispatch.csv`

---

## 4. Non-Regression Sweep — Currently-Passing M31 Cells

State A = 869ce43b measurements (from
`dev/bench_results/2026-05-26-869ce43b-blocked-echelon.md` §2 and §7).
State B = this issue (6a7d4c8e).
Method: 5-trial CCX1-pinned bench via `test_echelon_wall_time_full_sweep`.

| A8 row | n | regime | A median (ns) | B median (ns) | delta | PASS (≤5% regression) |
|--------|---|--------|--------------|--------------|-------|----------------------|
| 30 | 64 | uniform | 263,500 | 220,920 | −16.2% | Yes (improvement) |
| 31 | 64 | deficient | 234,100 | 191,660 | −18.1% | Yes (improvement) |
| — | 256 | uniform | 10,317,000 | 6,644,312 | −35.6% | Yes (improvement) |
| 32 | 256 | deficient | 9,558,900 | 6,083,612 | −36.4% | Yes (improvement) |

All currently-passing M31 cells improved (no regressions). The dispatch wire-in
accelerates ALL M31 echelon cells, not just the n=1024 target. At n=64 the
`GEMM_AXPY_FAST_PATH_THRESHOLD = 16^3 = 4096` is exceeded by the 64^3 = 262,144
total work, so M31 n=64 cells also benefit from the SIMD dispatch.

---

## 5. Correctness Verification

All existing and new tests pass:

```
prop_blocked_rref_boundary_sweep_uniform_mersenne31    PASS
prop_blocked_rref_boundary_sweep_deficient_mersenne31  PASS
test_gemm_axpy_into_view_mersenne31_simd_path          PASS
test_gemm_axpy_into_view_mersenne31_boundary_lengths   PASS
prop_gemm_axpy_into_view_mersenne31_matches_oracle     PASS
```

Full CI suite: 3949 tests passed, 0 failed.

`cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean.
`cargo fmt --all -- --check`: clean.

---

## 6. A8 Scorecard Update

Post-6a7d4c8e status:

| A8 row | n | regime | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|--------|---|--------|----------------|----------------|-------|-----------|
| 30 | 64 | uniform | 220,920 | 443,484 | 0.50× | PASS |
| 31 | 64 | deficient | 191,660 | 307,572 | 0.62× | PASS |
| — | 256 | uniform | 6,644,312 | (est. from §7) | — | PASS |
| 32 | 256 | deficient | 6,083,612 | 6,769,184 | 0.90× | PASS |
| 33 | 1024 | deficient | 269,035,767 | 410,728,353 | **0.655×** | **PASS** |

Row 33 now PASS. All GF(M31) echelon cells PASS.
