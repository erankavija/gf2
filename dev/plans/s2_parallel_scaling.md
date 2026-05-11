# S2 (jit:4513209c) — Parallel scaling 1..N cores at ≥ 0.85x linear, n ≥ 28

**Status:** measured 2026-05-11 (with two-sided 95% CI). All scaling criteria pass at the lower CI bound.
**Sibling:** S1 (50× single-thread speedup at n=36), S3 (cross-CPU portability).
**Consumes:** T15 (`permanent_bipedal3_parallel`).

This document records the parallel-scaling measurement required by T15-S2 success criterion 1–5 of the gf2-algebra permanent epic (`ae82bd73`). It corresponds to CLAUDE.md success criterion 5 / epic success criterion 5.

## Methodology

- **Function under test:** `gf2_algebra::permanent::permanent_bipedal3_parallel`, the rayon-parallel Ryser permanent over `F_3` built on the bipedal3 packed encoding (paper §2.2 / Theorem 2.1). The chunk-size default is `CHUNK_SUBSETS = 1 << 16` (justified by the T15 chunk-sweep CSV).
- **Harness:** `crates/gf2-algebra/examples/parallel_scaling_sweep.rs`. For each `n ∈ {28, 32, 36}` and `T ∈ {1, 2, 4, 8, 12}`:
  1. Draw `K` independent matrices from a deterministic LCG seeded by `seed_base_n ^ k` (`k = 0..K-1`). The SAME K matrices are timed at every thread count, so per-matrix scaling factors stay paired.
  2. Build a per-`T` `rayon::ThreadPoolBuilder::new().num_threads(T).build()` pool.
  3. Time each `pool.install(|| permanent_bipedal3_parallel(&mat))` call via `std::time::Instant`. Record the `Fp<3>` value alongside the timing for the determinism cross-check.
  4. Per-matrix scaling factor at `(n, T)`: `f[k][T] = T_1[k] / (T × T_T[k])`. Aggregate across the K matrices to a mean and a two-sided 95% CI on the mean, using Student's t with `df = K - 1`.
- **Sample counts (K):** `K = 5` for `n ∈ {28, 32}`, `K = 3` for `n = 36` (n=36 T=1 is ~150 s/matrix; K=3 keeps the n=36 sweep inside ~12 minutes).
- **Acceptance contract:** for each `(n ≥ 28, T ∈ {2, 4, 8, 12})` cell, the **lower** 95% CI bound of the scaling factor must be `≥ 0.85`. Point estimate alone is insufficient; the contract is "within 95% CI" per JIT issue 4513209c success criterion 2.
- **Determinism:** each of the K matrices produces an `Fp<3>` permanent that must be byte-for-byte identical across all thread counts. The harness asserts this at runtime and panics on mismatch.

## Hardware fingerprint

| | |
|---|---|
| CPU model | AMD Ryzen 9 5900X 12-Core Processor |
| Microarchitecture | Zen 3 |
| Physical cores | 12 |
| SMT | 2× (24 logical CPUs) |
| AVX2 | yes |
| AVX-512 | no |
| rayon version | 1.11.0 |

(`lscpu`-style fingerprint also recorded in the CSV header.)

## Results

CSV: `dev/benchmarks/gf2_algebra_permanent/s2_parallel_scaling-2026-05-11.csv`.

### n = 28 — total subsets ≈ 268 M, K = 5

| T  | mean (ms) | scaling factor | 95% CI lo | 95% CI hi | criterion ≥ 0.85 (lo) |
|---:|---:|---:|---:|---:|:---:|
| 1  | 574.95 | 1.0000 | 1.0000 | 1.0000 | — |
| 2  | 291.38 | 0.9866 | 0.9786 | 0.9946 | ✓ |
| 4  | 151.16 | 0.9510 | 0.9383 | 0.9638 | ✓ |
| 8  |  78.57 | 0.9150 | 0.8953 | 0.9347 | ✓ |
| 12 |  54.25 | 0.8833 | 0.8665 | 0.9001 | ✓ |

All 5 matrices' `Fp<3>` output was `0x1` at every thread count; determinism holds.

### n = 32 — total subsets ≈ 4.29 G, K = 5

| T  | mean (ms) | scaling factor | 95% CI lo | 95% CI hi | criterion ≥ 0.85 (lo) |
|---:|---:|---:|---:|---:|:---:|
| 1  | 9201.44 | 1.0000 | 1.0000 | 1.0000 | — |
| 2  | 4616.96 | 0.9965 | 0.9924 | 1.0006 | ✓ |
| 4  | 2384.14 | 0.9649 | 0.9606 | 0.9691 | ✓ |
| 8  | 1244.15 | 0.9245 | 0.9204 | 0.9286 | ✓ |
| 12 |  858.65 | 0.8930 | 0.8892 | 0.8969 | ✓ |

All 5 matrices' `Fp<3>` output was `0x2` at every thread count; determinism holds.

### n = 36 — total subsets ≈ 68.7 G, K = 3

| T  | mean (s) | scaling factor | 95% CI lo | 95% CI hi | criterion ≥ 0.85 (lo) |
|---:|---:|---:|---:|---:|:---:|
| 1  | 147.005 | 1.0000 | 1.0000 | 1.0000 | — |
| 2  |  73.949 | 0.9940 | 0.9924 | 0.9955 | ✓ |
| 4  |  38.281 | 0.9600 | 0.9579 | 0.9622 | ✓ |
| 8  |  19.969 | 0.9202 | 0.9165 | 0.9238 | ✓ |
| 12 |  13.891 | 0.8819 | 0.8784 | 0.8854 | ✓ |

All 3 matrices' `Fp<3>` output was `0x1` at every thread count; determinism holds.

## Worst-case scaling

| n  | worst T | scaling factor | 95% CI lo | margin over 0.85 |
|---:|---:|---:|---:|---:|
| 28 | 12 | 0.8833 | 0.8665 | +0.0165 |
| 32 | 12 | 0.8930 | 0.8892 | +0.0392 |
| 36 | 12 | 0.8819 | 0.8784 | +0.0284 |

Across the entire `(n ∈ {28, 32, 36}) × (T ∈ {2, 4, 8, 12})` grid the **lowest lower-95%-CI bound** is **0.8665** at the n=28, T=12 cell — still **above** the 0.85 contract by 0.0165 (~ 2σ on the per-cell estimator).

## Determinism

Per the methodology above, each of the K matrices is timed at every T, and the per-matrix `Fp<3>` output is asserted equal across T at runtime. Within each `n`-row block of the CSV all `fp3_result_hex` columns are identical:

- n=28: every cell `0x1`.
- n=32: every cell `0x2`.
- n=36: every cell `0x1`.

This satisfies criterion 5 (determinism: output across thread counts is bit-identical at the same seed).

## Scaling-shape commentary

The observed efficiency drop is gradual and tracks Amdahl's law plus shared-cache contention rather than parallel-algorithm overhead:

- **T = 2** retains ≥ 0.987 efficiency at all three n. Two threads fully fit in the L3 + LLC slice budget; the chunk default `2^16` is large enough that rayon scheduler overhead is amortised.
- **T = 4 → T = 8** efficiency drops from ~0.96 to ~0.92, consistent with crossing one CCX boundary on Zen 3 (each CCX has 6 cores with shared L3).
- **T = 12** loses an extra ~0.03–0.04 of efficiency, attributable to two CCX boundaries plus SMT-adjacent threads competing for the same execution port.

The CI widths are smallest at n=32 (~0.005) and widest at n=28 (~0.017), as expected: smaller `n` has shorter wall-clock and a larger fraction of timing noise. None of the lower bounds approach the 0.85 floor at any T ≤ 12. SMT scaling beyond T=12 (i.e. T=16, T=24) is not measured here — the criterion is "up to the host's physical core count", which is exactly 12 on Zen 3 5900X.

## Conclusion

**PASS** for criterion 1 (CSV at the named path with `n ∈ {28,32,36} × T ∈ {1,2,4,8,12}` and 95%-CI columns).
**PASS** for criterion 2 (lower 95% CI bound ≥ 0.85 at every `(n ≥ 28, T ∈ {2,4,8,12})` cell; worst case 0.8665 at n=28, T=12).
**PASS** for criterion 3 (hardware/rayon/seed-base recorded in CSV header).
**PASS** for criterion 4 (this writeup attached via `jit doc add`).
**PASS** for criterion 5 (bit-identical output across thread counts at the same per-matrix seed; verified at runtime by the harness, recorded in the CSV).

All five hard criteria are met **within 95% CI** as the issue text requires.

## Reproduce

```bash
# Run the K-matrix sweep (≈ 15 min on the dev host):
cargo run -p gf2-algebra --release --features "parallel test-support" \
  --example parallel_scaling_sweep

# Inspect the CSV:
cat dev/benchmarks/gf2_algebra_permanent/s2_parallel_scaling-$(date -u +%F).csv

# Override the output date:
SA_DATE=2026-05-11 cargo run -p gf2-algebra --release \
  --features "parallel test-support" --example parallel_scaling_sweep
```

The harness is deterministic at the LCG seed level (`gf2_core::rng::Lcg`); identical reruns produce identical timings up to wall-clock noise. The 95%-CI lookup table `t_critical_95` covers the supported `K - 1 ∈ {2, 4}` degrees of freedom; adding more `n` cells with different K requires extending it.
