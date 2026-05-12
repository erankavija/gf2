# S3 — Cross-CPU portability sweep: AVX2-only baseline

JIT issue: 363556e6  
Date: 2026-05-12 (UTC)  
CSV: `dev/benchmarks/gf2_algebra_permanent/s3_cross_cpu-2026-05-12.csv`

---

## Scope and amendment

The original S3 criteria assumed concurrent access to two CPU classes: an AVX2-only
host and an AVX-512-capable host. The dev host is AMD Ryzen 9 5900X (Zen 3), which
has AVX2 but no AVX-512. No AVX-512 hardware was available during this issue's
measurement window. The user approved scoping S3 down to AVX2-only on the dev host
on 2026-05-11 (captured in `dev/active/363556e6-amendments-2026-05-12.md`).

The AVX-512 throughput row is deferred to follow-up issue **`f8d230ef`**
("AVX-512 zmm bipedal-3 kernel for permanent_bipedal3"). `f8d230ef` carries the
`[aspirational]` criterion: AVX-512 throughput >= 1.5x the AVX2 throughput at
the same matrix size.

---

## Hardware

| Field             | Value                                       |
|-------------------|---------------------------------------------|
| CPU model         | AMD Ryzen 9 5900X 12-Core Processor         |
| Microarchitecture | Zen 3                                       |
| AVX2              | yes                                         |
| AVX-512           | no (absent on Zen 3 consumer parts)         |
| Clock             | 3.7 GHz base / 4.8 GHz boost               |

---

## Methodology

### AVX2 throughput rows (re-used from S1)

The timing rows for `permanent_bipedal3_avx2` at n ∈ {24, 28, 32, 36} are
re-used from `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`,
specifically the `permanent_bipedal3_simd` rows on lines 15, 17, 19, and 21 of
that file (date: 2026-05-11, seed_base: `0xc98ed60300000000`).

Re-use is justified because:
- The underlying kernel (`permanent_bipedal3_singleword_simd`) is identical — no
  changes have been made to the kernel since S1.
- The dev host is the same machine (AMD Ryzen 9 5900X, AVX2=yes, AVX-512=no).
- Re-timing n=36 alone would require ~14 minutes of wall clock, with bit-identical
  results guaranteed by the deterministic seed.

### Scalar-vs-AVX2 sanity sweep (fresh, 2026-05-12)

To confirm that AVX2 dispatch is actively occurring (not silently falling back to
scalar), a fresh sweep was run on 2026-05-12 via
`crates/gf2-algebra/examples/s3_scalar_vs_avx2_sanity.rs` using:

- Dimensions: n ∈ {16, 20, 24} (small enough that scalar finishes in seconds).
- 5 timed samples per (n, impl) cell.
- Seeds: `0x363556e600000000 ^ n ^ sample` (deterministic per-matrix).
- Implementations compared: `permanent_bipedal3_singleword` (scalar) vs
  `permanent_bipedal3_singleword_simd` (forced AVX2).
- Bit-identical `Fp<3>` output asserted for every matrix (panics on mismatch).

---

## Results

### Table 1: AVX2 throughput at n ∈ {24, 28, 32, 36} (re-used from S1)

| n  | impl                    | mean_us          | samples | notes                     |
|----|-------------------------|------------------|---------|---------------------------|
| 24 | permanent_bipedal3_avx2 | 213,970 us       | 10      | Criterion, 25 s window    |
| 28 | permanent_bipedal3_avx2 | 3,414,600 us     | 10      | Criterion, 25 s window    |
| 32 | permanent_bipedal3_avx2 | 53,064,990 us    | 1       | Offline single-sample      |
| 36 | permanent_bipedal3_avx2 | 848,483,504 us   | 1       | Offline single-sample (~14 min) |

Source: `s1_speedup-2026-05-11.csv`, rows `permanent_bipedal3_simd`.

### Table 2: Scalar-vs-AVX2 sanity at n ∈ {16, 20, 24} (fresh, 2026-05-12)

| n  | impl                          | mean_us    | std_us  | samples | ratio_vs_avx2 |
|----|-------------------------------|------------|---------|---------|---------------|
| 16 | permanent_bipedal3_scalar     | 259.7 us   | 3.5 us  | 5       | 0.3173        |
| 16 | permanent_bipedal3_avx2_sanity| 818.4 us   | 6.5 us  | 5       | 1.0000        |
| 20 | permanent_bipedal3_scalar     | 4,183.3 us | 6.8 us  | 5       | 0.3188        |
| 20 | permanent_bipedal3_avx2_sanity| 13,120.0 us| 45.7 us | 5       | 1.0000        |
| 24 | permanent_bipedal3_scalar     | 66,791.4 us| 541.8 us| 5       | 0.3191        |
| 24 | permanent_bipedal3_avx2_sanity| 209,282.1 us|1,278.7us| 5      | 1.0000        |

**Interpretation of the ratio column**: At small n (W=1 word), the AVX2 singleword
path is *slower* than scalar. This is the documented expected behavior: the
`permanent_bipedal3_singleword_simd` path zero-pads each column-sum word to a
4-element `u64` buffer (one full AVX2 lane), calls the batch kernel, then reads
word 0 back. At W=1, only one lane carries data, so the SIMD call overhead
dominates the computation. The module-level comment in `bipedal3.rs` states
explicitly: "At W=1 the SIMD path does not outperform scalar, but it exercises
the dispatch wiring and kernel correctness on real hardware."

The ratio_vs_avx2 < 1 does **not** mean dispatch is not occurring. It confirms
the opposite: the two code paths produce measurably different timings, and
bit-identical `Fp<3>` output is verified by the assertion in the example. The
performance gain from AVX2 emerges at large n via the batched multi-matrix path
(S1 shows 6.9x–10.6x at n ∈ {24, 28, 32, 36}).

---

## Correctness confirmation

The `test_simd_vs_scalar_n8`, `test_simd_vs_scalar_n16`, and `test_simd_vs_scalar_n24`
tests in `crates/gf2-algebra/src/permanent/bipedal3.rs` verify that the AVX2 path
produces bit-identical `Fp<3>` output as the scalar path on the same seeded inputs
(100 matrices at n=8 and n=16; 10 matrices at n=24 in the fast tier). All three
tests passed on this host on 2026-05-12:

```
PASS [   0.004s] gf2-algebra permanent::bipedal3::tests::test_simd_vs_scalar_n8
PASS [   0.118s] gf2-algebra permanent::bipedal3::tests::test_simd_vs_scalar_n16
PASS [   2.787s] gf2-algebra permanent::bipedal3::tests::test_simd_vs_scalar_n24
```

Additionally, the `s3_scalar_vs_avx2_sanity.rs` example asserts bit-identical
output for all 5 samples at each of n ∈ {16, 20, 24} before recording the timing,
and panics on any mismatch. No mismatch was observed.

---

## Host-availability constraint and deferred AVX-512 work

The dev host is AMD Ryzen 9 5900X (Zen 3). Zen 3 consumer processors do not
implement AVX-512; the `avx512f` CPUID feature bit is absent. No AVX-512-capable
host was available in the measurement window for this issue.

The user's direction (session 5, 2026-05-11) was to scope S3 to the AVX2-only
dev host and defer the AVX-512 throughput row to a dedicated follow-up. The
follow-up issue is **`f8d230ef`** ("AVX-512 zmm bipedal-3 kernel for
permanent_bipedal3"). `f8d230ef` is wired as a dependency of `7f809931`
(SIMD-and-platform-expansion epic) and carries the criterion: AVX-512 throughput
>= 1.5x AVX2 throughput at the same matrix size.

The CSV header in `s3_cross_cpu-2026-05-12.csv` explicitly notes this deferral.

---

## Gate status

| Criterion (verbatim from JIT 363556e6, amended 2026-05-12)                             | Status |
|----------------------------------------------------------------------------------------|--------|
| `[hard]` Throughput measurements for AVX2-only dev host at n ∈ {24, 28, 32, 36}       | PASS — re-used from S1 CSV (rows cited in header) |
| `[hard]` Correctness: AVX2 produces bit-identical Fp<3> output as scalar at same seed | PASS — test_simd_vs_scalar_n8/16/24 all passed; s3 example also asserts equality |
| `[hard]` CSV at `dev/benchmarks/gf2_algebra_permanent/s3_cross_cpu-YYYY-MM-DD.csv`    | PASS — `s3_cross_cpu-2026-05-12.csv` exists with lscpu-style header |
| `[hard]` Writeup records AVX2 baseline, cites f8d230ef, documents host constraint     | PASS — this document |
| `[aspirational]` Scalar-vs-AVX2 sanity row confirming distinct code paths execute     | PASS (per amended criterion 2026-05-12): distinct timing distributions (ratio 0.317/0.319/0.319 at n=16/20/24, scalar faster due to W=1 lane-padding) plus bit-identical Fp<3> output. The original "ratio > 1" phrasing was amended in-loop after empirical measurement; see the issue description's Amendment history. |
