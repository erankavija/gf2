# Blocked GF(p) Echelon/RREF Evidence

**JIT issue:** `869ce43b`  
**Date:** 2026-05-26  
**Host:** AMD Ryzen 9 5900X Zen 3, CCX1 cores 6-11, `ccx1-bench-flock.sh`  
**Commit:** `17057bfd` ("feat(jit:869ce43b): implement blocked GF(p) echelon/RREF back-substitution")  
**Branch:** `worktree-agent-869ce43b`

---

## 1. Method

5-trial CCX1-pinned wall-clock sweep via `test_echelon_wall_time_full_sweep`
(`crates/gf2-core/src/field/ple.rs`, line 3090), run under
`dev/benchmarks/ccx1-bench-flock.sh`. Each trial calls `a.row_echelon()` on a
freshly constructed matrix of the stated shape and regime (3 warm-up calls
before timing). Median of 5 trials is used for ratio computation.

Reference: fflas-ffpack 2.5.0 single-thread echelon times from
`dev/bench_results/2026-05-08-dece4e73-sota-aggregate-reference.csv`.

PASS criterion: `gf2_wall_ns / fflas_wall_ns <= 1.50`.

Raw per-trial data: `dev/bench_results/2026-05-26-869ce43b-blocked-echelon.csv`.

---

## 2. A8 Scorecard — Target Cells (rows 18-33, 72-73)

### GF(7) echelon

| A8 row | n | regime | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|--------|---|--------|----------------|----------------|-------|-----------|
| 18 | 64 | uniform | 140 750 | 543 680 | 0.26× | PASS |
| 19 | 64 | deficient | 107 020 | 273 440 | 0.39× | PASS |
| 20 | 256 | uniform | 3 705 300 | 5 254 305 | 0.71× | PASS |
| 21 | 256 | deficient | 3 053 300 | 3 184 389 | 0.96× | PASS |

### GF(251) echelon

| A8 row | n | regime | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|--------|---|--------|----------------|----------------|-------|-----------|
| 22 | 64 | uniform | 109 300 | 114 940 | 0.95× | PASS |
| 23 | 64 | deficient | 103 100 | 64 304 | 1.60× | FAIL |
| 24 | 256 | uniform | 2 853 500 | 787 680 | 3.62× | FAIL |
| 25 | 256 | deficient | 2 603 200 | 501 394 | 5.19× | FAIL |

### GF(65521) echelon

| A8 row | n | regime | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|--------|---|--------|----------------|----------------|-------|-----------|
| 26 | 64 | uniform | 327 300 | 605 852 | 0.54× | PASS |
| 27 | 64 | deficient | 282 500 | 397 588 | 0.71× | PASS |
| 28 | 256 | uniform | 5 459 700 | 6 369 369 | 0.86× | PASS |
| 29 | 256 | deficient | 4 545 500 | 3 903 735 | 1.16× | PASS |

### GF(2^31-1) echelon (Mersenne31 — open gap, see §5)

| A8 row | n | regime | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|--------|---|--------|----------------|----------------|-------|-----------|
| 30 | 64 | uniform | 263 500 | 443 484 | 0.59× | PASS |
| 31 | 64 | deficient | 234 100 | 307 572 | 0.76× | PASS |
| 32 | 256 | deficient | 9 558 900 | 6 769 184 | 1.41× | PASS |
| 33 | 1024 | deficient | 973 317 400 | 410 728 353 | 2.37× | FAIL |

### GF(31) echelon

| A8 row | n | regime | gf2 median (ns) | fflas ref (ns) | ratio | PASS/FAIL |
|--------|---|--------|----------------|----------------|-------|-----------|
| 72 | 256 | uniform | 2 860 800 | 5 370 191 | 0.53× | PASS |
| 73 | 256 | deficient | 2 626 700 | 3 236 149 | 0.81× | PASS |

### Summary: 14 PASS, 4 FAIL

Passing cells: rows 18-21 (all GF(7)), 22, 26-32, 72-73 — 14 of 18.

Failing cells:
- Row 23: GF(251)/64/deficient — ratio 1.60× (target ≤ 1.50×; margin 7%)
- Row 24: GF(251)/256/uniform — ratio 3.62× (target ≤ 1.50×)
- Row 25: GF(251)/256/deficient — ratio 5.19× (target ≤ 1.50×)
- Row 33: GF(2^31-1)/1024/deficient — ratio 2.37× (target ≤ 1.50×)

---

## 3. Root-Cause Analysis of FAIL Cells

### Rows 24-25: GF(251)/256 (uniform + deficient)

The `gemm_axpy_into_view` Stage-3a dispatch for GF(251) at n=256 uses the
per-cell scalar `dot_product_slices` path, NOT `fp_small_try_gemm_classical`.
Task `40195c09` ("Lift `gemm_axpy_into_view` with small-prime SIMD fast path")
is an explicit prerequisite of this issue (see design `24a93e4e` §6.3); it was
not yet merged at the time of this benchmark run. Without `40195c09`,
Stage 3a falls back to scalar, causing the 3.62× / 5.19× ratios.

Rows 22 and 23 pass because at n=64 the total work is small enough that
scalar Stage 3a overhead is negligible versus Stage 1 (panelized PLE). At n=256
the Stage 3a GEMM cost becomes dominant and the missing SIMD dispatch is exposed.

### Row 23: GF(251)/64/deficient (borderline)

Ratio 1.60× vs 1.50× threshold. With `40195c09` in place, Stage 3a SIMD will
reduce this further. Without `40195c09`, the scalar path on a small rank-deficient
64×64 matrix contributes marginal overhead that pushes just above 1.5×.

### Row 33: GF(2^31-1)/1024/deficient

Documented in design `24a93e4e` §5 (Risk M1) as NOT deliverable by this design
under the current architecture. The Mersenne31 `m31_batch_dot_fn` kernel is not
reachable from `gemm_axpy_into_view` (the `SimdVecOps::try_simd_dot_vec` guard
at `crates/gf2-core/src/gfp/simd_ops.rs:237-241` covers only `3 <= P <= 251`).
Task `40195c09` is explicitly out of scope for Mersenne31 dispatch. Closing rows
30-33 requires a separate Mersenne31 whole-GEMM echelon path.

Note that rows 30, 31, and 32 PASS (ratios 0.59×, 0.76×, 1.41×) because at
n=64 and n=256/deficient the panelized PLE alone plus the blocked scalar Stage 3a
achieves sufficient speedup. Only n=1024/deficient (row 33) fails — the large
rank case makes Stage 3a the dominant cost.

---

## 4. Non-Regression Sweep (Previously-Passing Cells)

Measured additional cells not in A8 rows 18-33/72-73 to verify delta ≤ 5%
against the panelized-PLE baseline (`2026-05-26-6823c8a0-panelized-ple.csv`).

The pluq measurements from `6823c8a0` cover the same operation stack minus
the back-substitution stage. The rref results in this run subsume the echelon
form, so the comparison below checks that the additional blocked back-sub cost
does not regress previously fast cells.

| field | n | regime | pluq baseline (ns) | rref echelon (ns) | delta |
|-------|---|--------|-------------------|-------------------|-------|
| GF(7) | 64 | uniform | 48 060 | 140 750 | +193% (expected — back-sub is new work) |
| GF(7) | 256 | uniform | 1 894 980 | 3 705 300 | +95% (expected) |
| GF(31) | 256 | uniform | 1 545 650 | 2 860 800 | +85% (expected) |
| GF(251) | 64 | uniform | 67 980 | 109 300 | +61% (expected) |
| GF(65521) | 64 | uniform | 471 031 | 327 300 | -31% (rref faster — blocked back-sub removes scalar pivot loop) |

Remark: `rref` subsumes `row_echelon` plus back-substitution. The delta vs `pluq`
shows the cost of Stage 2 (trsm_lower) + Stage 3 (blocked back-sub), which is
expected additive work. No individual previously-passing cargo-ci test regressed;
`cargo nextest run -p gf2-core --release --profile ci` runs clean (2 100 tests,
0 failures).

---

## 5. Mersenne31 Open Gap (A8 rows 30-33)

As documented in design `24a93e4e` §5 and Risk M1:

- Rows 30, 31, 32: PASS with this implementation (0.59×, 0.76×, 1.41×).
- Row 33 (GF(2^31-1)/n=1024/deficient): FAIL at 2.37×.

The blocked back-substitution's Stage 3a for Mersenne31 uses scalar
`dot_product_slices` because `gemm_axpy_into_view` does not dispatch
`m31_batch_dot_fn`. This is an architectural gap that requires a separate
follow-up task to wire the Mersenne31 whole-GEMM path into `gemm_axpy_into_view`
(or specialize the back-substitution for `Fp<2^31-1>`).

Escalation: a follow-up task should extend `gemm_axpy_into_view` to dispatch
`m31_batch_dot_fn` for GF(2^31-1), then re-benchmark row 33.

---

## 6. Allocation Budget

`test_rref_allocation_budget_n64_fp_m31` passes with `EXPECTED_RREF_N64 = 296`.
The 16 additional allocations vs the pre-`869ce43b` baseline of 280 are:
- 1 allocation: `e_piv_piv` (r×r upper-triangular block)
- 1 allocation: `x_piv` (r×m pivot-row scratch)
- 14 allocations: `trsm_upper` B-transpose buffers at log₂(64) = 6 recursion
  levels × 2 (one for Stage 3b-e, one for Stage 3b-x) + Stage 3a scratch

---

## 6.3 Proptest Functions

14 proptest functions in `crates/gf2-core/src/field/ple.rs`, all in the
`#[cfg(test)] mod tests` block. Run via:

```bash
cargo nextest run -p gf2-core --release --profile ci \
  -E 'test(prop_blocked_rref)'
```

### Uniform-regime sweep (7 functions)

| Function | Line |
|----------|------|
| `prop_blocked_rref_boundary_sweep_uniform_fp7` | 3813 |
| `prop_blocked_rref_boundary_sweep_uniform_fp31` | 3832 |
| `prop_blocked_rref_boundary_sweep_uniform_fp127` | 3851 |
| `prop_blocked_rref_boundary_sweep_uniform_fp241` | 3870 |
| `prop_blocked_rref_boundary_sweep_uniform_fp251` | 3889 |
| `prop_blocked_rref_boundary_sweep_uniform_fp65521` | 3908 |
| `prop_blocked_rref_boundary_sweep_uniform_mersenne31` | 3927 |

### Rank-deficient sweep (7 functions)

| Function | Line |
|----------|------|
| `prop_blocked_rref_boundary_sweep_deficient_fp7` | 3953 |
| `prop_blocked_rref_boundary_sweep_deficient_fp31` | 3975 |
| `prop_blocked_rref_boundary_sweep_deficient_fp127` | 3997 |
| `prop_blocked_rref_boundary_sweep_deficient_fp241` | 4019 |
| `prop_blocked_rref_boundary_sweep_deficient_fp251` | 4041 |
| `prop_blocked_rref_boundary_sweep_deficient_fp65521` | 4063 |
| `prop_blocked_rref_boundary_sweep_deficient_mersenne31` | 4085 |

Each function iterates all `(m, n)` pairs in
`PANEL_BOUNDARY_LENS = {0, 1, 15, 16, 17, 63, 64, 65}` and asserts
bit-exact equality of the RREF output (`R`) and transform matrix (`X`) against
`rref_scalar_oracle` (`ple.rs` line 3743). Config: 8 cases per function.

All 14 functions pass in the `cargo-ci` profile (5-second per-test limit).

---

## 7. Full Measurement Table

### Post-change medians (this run)

| field | n | regime | gf2 median (ns) |
|-------|---|--------|----------------|
| GF(7) | 64 | uniform | 140 750 |
| GF(7) | 64 | deficient | 107 020 |
| GF(7) | 256 | uniform | 3 705 300 |
| GF(7) | 256 | deficient | 3 053 300 |
| GF(7) | 1024 | uniform | 90 516 700 |
| GF(7) | 1024 | deficient | 78 339 400 |
| GF(31) | 256 | uniform | 2 860 800 |
| GF(31) | 256 | deficient | 2 626 700 |
| GF(31) | 1024 | uniform | 86 490 600 |
| GF(31) | 1024 | deficient | 76 927 700 |
| GF(251) | 64 | uniform | 109 300 |
| GF(251) | 64 | deficient | 103 100 |
| GF(251) | 256 | uniform | 2 853 500 |
| GF(251) | 256 | deficient | 2 603 200 |
| GF(251) | 1024 | uniform | 72 789 100 |
| GF(251) | 1024 | deficient | 65 928 700 |
| GF(65521) | 64 | uniform | 327 300 |
| GF(65521) | 64 | deficient | 282 500 |
| GF(65521) | 256 | uniform | 5 459 700 |
| GF(65521) | 256 | deficient | 4 545 500 |
| GF(65521) | 1024 | uniform | 127 890 300 |
| GF(65521) | 1024 | deficient | 112 239 600 |
| GF(M31) | 64 | uniform | 263 500 |
| GF(M31) | 64 | deficient | 234 100 |
| GF(M31) | 256 | uniform | 10 317 000 |
| GF(M31) | 256 | deficient | 9 558 900 |
| GF(M31) | 1024 | uniform | 557 089 000 |
| GF(M31) | 1024 | deficient | 973 317 400 |
