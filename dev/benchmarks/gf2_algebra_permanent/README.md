# gf2-algebra Permanent Benchmark Artefact

Publication-grade benchmark dataset for the `gf2-algebra-permanent` epic (JIT issue `7cd9afdb`).
Covers S1 (single-thread AVX2 speedup), S2 (parallel scaling), S3 (cross-CPU portability),
S5 (GPU vs CPU SIMD crossover), and S1g (GPU 50x speedup vs reference).

## Hardware Fingerprint

| Field | Value |
|-------|-------|
| CPU | AMD Ryzen 9 5900X 12-Core Processor |
| Microarchitecture | AMD Zen 3 |
| Cores | 12 physical / 24 logical (SMT 2x) |
| CPU max MHz | 4954.6 |
| L1d / L1i | 384 KiB (12 instances each) |
| L2 | 6 MiB (12 instances) |
| L3 | 64 MiB (2 instances) |
| AVX2 | yes |
| AVX-512 | no (deferred to JIT issue f8d230ef) |
| GPU | AMD Radeon RX 6950 XT |
| GPU ISA target | gfx1030 |
| GPU UUID | GPU-8cd14d6d8a3c8a73 |
| ROCm | 7.2.x (HSA Runtime 1.18; HIP cmake package 7.2.53211) |
| OS | Linux fraktaali 7.0.3-arch1-1 (Arch Linux) |
| rustc | rustc 1.95.0 (59807616e 2026-04-14) |

Full lscpu and rocminfo fields are recorded in `provenance.json`.

## Seed Pins

| Dataset | Seed |
|---------|------|
| S1 | `0xc98ed60300000000` |
| S2 n=28 | `0x4513209c0000001c` |
| S2 n=32 | `0x4513209c00000020` |
| S2 n=36 | `0x4513209c00000024` |
| S3 | `0x363556e600000000` |
| S5 | `0x00c0ffee00000000` |
| S1g | `0x9480f8a600000000` |

## Dataset Descriptions and Headline Numbers

### S1 — Single-thread AVX2 speedup vs reference (JIT c98ed603)

**File:** `s1_speedup-2026-05-11.csv` | **Snapshot:** `csvs/s1_speedup.csv`

Measures `permanent_bipedal3_simd` vs `permanent_mod3_reference` on a single thread.
n=24/28: Criterion (sample_size=10, 25s measurement_time). n=32/36: offline single-sample.

| n | Reference (us) | Bipedal3-SIMD (us) | Speedup |
|---|---------------|--------------------|---------|
| 24 | 1 473 800 | 213 970 | 6.89x |
| 28 | 27 360 000 | 3 414 600 | 8.01x |
| 32 | 500 027 842 | 53 064 990 | 9.42x |
| 36 | 9 030 740 871 | 848 483 504 | **10.64x** |

Headline: **10.64x single-thread AVX2 speedup at n=36** (ratio_vs_reference = 10.6434).

### S2 — Parallel scaling 1..12 cores (JIT 4513209c)

**File:** `s2_parallel_scaling-2026-05-11.csv` | **Snapshot:** `csvs/s2_parallel_scaling.csv`

Rayon parallel sweep at n=28/32/36, thread counts 1/2/4/8/12 (K matrices per run).
RNG: `gf2_core::rng::Lcg`, per-matrix seed = base ^ k.

| n | Threads | Scaling factor |
|---|---------|---------------|
| 36 | 12 | 0.882 |
| 32 | 12 | 0.893 |
| 28 | 12 | 0.883 |

Headline: **>=0.85x linear scaling at n>=28** satisfied at all measured n/thread combinations
(minimum scaling factor across all rows: 0.883 at n=28/36, 12 threads).

### S3 — Cross-CPU portability (JIT 363556e6)

**File:** `s3_cross_cpu-2026-05-12.csv` | **Snapshot:** `csvs/s3_cross_cpu.csv`

AVX2-only scope (AVX-512 deferred to JIT f8d230ef per amendment 2026-05-12).
AVX2 throughput data reused from S1; scalar sanity measurements taken fresh 2026-05-12.

Key observation: at small n (n in {16, 20, 24}), scalar is faster than the single-word SIMD path
(zero-padding to a 4-element AVX2 lane has overhead at W=1). At large n (from S1), AVX2 is
6.9x-10.6x faster than the reference. Two distinct code paths verified by bit-identical assertions.

### S5 — GPU vs CPU SIMD crossover at fixed M=256 (JIT a9e461de)

**File:** `s5_gpu_crossover-2026-05-15.csv` | **Snapshot:** `csvs/s5_gpu_crossover.csv`

AMD Radeon RX 6950 XT (gfx1030) vs CPU SIMD single-thread, fixed batch M=256, reps=3 (median).
Seed: `0x00c0ffee00000000`.

| n | CPU SIMD (perm/s) | GPU (perm/s) | GPU/CPU ratio |
|---|-------------------|--------------|--------------:|
| 24 | 4.79 | 137.25 | **28.65x** |
| 28 | 0.280 | 8.490 | **30.32x** |

Headline: **GPU batch 28.65x-30.32x faster than CPU SIMD** at M=256; GPU wins at both n=24 and n=28.
Crossover for this configuration is below n=24 (GPU always wins in the measured range).

### S1g — GPU 50x speedup vs reference (JIT 9480f8a6)

**File:** `s1g_gpu_speedup-2026-05-16.csv` | **Snapshot:** `csvs/s1g_gpu_speedup.csv`

AMD Radeon RX 6950 XT (gfx1030), batch M=80, compared to `permanent_mod3_reference` timing
from S1 CSV. Methodology: T_gpu_equiv = total_gpu_wallclock_s / M (batched per-matrix-equivalent);
speedup = T_reference / T_gpu_equiv.

| n | Speedup vs reference |
|---|---------------------|
| 24 | 66.47x |
| 28 | 78.18x |
| 32 | 88.58x |
| 36 | **100.24x** |

Headline: **>=50x GPU speedup vs reference satisfied at all measured n** (minimum 66.47x at n=24).

### Supporting CSVs

| File | Description |
|------|-------------|
| `paper_repro_slope-2026-05-11.csv` | Exponential-slope reproduction from bipedal3 paper baseline (n=8..24, 5 samples each) |
| `parallel_chunk_sweep-2026-05-11.csv` | Gray-code chunk-size sweep for Rayon work-stealing dispatch (chunk_size=128..4M, n=28) |

## File Index

### Top-level CSVs (canonical SSOT, dated filenames — do not rename or move)

| File | Dataset |
|------|---------|
| `s1_speedup-2026-05-11.csv` | S1 single-thread speedup |
| `s2_parallel_scaling-2026-05-11.csv` | S2 parallel scaling |
| `s3_cross_cpu-2026-05-12.csv` | S3 cross-CPU portability |
| `s5_gpu_crossover-2026-05-15.csv` | S5 GPU crossover |
| `s1g_gpu_speedup-2026-05-16.csv` | S1g GPU 50x speedup |
| `paper_repro_slope-2026-05-11.csv` | Supporting: paper slope reproduction |
| `parallel_chunk_sweep-2026-05-11.csv` | Supporting: Rayon chunk sweep |

The dated filenames are referenced by path in ROADMAP.md, dev/plans/, scripts/, source code,
and JIT issue descriptions. They are the single source of truth and must not be moved or renamed.

### csvs/ — Frozen published snapshot (conventional names)

`csvs/` is a **pinned publication snapshot** containing copies of the canonical dated CSVs
under conventional (undated) names. It exists solely to satisfy the artefact layout contract
(JIT issue 7cd9afdb success criterion: `csvs/` contains S1-S5 CSVs with conventional names).
The duplication is intentional and documented here. Provenance: each file in csvs/ was copied
from the top-level dated file listed in the table below.

| csvs/ file | Copied from |
|-----------|-------------|
| `csvs/s1_speedup.csv` | `s1_speedup-2026-05-11.csv` |
| `csvs/s2_parallel_scaling.csv` | `s2_parallel_scaling-2026-05-11.csv` |
| `csvs/s3_cross_cpu.csv` | `s3_cross_cpu-2026-05-12.csv` |
| `csvs/s5_gpu_crossover.csv` | `s5_gpu_crossover-2026-05-15.csv` |
| `csvs/s1g_gpu_speedup.csv` | `s1g_gpu_speedup-2026-05-16.csv` |

The top-level dated files remain the authoritative SSOT. The csvs/ copies are not updated
independently; if a dataset is superseded, both the dated top-level file and the csvs/ copy
must be updated together.

### figures/ — Generated by scripts/plot_permanent_benchmarks.py

All four figures are produced by running:

```
python3 scripts/plot_permanent_benchmarks.py all \
    --input-dir dev/benchmarks/gf2_algebra_permanent/ \
    --output-dir dev/benchmarks/gf2_algebra_permanent/figures/
```

| Figure | Description |
|--------|-------------|
| `figures/s1_perm_log_time_vs_n.png` | Fig (a): log-time vs n for F_3 permanent paths (S1) |
| `figures/s2_parallel_scaling.png` | Fig (b): parallel scaling factor vs core count (S2) |
| `figures/s3_cross_cpu_bars.png` | Fig (c): AVX2 vs scalar bars (S3; AVX-512 N/A gracefully) |
| `figures/s5_gpu_vs_cpu_crossover.png` | Fig (d): GPU vs CPU SIMD crossover at M=256 (S5) |

Figures were regenerated at commit `e896ce4a246999c19ee7a72c891ce71dffd74f45` using matplotlib
(Agg backend, deterministic PNG). matplotlib version pinned: 3.10.9.

### criterion/ — Not populated

No standalone Criterion JSON output files were committed as part of this epic. Criterion run
statistics are recorded inline in the CSV comment headers (see `# criterion_output:` lines in
`s1_speedup-2026-05-11.csv`). There is no `criterion/` subdirectory.

### provenance.json

Records commit SHA, rustc version, lscpu summary, rocminfo GPU identity, OS, and seed pins.
See `provenance.json` for the complete machine-readable record.

## Approval

**2026-05-17:** User signed off on this publication-grade benchmark artefact
via the project-lead escalation path, after reviewing the README and
`provenance.json` linked to JIT issue `7cd9afdb` (`jit doc list 7cd9afdb`).
The dataset (S1/S2/S3/S5/S1g CSVs, the 4 regenerated figures, `provenance.json`,
and the pinned seeds) is approved as the frozen publication snapshot for the
`gf2-algebra-permanent` epic.
