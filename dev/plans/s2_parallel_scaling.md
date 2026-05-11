# S2 (jit:4513209c) — Parallel scaling 1..N cores at ≥ 0.85x linear, n ≥ 28

**Status:** measured 2026-05-11. All scaling criteria pass.
**Sibling:** S1 (50× single-thread speedup at n=36), S3 (cross-CPU portability).
**Consumes:** T15 (`permanent_bipedal3_parallel`).

This document records the parallel-scaling measurement required by T15-S2 success criterion 1–5 of the gf2-algebra permanent epic (`ae82bd73`). It corresponds to CLAUDE.md success criterion 5 / epic success criterion 5.

## Methodology

- **Function under test:** `gf2_algebra::permanent::permanent_bipedal3_parallel`, which is the rayon-parallel Ryser permanent over F_3 built on the bipedal3 packed encoding (paper §2.2 / Theorem 2.1). The chunk-size default is `CHUNK_SUBSETS = 1 << 16` (justified by the T15 chunk-sweep CSV).
- **Harness:** `crates/gf2-algebra/examples/parallel_scaling_sweep.rs`. For each `n ∈ {28, 32, 36}` and `T ∈ {1, 2, 4, 8, 12}` the harness:
  1. Builds a per-(n, T) `rayon::ThreadPoolBuilder::new().num_threads(T).build()` pool.
  2. Generates `K` independent matrices via `gf2_algebra::testutil::random_matrix::<3>(n, seed)` where the seed varies per sample but is a deterministic function of `n` plus the JIT short ID (so same n produces same matrix sequence regardless of thread count).
  3. Times each `pool.install(|| permanent_bipedal3_parallel(&mat))` call via `std::time::Instant`.
  4. Records the resulting `Fp<3>` value alongside the timing so determinism can be cross-checked across thread counts.
- **Sample counts:** `K = 5` for `n ∈ {28, 32}`, `K = 3` for `n = 36` (n=36 T=1 is ~148 s/sample; 5 samples × 167 s would exceed the slow-tier budget envelope; 3 samples is enough for the worst-case ratio + standard-deviation bound).
- **Statistics:** mean and (Bessel-corrected) sample standard deviation are reported per (n, T) cell. The scaling factor reported per (n, T) is `T_1 / (T × T_T)` where `T_T` is the mean wall-clock at `T` threads.

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

### n = 28 — total subsets ≈ 268 M

| T | mean (ms) | std (ms) | scaling factor | criterion ≥ 0.85 |
|---:|---:|---:|---:|:---:|
| 1  | 577.12 | 4.07 | 1.0000 | — |
| 2  | 290.47 | 1.63 | 0.9934 | ✓ |
| 4  | 149.91 | 1.70 | 0.9624 | ✓ |
| 8  |  78.99 | 1.68 | 0.9133 | ✓ |
| 12 |  53.70 | 0.11 | 0.8956 | ✓ |

All five thread-count samples produced `Fp<3>(0x1)`; determinism holds.

### n = 32 — total subsets ≈ 4.29 G

| T | mean (ms) | std (ms) | scaling factor | criterion ≥ 0.85 |
|---:|---:|---:|---:|:---:|
| 1  | 9183.20 | 19.00 | 1.0000 | — |
| 2  | 4608.12 |  7.19 | 0.9964 | ✓ |
| 4  | 2383.11 |  2.63 | 0.9634 | ✓ |
| 8  | 1242.55 |  0.42 | 0.9238 | ✓ |
| 12 |  860.11 |  2.49 | 0.8897 | ✓ |

All five thread-count samples produced `Fp<3>(0x2)`; determinism holds.

### n = 36 — total subsets ≈ 68.7 G

| T | mean (s) | std (s) | scaling factor | criterion ≥ 0.85 |
|---:|---:|---:|---:|:---:|
| 1  | 147.029 | 0.048 | 1.0000 | — |
| 2  |  73.975 | 0.025 | 0.9938 | ✓ |
| 4  |  38.279 | 0.029 | 0.9602 | ✓ |
| 8  |  19.980 | 0.029 | 0.9199 | ✓ |
| 12 |  13.903 | 0.023 | 0.8813 | ✓ |

All three thread-count samples produced `Fp<3>(0x1)`; determinism holds.

## Worst-case scaling

| n | worst T | worst scaling factor | criterion ≥ 0.85 |
|---:|---:|---:|:---:|
| 28 | 12 | 0.8956 | ✓ |
| 32 | 12 | 0.8897 | ✓ |
| 36 | 12 | 0.8813 | ✓ |

Across the entire (n ∈ {28, 32, 36}) × (T ∈ {2, 4, 8, 12}) grid the worst observed factor is **0.8813** at the n=36, T=12 cell — comfortably above the 0.85 contract.

## Determinism

Per the methodology above, the same `(n, seed)` produces the same matrix regardless of thread count. The harness records the `Fp<3>` output of every call. Within each `n`-row block of the CSV all `fp3_result_hex` columns are identical:

- n=28: every cell `0x1`.
- n=32: every cell `0x2`.
- n=36: every cell `0x1`.

This satisfies criterion 5 (determinism: output across thread counts is bit-identical at the same seed).

## Scaling-shape commentary

The observed efficiency drop is gradual and tracks Amdahl's law plus shared-cache contention rather than parallel-algorithm overhead:

- **T = 2** retains ≥ 0.993 efficiency at all three n. Two threads fully fit in the L3 + LLC slice budget; the chunk default `2^16` is large enough that rayon scheduler overhead is amortised.
- **T = 4 → T = 8** efficiency drops from ~0.96 to ~0.92 — consistent with crossing one CCX boundary on Zen 3 (each CCX has 6 cores with shared L3).
- **T = 12** loses an extra ~0.03–0.04 of efficiency, attributable to two CCX boundaries plus SMT-adjacent threads competing for the same execution port.

None of this approaches the 0.85 floor, so the contract holds at the physical-core ceiling. SMT scaling beyond T=12 (i.e. T=16, T=24) is not measured here — the criterion is "up to the host's physical core count", which is exactly 12 on Zen 3 5900X.

## Conclusion

**PASS** for criterion 1 (CSV at the named path with n ∈ {28,32,36} × T ∈ {1,2,4,8,12}),
**PASS** for criterion 2 (worst-case factor 0.8813 > 0.85 at every required cell),
**PASS** for criterion 3 (hardware/rayon/seed recorded in CSV header),
**PASS** for criterion 4 (this writeup attached via `jit doc add`),
**PASS** for criterion 5 (bit-identical output across thread counts at the same seed).

All five hard criteria are met.

## Reproduce

```bash
# Run the sweep (≈ 15 min on the dev host):
cargo run -p gf2-algebra --release --features "parallel test-support" \
  --example parallel_scaling_sweep

# Inspect the CSV:
cat dev/benchmarks/gf2_algebra_permanent/s2_parallel_scaling-$(date -u +%F).csv

# Override the output date:
SA_DATE=2026-05-11 cargo run -p gf2-algebra --release \
  --features "parallel test-support" --example parallel_scaling_sweep
```

The harness is deterministic at the LCG seed level (`gf2_core::rng::Lcg`); identical reruns produce identical timings up to wall-clock noise.
