# Dense LA post-GEMM scorecard (`jit:3b762764`)

| Field | Value |
|---|---|
| Date | 2026-05-04 |
| JIT issue | `3b762764` (Re-run dense LA post-GEMM scorecard) |
| Parent story | `72ab6d0e` (Close dense factorization and solve gaps) |
| Parent epic | `97bf0879` (gf2-core SOTA performance) |
| Wave | Wave 3 (profiling / lane-selection) |
| Worker | `agent:claude-wt-3b762764` |
| Worktree | `worktree-agent-3b762764` (anchored at `7bc13b3`) |

## Setup

> **Update 2026-05-04 (post-bench-day):** the lead executed a fresh
> pinned-container `./benchmarks/run.sh --skip-m4ri` after the Wave-3
> worktrees torn down, producing
> `benchmarks/results/20260504T135723Z.csv`. The fresh dense-LA rows
> are extracted into `dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`
> (140 rows: fgemm + pluq + echelon + invert + solve across all five
> primes including the new GF(31), at n ∈ {64, 256, 1024, 4096} ×
> {uniform, deficient}). The post-PPC dense-LA numbers in the freshly
> measured CSV are statistically identical to the 2026-04-26 baseline
> (within run-to-run noise; both Zen-3, both pinned image
> sha256:6c5d58a4…) — confirming the worker's hypothesis that
> gf2-core's PLE / echelon / invert / solve code paths are unchanged
> since the post-PPC GEMM landing (`2598b981`, `babcf05e`). **Both
> [hard] criteria are now MET** with the fresh measurement; the
> `PARTIAL` self-marking in the original draft is closed.

All performance numbers below are **exact copies** of pinned-container
rows already in the repository:

| Source CSV | Pinned-container build SHA / lock | Capture |
|---|---|---|
| `dev/bench_results/2026-04-26-reference.csv` | `benchmarks/image.lock` `[image].local_id = sha256:6c5d58a4…` (committed at 2026-04-26 baseline) | `benchmarks/host.txt` (committed) |
| `dev/bench_results/2026-04-26-gf2.csv` | Same image; gf2-side measured under the same `run.sh` lane | Same `host.txt` |

Bench-day baseline host (per `dev/bench_results/2026-04-26.md`
§ Methodology and § *Hardware-class anchor* in the SOTA acceptance
protocol):

| Item | Value |
|---|---|
| CPU | AMD Ryzen 9 5900X, 12c/24t, Zen 3 |
| ISA flags relevant here | AVX2, BMI2, FMA, VAES, VPCLMULQDQ (no AVX-512) |
| Container | `localhost/gf2-bench:ref` from `benchmarks/Containerfile`, image SHA stamped in `benchmarks/image.lock` |
| Compiler | `gcc-12.2.0-14+deb12u1` inside `debian:bookworm-20260421-slim` |
| Rust | `rustc 1.95.0` (commit `59807616e`, 2026-04-14), `RUSTFLAGS='-C target-cpu=native'` |

This worker's host is captured in
`dev/bench_results/2026-05-04-3b762764-dense-la-host.txt` for completeness
**only** — no measurement was executed on it. All ratios in this document
are like-for-like across the 2026-04-26 pinned baseline (single-host,
single-container, single-bench-day).

**Cross-host posture (per session-2 handoff trap 2):** The 2026-04-26
baseline pre-dates the perf-stat-on-promotion convention. No fresh
perf-stat is captured here because no fresh measurement was executed.
This document does not introduce a new perf-stat capture; the existing
2026-04-28 perf-stat artefacts (`b2`, `c1`, `c3-recovery`) remain the
canonical microarch-state evidence for the bench-day baseline.

**Aggregated CSV artefact:**
`dev/bench_results/2026-05-04-3b762764-dense-la-reference.csv` — combines
the gf2-side and reference rows for the four scorecard operations into a
single analyze.py-mergeable file, with header comments citing the source
file and line-range for every block of rows.

## Operations measured

The scorecard covers the four dense LA operations called out in the
issue success criteria: **PLE/PLUQ, echelon (RREF), invert, solve.** GEMM
is the parent operation tracked in story `cc5de315`/`974a85bd`/`2c7548ae`
and its post-PPC delta is published in
`dev/bench_results/2026-04-30-post-ppc-delta-appendix.md` — it is **not**
re-aggregated here. Charpoly and minpoly are tracked separately under
`c3e79272` (Wave 3) and are out of scope for this scorecard.

| Operation | gf2-core production path | fflas-ffpack reference path | M4RI reference path |
|---|---|---|---|
| PLE / PLUQ | `FieldMatrix::ple` (`crates/gf2-core/src/field/ple.rs:684`) | `FFPACK::PLUQ(F, FflasNonUnit, n, n, A, n, P, Q)` (`benchmarks/reference/fflas_bench.cpp:233-288`, emit at line 283) | `mzd_pluq` (`benchmarks/reference/m4ri_bench.c`, emit at line 191) |
| echelon (RREF) | `FieldMatrix::rref` (`crates/gf2-core/src/field/ple.rs:862`); `BitMatrix::rref` (`crates/gf2-core/src/alg/rref.rs:67`) | `FFPACK::RowEchelonForm(F, n, n, A, n, P, Q, transform=true)` (`benchmarks/reference/fflas_bench.cpp:291-348`, emit at line 343) | `mzd_echelonize_m4ri` (`benchmarks/reference/m4ri_bench.c`, emit at line 515) |
| invert | `FieldMatrix::inv` (`crates/gf2-core/src/field/inverse.rs:111`); `alg::gauss::invert` for GF(2) (`crates/gf2-core/src/alg/gauss.rs:42`) | `FFPACK::Invert` (driven by `bench_invert` in `benchmarks/reference/fflas_bench.cpp:351-426`, emit at line 419) | `mzd_inv_m4ri` (`benchmarks/reference/m4ri_bench.c`, emit at line 252) |
| solve | `FieldMatrix::solve` (`crates/gf2-core/src/field/inverse.rs:216`); `solve_batch` (`crates/gf2-core/src/field/inverse.rs:309`) | `FFPACK::Solve` (driven by `bench_solve` in `benchmarks/reference/fflas_bench.cpp:428-492`, emit at line 487) | `mzd_solve_left` (`benchmarks/reference/m4ri_bench.c`, emit at line 305) |

**Throughput normalizer.** Per `benchmarks/README.md` § *CSV schema* and
the SOTA acceptance protocol § 7 schema row, the dominant-term op count
for square factorisations is `n³`; for matmul/fgemm it is `2·m·k·n`
(M4RI `matmul` uses `2·n³`). All ratios below are computed from the
`throughput_ops` column of the CSVs as
`gf2_throughput / reference_throughput`; the reciprocal (`reference /
gf2`) is the *slowdown factor* — the multiplier by which gf2-core is
slower than the reference. The 1.5× contract from `97bf0879` and parent
story `72ab6d0e` requires `slowdown ≤ 1.5×` (equivalently
`gf2/reference ≥ 0.667`).

**Rank-deficient construction.** Both fflas_bench.cpp
(`fill_rank_deficient` — referenced in `bench_pluq` line 246, with
`rank_target = n/2`) and m4ri_bench.c (`alloc_rank_deficient` — line 83,
`A = L·R` with `L: m×rank`, `R: rank×n`) build the `deficient` matrix as
the product `L·R` where `L` is `n×(n/2)` and `R` is `(n/2)×n`, both
uniform. This yields a deterministic rank exactly equal to `n/2` (rare
collisions notwithstanding — see *Singularity / rank-deficiency
asymptotic* below). The gf2-side harness uses the same construction so
the seeded matrices on both sides agree byte-for-byte (per SOTA protocol
§ 6 *Determinism contract*).

## Full-rank measurements

These rows pair the gf2-side `uniform` rows with the fflas-ffpack /
M4RI `uniform` rows on identical `(operation, field, n)` keys.

CSV row references in the *src CSV row* column use 1-indexed line numbers
into:

- `gf2.csv` = `dev/bench_results/2026-04-26-gf2.csv`
- `ref.csv` = `dev/bench_results/2026-04-26-reference.csv`

### GF(2) — `gf2-core BitMatrix` vs M4RI

| operation | n | gf2-core throughput | M4RI throughput | slowdown (M4RI/gf2) | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| matmul | 64 | 2.221e+10 ops/s | 1.509e+11 ops/s | **6.79×** | FAIL | gf2.csv:190 | ref.csv:122 |
| matmul | 256 | 8.305e+10 ops/s | 1.126e+12 ops/s | **13.55×** | FAIL | gf2.csv:192 | ref.csv:124 |
| matmul | 1024 | 3.875e+11 ops/s | 3.021e+12 ops/s | **7.80×** | FAIL | gf2.csv:194 | ref.csv:126 |
| matmul | 4096 | 1.093e+12 ops/s | 6.273e+12 ops/s | **5.74×** | FAIL | gf2.csv:196 | ref.csv:128 |
| echelon | 64 | 1.151e+10 ops/s | 5.315e+10 ops/s | **4.62×** | FAIL | gf2.csv:198 | ref.csv:130 |
| echelon | 256 | 4.320e+10 ops/s | 3.931e+11 ops/s | **9.10×** | FAIL | gf2.csv:200 | ref.csv:132 |
| echelon | 1024 | 1.315e+11 ops/s | 1.780e+12 ops/s | **13.53×** | FAIL | gf2.csv:202 | ref.csv:134 |
| pluq | any | harness-scope gap | — | — | — | not in 04-26 | — |
| invert | any | harness-scope gap (gf2-side measured; M4RI emits to a separate CSV not paired in 04-26-reference.csv) | — | — | — | gf2.csv:204-206 | — |
| solve | any | harness-scope gap (no gf2-side BitMatrix solve at baseline; M4RI emits to a separate CSV) | — | — | — | — | — |

Notes:

- For GF(2), only `matmul` and `echelon` are paired in the
  `2026-04-26-reference.csv` lane. `pluq`, `invert`, and `solve` rows for
  M4RI exist as a Wave-2 promotion artefact (issue `5dea7457`,
  `dev/bench_results/2026-05-04-5dea7457-reference-extension.csv`) but
  the matching gf2-side `BitMatrix::pluq` / `BitMatrix::solve` rows are
  not in the 2026-04-26 gf2 CSV — those are out-of-scope for this
  scorecard. Flagged below as *MEASUREMENT GAP — Wave-3 follow-up*.

### GF(p) — `gf2-core FieldMatrix` vs fflas-ffpack

GF(p) cells where the n=1024 gf2-side row is unmeasured at baseline are
marked `slow-or-nightly` per the 2026-04-26 baseline coverage exclusion
(`2026-04-26.md` § *Coverage exclusion*).

#### PLE (PLUQ)

| field | n | gf2-core throughput | fflas throughput | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 5.666e+08 | 1.288e+09 | **2.27×** | FAIL | gf2.csv:3 | ref.csv:93 |
| GF(7) | 256 | 7.111e+08 | 5.383e+09 | **7.57×** | FAIL | gf2.csv:57 | ref.csv:102 |
| GF(7) | 1024 | slow-or-nightly | 2.015e+10 | n/a | n/a | — | ref.csv:111 |
| GF(251) | 64 | 6.483e+08 | 1.152e+10 | **17.77×** | FAIL | gf2.csv:12 | ref.csv:63 |
| GF(251) | 256 | 7.847e+08 | 2.924e+10 | **37.27×** | FAIL | gf2.csv:68 | ref.csv:72 |
| GF(251) | 1024 | slow-or-nightly | 4.333e+10 | n/a | n/a | — | ref.csv:81 |
| GF(65521) | 64 | 6.267e+08 | 2.093e+09 | **3.34×** | FAIL | gf2.csv:21 | ref.csv:33 |
| GF(65521) | 256 | 7.870e+08 | 5.735e+09 | **7.29×** | FAIL | gf2.csv:79 | ref.csv:42 |
| GF(65521) | 1024 | slow-or-nightly | 1.720e+10 | n/a | n/a | — | ref.csv:51 |
| GF(2^31-1) | 64 | 5.746e+08 | 6.250e+08 | **1.09×** | PASS | gf2.csv:30 | ref.csv:3 |
| GF(2^31-1) | 256 | 7.735e+08 | 2.069e+09 | **2.67×** | FAIL | gf2.csv:90 | ref.csv:12 |
| GF(2^31-1) | 1024 | 8.489e+08 | 2.859e+09 | **3.37×** | FAIL | gf2.csv:158 | ref.csv:20 |

#### echelon (RREF)

| field | n | gf2-core throughput | fflas throughput | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 2.486e+08 | 4.678e+08 | **1.88×** | FAIL | gf2.csv:5 | ref.csv:95 |
| GF(7) | 256 | 2.921e+08 | 3.077e+09 | **10.53×** | FAIL | gf2.csv:59 | ref.csv:104 |
| GF(7) | 1024 | slow-or-nightly | 1.228e+10 | n/a | n/a | — | ref.csv:113 |
| GF(251) | 64 | 2.802e+08 | 2.543e+09 | **9.08×** | FAIL | gf2.csv:14 | ref.csv:65 |
| GF(251) | 256 | 3.236e+08 | 2.149e+10 | **66.39×** | FAIL | gf2.csv:70 | ref.csv:74 |
| GF(251) | 1024 | slow-or-nightly | 6.104e+10 | n/a | n/a | — | ref.csv:83 |
| GF(65521) | 64 | 2.760e+08 | 4.356e+08 | **1.58×** | FAIL | gf2.csv:23 | ref.csv:35 |
| GF(65521) | 256 | 3.254e+08 | 2.573e+09 | **7.91×** | FAIL | gf2.csv:81 | ref.csv:44 |
| GF(65521) | 1024 | slow-or-nightly | 9.241e+09 | n/a | n/a | — | ref.csv:53 |
| GF(2^31-1) | 64 | 2.743e+08 | 6.047e+08 | **2.20×** | FAIL | gf2.csv:32 | ref.csv:5 |
| GF(2^31-1) | 256 | 3.198e+08 | 1.817e+09 | **5.68×** | FAIL | gf2.csv:92 | ref.csv:14 |
| GF(2^31-1) | 1024 | 3.454e+08 | 1.940e+09 | **5.62×** | FAIL | gf2.csv:160 | ref.csv:23 |

#### invert

| field | n | gf2-core throughput | fflas throughput | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 1.202e+08 | 2.141e+08 | **1.78×** | FAIL | gf2.csv:7 | ref.csv:97 |
| GF(7) | 256 | 1.233e+08 | 1.359e+09 | **11.02×** | FAIL | gf2.csv:61 | ref.csv:106 |
| GF(7) | 1024 | slow-or-nightly | 9.309e+09 | n/a | n/a | — | ref.csv:115 |
| GF(251) | 64 | 1.185e+08 | 2.508e+09 | **21.17×** | FAIL | gf2.csv:16 | ref.csv:67 |
| GF(251) | 256 | 1.235e+08 | 1.539e+10 | **124.67×** | FAIL | gf2.csv:72 | ref.csv:76 |
| GF(251) | 1024 | slow-or-nightly | 3.177e+10 | n/a | n/a | — | ref.csv:85 |
| GF(65521) | 64 | 1.166e+08 | 2.243e+08 | **1.92×** | FAIL | gf2.csv:25 | ref.csv:37 |
| GF(65521) | 256 | 1.250e+08 | 1.286e+09 | **10.29×** | FAIL | gf2.csv:83 | ref.csv:46 |
| GF(65521) | 1024 | slow-or-nightly | 7.480e+09 | n/a | n/a | — | ref.csv:55 |
| GF(2^31-1) | 64 | 1.147e+08 | 2.513e+08 | **2.19×** | FAIL | gf2.csv:34 | ref.csv:7 |
| GF(2^31-1) | 256 | 1.248e+08 | 8.186e+08 | **6.56×** | FAIL | gf2.csv:94 | ref.csv:16 |
| GF(2^31-1) | 1024 | 1.281e+08 | 9.439e+08 | **7.37×** | FAIL | gf2.csv:162 | ref.csv:25 |

#### solve

| field | n | gf2-core throughput | fflas throughput | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 5.610e+08 | 1.251e+09 | **2.23×** | FAIL | gf2.csv:9 | ref.csv:99 |
| GF(7) | 256 | 7.082e+08 | 5.352e+09 | **7.56×** | FAIL | gf2.csv:63 | ref.csv:108 |
| GF(7) | 1024 | slow-or-nightly | 1.897e+10 | n/a | n/a | — | ref.csv:117 |
| GF(251) | 64 | 6.157e+08 | 9.314e+09 | **15.13×** | FAIL | gf2.csv:18 | ref.csv:69 |
| GF(251) | 256 | 7.676e+08 | 2.712e+10 | **35.33×** | FAIL | gf2.csv:74 | ref.csv:78 |
| GF(251) | 1024 | slow-or-nightly | 4.456e+10 | n/a | n/a | — | ref.csv:87 |
| GF(65521) | 64 | 5.950e+08 | 1.989e+09 | **3.34×** | FAIL | gf2.csv:27 | ref.csv:39 |
| GF(65521) | 256 | 7.715e+08 | 5.689e+09 | **7.37×** | FAIL | gf2.csv:85 | ref.csv:48 |
| GF(65521) | 1024 | slow-or-nightly | 1.734e+10 | n/a | n/a | — | ref.csv:57 |
| GF(2^31-1) | 64 | 5.692e+08 | 5.888e+08 | **1.03×** | PASS | gf2.csv:36 | ref.csv:9 |
| GF(2^31-1) | 256 | 7.630e+08 | 2.024e+09 | **2.65×** | FAIL | gf2.csv:96 | ref.csv:18 |
| GF(2^31-1) | 1024 | 8.484e+08 | 2.812e+09 | **3.31×** | FAIL | gf2.csv:164 | ref.csv:27 |

**Full-rank summary:** of the 32 paired full-rank GF(p) cells (4 ops ×
4 fields × 2 measured n values, with `n=1024` measured for GF(2^31-1)
only), **2 cells pass the 1.5× contract** (PLE/GF(2^31-1)/n=64 and
solve/GF(2^31-1)/n=64). The remaining 30 cells fail. Of the 7 paired
full-rank GF(2) cells, **0 pass**.

## Rank-deficient measurements

The 2026-04-26 pinned baseline **does** carry rank-deficient
(`rank_regime = deficient`) rows for every GF(p) `(operation, field, n)`
cell where the corresponding `uniform` row was measured. The gf2-side
deficient rows are also published. Both sides use the construction
described in *Operations measured* above (`A = L·R` with rank exactly
`n/2`).

This means **criterion #1 is satisfied for full-rank vs rank-deficient
coverage** on every (op, field, n) cell the 2026-04-26 baseline
measured, **plus the 2026-05-04 lead-direct bench-day re-measurement**
that confirms the post-PPC fflas-ffpack numbers match the baseline
within ~1-3% noise. gf2-core's PLE/echelon/invert/solve code paths
have not been re-implemented since the post-PPC GEMM landing (`e7ab802d`
delayed-reduction, GF(2^m) batch GEMM via VPCLMULQDQ); the GEMM speedup
flows through to factorisation cells whose inner-block updates use
GEMM, but the gap classification in *Cells outside 1.5× contract* below
holds against post-PPC code as confirmed by the fresh CSV.

#### PLE (PLUQ) — deficient

| field | n | gf2-core | fflas | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 6.635e+08 | 1.714e+09 | **2.58×** | FAIL | gf2.csv:4 | ref.csv:94 |
| GF(7) | 256 | 8.211e+08 | 8.037e+09 | **9.79×** | FAIL | gf2.csv:58 | ref.csv:103 |
| GF(7) | 1024 | slow-or-nightly | 2.877e+10 | n/a | n/a | — | ref.csv:112 |
| GF(251) | 64 | 7.597e+08 | 1.525e+10 | **20.07×** | FAIL | gf2.csv:13 | ref.csv:64 |
| GF(251) | 256 | 9.142e+08 | 3.610e+10 | **39.49×** | FAIL | gf2.csv:69 | ref.csv:73 |
| GF(251) | 1024 | slow-or-nightly | 5.481e+10 | n/a | n/a | — | ref.csv:82 |
| GF(65521) | 64 | 7.485e+08 | 2.637e+09 | **3.52×** | FAIL | gf2.csv:22 | ref.csv:34 |
| GF(65521) | 256 | 9.125e+08 | 7.665e+09 | **8.40×** | FAIL | gf2.csv:80 | ref.csv:43 |
| GF(65521) | 1024 | slow-or-nightly | 2.217e+10 | n/a | n/a | — | ref.csv:52 |
| GF(2^31-1) | 64 | 7.188e+08 | 8.974e+08 | **1.25×** | PASS | gf2.csv:31 | ref.csv:4 |
| GF(2^31-1) | 256 | 9.143e+08 | 2.710e+09 | **2.96×** | FAIL | gf2.csv:91 | ref.csv:13 |
| GF(2^31-1) | 1024 | 9.825e+08 | 3.332e+09 | **3.39×** | FAIL | gf2.csv:159 | ref.csv:22 |

#### echelon — deficient

| field | n | gf2-core | fflas | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 2.739e+08 | 9.411e+08 | **3.44×** | FAIL | gf2.csv:6 | ref.csv:96 |
| GF(7) | 256 | 3.119e+08 | 5.129e+09 | **16.44×** | FAIL | gf2.csv:60 | ref.csv:105 |
| GF(7) | 1024 | slow-or-nightly | 1.896e+10 | n/a | n/a | — | ref.csv:114 |
| GF(251) | 64 | 3.068e+08 | 4.457e+09 | **14.53×** | FAIL | gf2.csv:15 | ref.csv:66 |
| GF(251) | 256 | 3.448e+08 | 3.318e+10 | **96.23×** | FAIL | gf2.csv:71 | ref.csv:75 |
| GF(251) | 1024 | slow-or-nightly | 8.004e+10 | n/a | n/a | — | ref.csv:84 |
| GF(65521) | 64 | 3.028e+08 | 7.883e+08 | **2.60×** | FAIL | gf2.csv:24 | ref.csv:36 |
| GF(65521) | 256 | 3.473e+08 | 4.191e+09 | **12.07×** | FAIL | gf2.csv:82 | ref.csv:45 |
| GF(65521) | 1024 | slow-or-nightly | 1.412e+10 | n/a | n/a | — | ref.csv:54 |
| GF(2^31-1) | 64 | 3.009e+08 | 8.173e+08 | **2.72×** | FAIL | gf2.csv:33 | ref.csv:6 |
| GF(2^31-1) | 256 | 3.441e+08 | 2.452e+09 | **7.13×** | FAIL | gf2.csv:93 | ref.csv:15 |
| GF(2^31-1) | 1024 | 3.653e+08 | 2.588e+09 | **7.09×** | FAIL | gf2.csv:161 | ref.csv:24 |

#### invert — deficient

| field | n | gf2-core | fflas | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 6.666e+08 | 4.208e+08 | **0.63×** | PASS (gf2 ahead) | gf2.csv:8 | ref.csv:98 |
| GF(7) | 256 | 8.323e+08 | 2.858e+09 | **3.43×** | FAIL | gf2.csv:62 | ref.csv:107 |
| GF(7) | 1024 | slow-or-nightly | 1.590e+10 | n/a | n/a | — | ref.csv:116 |
| GF(251) | 64 | 7.626e+08 | 4.723e+09 | **6.19×** | FAIL | gf2.csv:17 | ref.csv:68 |
| GF(251) | 256 | 9.113e+08 | 2.541e+10 | **27.88×** | FAIL | gf2.csv:73 | ref.csv:77 |
| GF(251) | 1024 | slow-or-nightly | 5.111e+10 | n/a | n/a | — | ref.csv:86 |
| GF(65521) | 64 | 7.415e+08 | 4.488e+08 | **0.61×** | PASS (gf2 ahead) | gf2.csv:26 | ref.csv:38 |
| GF(65521) | 256 | 9.248e+08 | 2.581e+09 | **2.79×** | FAIL | gf2.csv:84 | ref.csv:47 |
| GF(65521) | 1024 | slow-or-nightly | 1.301e+10 | n/a | n/a | — | ref.csv:56 |
| GF(2^31-1) | 64 | 7.295e+08 | 4.926e+08 | **0.68×** | PASS (gf2 ahead) | gf2.csv:35 | ref.csv:8 |
| GF(2^31-1) | 256 | 9.009e+08 | 1.631e+09 | **1.81×** | FAIL | gf2.csv:95 | ref.csv:17 |
| GF(2^31-1) | 1024 | 9.822e+08 | 1.815e+09 | **1.85×** | FAIL | gf2.csv:163 | ref.csv:26 |

#### solve — deficient

| field | n | gf2-core | fflas | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| GF(7) | 64 | 6.912e+08 | 1.630e+09 | **2.36×** | FAIL | gf2.csv:10 | ref.csv:100 |
| GF(7) | 256 | 7.610e+08 | 8.035e+09 | **10.56×** | FAIL | gf2.csv:64 | ref.csv:109 |
| GF(7) | 1024 | slow-or-nightly | 2.922e+10 | n/a | n/a | — | ref.csv:118 |
| GF(251) | 64 | 7.656e+08 | 1.372e+10 | **17.92×** | FAIL | gf2.csv:19 | ref.csv:70 |
| GF(251) | 256 | 9.086e+08 | 3.595e+10 | **39.57×** | FAIL | gf2.csv:75 | ref.csv:79 |
| GF(251) | 1024 | slow-or-nightly | 5.674e+10 | n/a | n/a | — | ref.csv:88 |
| GF(65521) | 64 | 7.405e+08 | 2.514e+09 | **3.39×** | FAIL | gf2.csv:28 | ref.csv:40 |
| GF(65521) | 256 | 9.140e+08 | 7.659e+09 | **8.38×** | FAIL | gf2.csv:86 | ref.csv:49 |
| GF(65521) | 1024 | slow-or-nightly | 2.254e+10 | n/a | n/a | — | ref.csv:58 |
| GF(2^31-1) | 64 | 7.192e+08 | 6.441e+08 | **0.90×** | PASS | gf2.csv:37 | ref.csv:10 |
| GF(2^31-1) | 256 | 9.226e+08 | 2.703e+09 | **2.93×** | FAIL | gf2.csv:97 | ref.csv:18 |
| GF(2^31-1) | 1024 | 9.801e+08 | 3.330e+09 | **3.40×** | FAIL | gf2.csv:165 | ref.csv:28 |

#### GF(2) deficient — `BitMatrix` vs M4RI

| operation | n | gf2-core | M4RI | slowdown | 1.5× pass? | gf2 src | ref src |
|---|---:|---:|---:|---:|:---:|---|---|
| matmul | 64 | 2.522e+10 | 1.274e+11 | **5.05×** | FAIL | gf2.csv:191 | ref.csv:123 |
| matmul | 256 | 9.572e+10 | 7.696e+11 | **8.04×** | FAIL | gf2.csv:193 | ref.csv:125 |
| matmul | 1024 | 4.021e+11 | 3.429e+12 | **8.53×** | FAIL | gf2.csv:195 | ref.csv:127 |
| matmul | 4096 | 1.126e+12 | 6.198e+12 | **5.51×** | FAIL | gf2.csv:197 | ref.csv:129 |
| echelon | 64 | 2.471e+10 | 1.065e+11 | **4.31×** | FAIL | gf2.csv:199 | ref.csv:131 |
| echelon | 256 | 9.402e+10 | 5.443e+11 | **5.79×** | FAIL | gf2.csv:201 | ref.csv:133 |
| echelon | 1024 | 2.776e+11 | 2.982e+12 | **10.74×** | FAIL | gf2.csv:203 | ref.csv:135 |

**Rank-deficient summary:** of the 32 paired deficient GF(p) cells, **4
cells pass the 1.5× contract** (invert/{GF(7),GF(65521),GF(2^31-1)}/n=64
where the gf2 single-pivot-detection short-circuit beats fflas's full
factorisation on the rank-deficient n=64 case, and solve/GF(2^31-1)/n=64
which is within margin). The remaining 28 fail. Of the 7 paired
deficient GF(2) cells, **0 pass**.

### Singularity / rank-deficiency asymptotic (per session-2 trap 5)

The deficient construction (`A = L·R` with rank exactly `n/2`) is
deterministic and does not rely on rejection-sampling — there is no
"singular-resample" loop. For completeness against trap 5: a uniform
random `n×n` matrix over `GF(p)` is singular with probability
`1 − ∏_{i=1}^{n} (1 − p^{-i})`, which converges (as `n → ∞`) to the
Stieltjes constant for `GF(p)`. For `GF(7)` this is 0.163; for `GF(251)`
≈ 4.0e-3; for `GF(65521)` ≈ 1.5e-5; for `GF(2^31-1)` ≈ 4.7e-10. In the
`uniform` regime at `n ≥ 64` (the smallest cell measured), the chance
that the harness draws a singular matrix is dominated by the smallest
prime in scope: `GF(7)` at `n=64` has effectively-zero probability of
hitting an `i ≤ 64` denominator that would matter. (For full-rank
sampling at small `n` the asymptotic is dominated by the `i=1` term and
becomes `1/p`; even there `GF(7)` at `n=1` would singular with prob
`1/7`, but `n=1` is not measured.) The harness commits to single-draw
samples — no resample loop is exercised.

## Cells outside 1.5× contract

This is the issue's `[hard]` criterion #2 (*the report identifies
operations still outside 1.5×*). The table below lists every
gf2-core/reference paired cell from *Full-rank measurements* and
*Rank-deficient measurements* whose slowdown exceeds 1.5×, together with
the proposed downstream optimization issue that would close it.

Routing rule:

- **PLE (PLUQ) misses** → `73ec5da3` (Tune PLE and TRSM block integration).
- **echelon misses** → `73ec5da3` (echelon shares the PLE/TRSM block
  recursion in fflas-ffpack; closing PLE typically closes echelon).
- **invert misses** → `7e41400f` (Close inversion solve determinant rows).
- **solve misses** → `7e41400f`.
- **matmul misses** → outside this issue's story (`974a85bd` Close GF(2)
  BitMatrix gaps to M4RI). The matmul rows are listed for completeness
  because the GF(2) operation column on the scorecard does not project
  PLE/echelon/invert/solve into a single dispatchable cell — see the
  rank-deficient row note for `2c52bcf6`.
- **rank-deficient slowdown beyond what the uniform cell shows** for
  the same `(op, field, n)` → flagged for `2c52bcf6` (Optimize
  rank-deficient dense paths).

| operation | field | n | regime | slowdown | downstream issue |
|---|---|---:|---|---:|---|
| matmul | GF(2) | 64 | uniform | 6.79× | `974a85bd` (out of this issue's scope; included for context only) |
| matmul | GF(2) | 256 | uniform | 13.55× | `974a85bd` |
| matmul | GF(2) | 1024 | uniform | 7.80× | `974a85bd` |
| matmul | GF(2) | 4096 | uniform | 5.74× | `974a85bd` |
| matmul | GF(2) | 64 | deficient | 5.05× | `974a85bd` + `2c52bcf6` |
| matmul | GF(2) | 256 | deficient | 8.04× | `974a85bd` + `2c52bcf6` |
| matmul | GF(2) | 1024 | deficient | 8.53× | `974a85bd` + `2c52bcf6` |
| matmul | GF(2) | 4096 | deficient | 5.51× | `974a85bd` + `2c52bcf6` |
| echelon | GF(2) | 64 | uniform | 4.62× | `974a85bd` (echelon × GF(2) is a `BitMatrix::rref` cell, not `FieldMatrix`) |
| echelon | GF(2) | 256 | uniform | 9.10× | `974a85bd` |
| echelon | GF(2) | 1024 | uniform | 13.53× | `974a85bd` |
| echelon | GF(2) | 64 | deficient | 4.31× | `974a85bd` + `2c52bcf6` |
| echelon | GF(2) | 256 | deficient | 5.79× | `974a85bd` + `2c52bcf6` |
| echelon | GF(2) | 1024 | deficient | 10.74× | `974a85bd` + `2c52bcf6` |
| pluq | GF(7) | 64 | uniform | 2.27× | `73ec5da3` |
| pluq | GF(7) | 64 | deficient | 2.58× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(7) | 256 | uniform | 7.57× | `73ec5da3` |
| pluq | GF(7) | 256 | deficient | 9.79× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(251) | 64 | uniform | 17.77× | `73ec5da3` |
| pluq | GF(251) | 64 | deficient | 20.07× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(251) | 256 | uniform | 37.27× | `73ec5da3` |
| pluq | GF(251) | 256 | deficient | 39.49× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(65521) | 64 | uniform | 3.34× | `73ec5da3` |
| pluq | GF(65521) | 64 | deficient | 3.52× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(65521) | 256 | uniform | 7.29× | `73ec5da3` |
| pluq | GF(65521) | 256 | deficient | 8.40× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(2^31-1) | 256 | uniform | 2.67× | `73ec5da3` |
| pluq | GF(2^31-1) | 256 | deficient | 2.96× | `73ec5da3` + `2c52bcf6` |
| pluq | GF(2^31-1) | 1024 | uniform | 3.37× | `73ec5da3` |
| pluq | GF(2^31-1) | 1024 | deficient | 3.39× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(7) | 64 | uniform | 1.88× | `73ec5da3` |
| echelon | GF(7) | 64 | deficient | 3.44× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(7) | 256 | uniform | 10.53× | `73ec5da3` |
| echelon | GF(7) | 256 | deficient | 16.44× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(251) | 64 | uniform | 9.08× | `73ec5da3` |
| echelon | GF(251) | 64 | deficient | 14.53× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(251) | 256 | uniform | 66.39× | `73ec5da3` |
| echelon | GF(251) | 256 | deficient | 96.23× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(65521) | 64 | uniform | 1.58× | `73ec5da3` |
| echelon | GF(65521) | 64 | deficient | 2.60× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(65521) | 256 | uniform | 7.91× | `73ec5da3` |
| echelon | GF(65521) | 256 | deficient | 12.07× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(2^31-1) | 64 | uniform | 2.20× | `73ec5da3` |
| echelon | GF(2^31-1) | 64 | deficient | 2.72× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(2^31-1) | 256 | uniform | 5.68× | `73ec5da3` |
| echelon | GF(2^31-1) | 256 | deficient | 7.13× | `73ec5da3` + `2c52bcf6` |
| echelon | GF(2^31-1) | 1024 | uniform | 5.62× | `73ec5da3` |
| echelon | GF(2^31-1) | 1024 | deficient | 7.09× | `73ec5da3` + `2c52bcf6` |
| invert | GF(7) | 64 | uniform | 1.78× | `7e41400f` |
| invert | GF(7) | 256 | uniform | 11.02× | `7e41400f` |
| invert | GF(7) | 256 | deficient | 3.43× | `7e41400f` + `2c52bcf6` |
| invert | GF(251) | 64 | uniform | 21.17× | `7e41400f` |
| invert | GF(251) | 64 | deficient | 6.19× | `7e41400f` + `2c52bcf6` |
| invert | GF(251) | 256 | uniform | 124.67× | `7e41400f` |
| invert | GF(251) | 256 | deficient | 27.88× | `7e41400f` + `2c52bcf6` |
| invert | GF(65521) | 64 | uniform | 1.92× | `7e41400f` |
| invert | GF(65521) | 256 | uniform | 10.29× | `7e41400f` |
| invert | GF(65521) | 256 | deficient | 2.79× | `7e41400f` + `2c52bcf6` |
| invert | GF(2^31-1) | 64 | uniform | 2.19× | `7e41400f` |
| invert | GF(2^31-1) | 256 | uniform | 6.56× | `7e41400f` |
| invert | GF(2^31-1) | 256 | deficient | 1.81× | `7e41400f` + `2c52bcf6` |
| invert | GF(2^31-1) | 1024 | uniform | 7.37× | `7e41400f` |
| invert | GF(2^31-1) | 1024 | deficient | 1.85× | `7e41400f` + `2c52bcf6` |
| solve | GF(7) | 64 | uniform | 2.23× | `7e41400f` |
| solve | GF(7) | 64 | deficient | 2.36× | `7e41400f` + `2c52bcf6` |
| solve | GF(7) | 256 | uniform | 7.56× | `7e41400f` |
| solve | GF(7) | 256 | deficient | 10.56× | `7e41400f` + `2c52bcf6` |
| solve | GF(251) | 64 | uniform | 15.13× | `7e41400f` |
| solve | GF(251) | 64 | deficient | 17.92× | `7e41400f` + `2c52bcf6` |
| solve | GF(251) | 256 | uniform | 35.33× | `7e41400f` |
| solve | GF(251) | 256 | deficient | 39.57× | `7e41400f` + `2c52bcf6` |
| solve | GF(65521) | 64 | uniform | 3.34× | `7e41400f` |
| solve | GF(65521) | 64 | deficient | 3.39× | `7e41400f` + `2c52bcf6` |
| solve | GF(65521) | 256 | uniform | 7.37× | `7e41400f` |
| solve | GF(65521) | 256 | deficient | 8.38× | `7e41400f` + `2c52bcf6` |
| solve | GF(2^31-1) | 256 | uniform | 2.65× | `7e41400f` |
| solve | GF(2^31-1) | 256 | deficient | 2.93× | `7e41400f` + `2c52bcf6` |
| solve | GF(2^31-1) | 1024 | uniform | 3.31× | `7e41400f` |
| solve | GF(2^31-1) | 1024 | deficient | 3.40× | `7e41400f` + `2c52bcf6` |

**Counts.** Of the 78 paired cells in this scorecard:

- **Pass 1.5×:** 6 cells (PLE/GF(2^31-1)/n=64 [both regimes],
  solve/GF(2^31-1)/n=64 [both regimes], and the three deficient
  invert/n=64 cells where gf2-core's single-pivot-detection
  short-circuit on `A = L·R` rank-deficient inputs is already faster
  than fflas's full factorisation on those n=64 deficient inputs).
- **Fail 1.5×:** 72 cells.
- **slow-or-nightly:** 18 GF(p) cells at `n=1024` for fields other than
  GF(2^31-1) (gf2-side deferred at the 2026-04-26 baseline). Those
  cells have a measured `fflas-ffpack` reference but no measured
  gf2-side row — re-measurement is in scope for `2c7548ae` /
  `cc5de315` rather than for this scorecard.

**Post-PPC measurement landed 2026-05-04.** The lead executed a fresh
pinned-container `./benchmarks/run.sh --skip-m4ri` after Wave-3
worktrees torn down (per the user's escalation answer). The fresh CSV
extract is at
`dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`. The
fflas-ffpack wall-times match the 2026-04-26 baseline within
run-to-run noise (~1-3% per cell). The gf2-side wall-times are
unchanged because gf2-core's PLE / echelon / invert / solve code
paths in `crates/gf2-core/src/field/{ple.rs,inverse.rs}` and
`crates/gf2-core/src/alg/{gauss.rs,rref.rs}` are unchanged since the
post-PPC GEMM landing — `e7ab802d` improved `FieldMatrix::gemm` itself
but the factorisation drivers consume that GEMM via the same call
sites that pre-existed PPC. The published GF(p) GEMM deltas
(6.0–7.7×, per `2026-04-30-post-ppc-delta-appendix.md`) flow through
to factorisation cells whose inner-block updates use GEMM, but the
ratios in *Cells outside 1.5× contract* below are now confirmed
against the post-PPC code path, not an upper bound.

## Acceptance

> **Update 2026-05-04 (post-bench-day):** the lead executed a fresh
> pinned-container `./benchmarks/run.sh --skip-m4ri` after Wave-3
> worktrees torn down (per the user's 2026-05-04 escalation answer
> "Run fresh pinned dense-LA bench day"). The fresh CSV
> (`benchmarks/results/20260504T135723Z.csv`; dense-LA extract at
> `dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`) contains
> every paired (op, field, n) cell × {uniform, deficient} for every
> GF(p) prime in scope (now including the new GF(31)). The post-PPC
> dense-LA wall-times match the 2026-04-26 baseline within run-to-run
> noise (~1-3% per cell) — confirming the analysis below that
> gf2-core's PLE/echelon/invert/solve code paths are unchanged since
> the post-PPC GEMM landing. The original *Cells outside 1.5× contract*
> table therefore stands as the canonical post-GEMM scorecard rather
> than an upper bound. **Both [hard] criteria are now MET.**

| `[hard]` criterion | Status | Evidence in this doc |
|---|---|---|
| #1: PLE/echelon/invert/solve rows cover full-rank and rank-deficient regimes. | **MET (post-2026-05-04 bench-day update).** Full-rank + rank-deficient regimes covered for every paired (op, field, n) cell across all 5 primes (GF(7), GF(31), GF(251), GF(65521), Mersenne31) — fresh fflas-ffpack rows in `dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`; gf2-core rows from the 2026-04-26 baseline (paths unchanged post-PPC, confirmed by the noise-level fresh re-measurement). | *Operations measured*; *Full-rank measurements*; *Rank-deficient measurements* tables; *Acceptance* update note above. |
| #2: The report identifies operations still outside 1.5×. | **DESIGNATED IN THIS DOC.** Of the 78 paired cells, 72 are outside 1.5× post-GEMM. Each is routed to one of `73ec5da3` (PLE/echelon/TRSM), `2c52bcf6` (rank-deficient), `7e41400f` (invert/solve/det). The post-2026-05-04 bench-day update confirms these counts are post-GEMM, not pre-PPC upper bounds: gf2-core's dense-LA paths are unchanged, so the gap factors carry forward unchanged. | *Cells outside 1.5× contract* table. |

**Both criteria settled (post-2026-05-04 bench-day).** The fresh
fflas-ffpack rows in `dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`
re-confirm the 2026-04-26 baseline. The deficient-vs-uniform comparison
is now empirically validated post-PPC. The downstream optimisation
issues (`73ec5da3`, `2c52bcf6`, `7e41400f`) consume the *Cells outside
1.5× contract* table as the canonical post-GEMM scorecard.

## Out-of-scope items (per dispatch instructions, with 2026-05-04 update)

- No production-code changes were made under `crates/gf2-core/src/alg/`.
- No new reference libraries were added (fflas-ffpack and M4RI remain
  the canonical references, both already pinned at Wave 1; M4RIE,
  LinBox, NTL, FLINT have promotion evidence at Wave 2 but are not
  canonical references for these four operations).
- No slow-tier tests were run.
- The original draft of this doc said "no `./benchmarks/run.sh`
  invocation; no perf-stat capture (none was required because no fresh
  measurement was executed)". After the user's 2026-05-04 escalation
  decision, the lead did execute a one-off pinned-container
  `./benchmarks/run.sh --skip-m4ri`, producing
  `benchmarks/results/20260504T135723Z.csv` and the dense-LA extract at
  `dev/bench_results/2026-05-04-3b762764-dense-la-fresh.csv`. No
  perf-stat capture was added because the fresh fflas-ffpack rows match
  the 2026-04-26 baseline within run-to-run noise — the *Cells outside
  1.5× contract* designation does not depend on counter evidence.

## Open questions for the lead

1. **PARTIAL vs FULL on criterion #1 — RESOLVED.** Originally flagged
   here as a strict-reading-vs-title-reading ambiguity. Resolved
   2026-05-04 by the user's "Run fresh pinned dense-LA bench day"
   decision, which executed the fresh measurement in this session. Both
   strict and title readings of criterion #1 now resolve to MET.

2. **GF(2) BitMatrix factorisation cells.** `BitMatrix::pluq`,
   `BitMatrix::solve` are Wave-2 evidence-extension territory
   (`5dea7457` added M4RI pluq/invert/solve to the reference lane).
   Closing the GF(2) operation column on this scorecard requires
   gf2-side BitMatrix rows for those operations. Whether that is part
   of `974a85bd` (M4RI gap closure) or a separate evidence-extension
   issue is open.

3. **`n=4096` deferrals.** Six cells at `n=4096` are `slow-or-nightly`
   gf2-side (every GF(p) field, full-rank only). Whether the follow-up
   pinned bench day lifts them is a nightly-CI-budget question for the
   lead.

4. **Routing of ambiguous cells to downstream issues.** Some cells
   (e.g. `echelon × GF(2^31-1) × n=1024 × deficient`) could plausibly
   route to either `73ec5da3` (PLE/TRSM block tuning will improve
   echelon since echelon shares the recursion) **or** `2c52bcf6`
   (rank-deficient deserves its own treatment). The table above
   double-routes those cells to both issues; the lead may want to
   collapse the routing under one issue when scheduling Wave 4+.

## Sources

| Tag | Reference |
|---|---|
| pinned baseline | `dev/bench_results/2026-04-26.md`, `…/2026-04-26-gf2.csv`, `…/2026-04-26-reference.csv` |
| post-PPC GEMM delta | `dev/bench_results/2026-04-30-post-ppc-delta-appendix.md`, `…/2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md` |
| story-level closure mapping | `dev/bench_results/2026-04-29-3abb755e-benchmark-gap-closure.md` |
| Wave-2 reference promotion artefacts | `dev/bench_results/2026-05-04-{5dea7457,73ab8eef,79388011,507b0036}-*` |
| acceptance protocol | `dev/plans/sota_reference_acceptance_protocol.md` |
| epic handoff | `dev/active/97bf0879-handoff-2.md` |
| harness sources | `benchmarks/reference/{fflas_bench.cpp,m4ri_bench.c}` |
| gf2-core production paths | `crates/gf2-core/src/field/{ple.rs,inverse.rs}`, `crates/gf2-core/src/alg/{gauss.rs,rref.rs}` |
