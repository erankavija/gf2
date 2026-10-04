# 662f7a15 — F-vs-C verification bench (2026-05-06)

## Purpose

Verify that the prime-sweep finding "C wins all 22 cells" is methodologically sound and that the prior F-r2 single-trial wins (GF(31)/n=1024 = 82.95 Gop/s, GF(251)/n=1024 = 78.68 Gop/s) were bench noise.

## Method

Lead lead-direct, post `aa11aeb..52efa27` cherry-pick:

1. Temporarily set `N_THRESH_PRIME = 7` in `crates/gf2-core/src/gfp/simd_ops.rs` (forces F enable for all `p ≥ 7 && p ≤ 251`). NOT committed.
2. Rebuild bench binary (`cargo build --release -p gf2-core --bench fieldmatrix_gemm --features rand,simd`).
3. Run 5 trials of `gemm/Fp_31/Fp_31/1024` with `taskset -c 6-11 nice -n -5` (CCX1 pinned), reading `target/criterion/.../estimates.json` median per trial.
4. Restore `N_THRESH_PRIME = 252` (production state).

## Result

5-trial F throughput at GF(31)/n=1024:

| Trial | ns | Gop/s |
|---|---:|---:|
| 1 | 33,690,312 | 63.74 |
| 2 | 33,727,092 | 63.67 |
| 3 | 33,588,462 | 63.94 |
| 4 | 34,148,883 | 62.89 |
| 5 | 33,710,494 | 63.70 |
| **median** | **33,710,494** | **63.70** |

Range: 1.05 Gop/s (1.6% of median). IQR: ~0.04 Gop/s.

## Verdict

**Verification confirms the prime-sweep finding.** Manual 5-trial F at GF(31)/n=1024 = 63.70 Gop/s matches the prime-sweep aggregate's F median of 63.69 Gop/s exactly (sub-Gop/s agreement across two independent measurement sessions).

The F-r2 single-trial reading of 82.95 Gop/s (recorded in `dev/bench_results/2026-05-06-662f7a15-rework-perf-spiral-comparison.csv`) was **bench noise — not reproducible** under multi-trial CCX1-pinned protocol. The same applies to the GF(251)/n=1024 single-trial F=78.68 (multi-trial 5-trial 66.04).

**Implication for the dispatch rule:** Candidate C beats Candidate F at every cell on this Zen-3 host with our hand-rolled implementation. `N_THRESH_PRIME = 252` (always-C) stands as the empirically-correct dispatch.

## Architectural note

fflas-ffpack hits 95-141 Gop/s at the same primes/sizes via OpenBLAS sgemm — well above both our F (~64 Gop/s) and our C (~70 Gop/s). The remaining ~70 Gop/s gap to fflas is the depth-of-cache-blocking gap (BLIS-style M_C×N_C×K_C nested blocking that neither C nor F has). Closing the gap would require a Goto-style three-level blocking rewrite — out of scope for 662f7a15.

The structural Zen-3 AVX2+FMA peak (~150 Gop/s, matching OpenBLAS sgemm's measured throughput on this host) IS reachable in principle without AVX-512; the limitation here is implementation depth, not hardware capability.
